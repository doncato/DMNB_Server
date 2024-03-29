pub mod state_functions {
    #![allow(non_snake_case)]

    use crate::data::data_forms::{
        RequestPayload, ResponsePayload, ResponsePayloadTypes, ServerStatus, User,
    };
    use crate::data_handler::sqlite_handler::DatabaseState;

    use actix_web::{web, HttpResponse};
    use chrono::{DateTime, Local, Utc};
    use linecount::count_lines;
    use std::{
        collections::HashMap,
        convert::{TryFrom, TryInto},
        fs::OpenOptions,
        io::{prelude::*, BufReader},
        sync::mpsc::Sender,
    };

    impl RequestPayload {
        fn log_audit(&self, user: &User, logpath: &str) -> Result<(), std::io::Error> {
            let fullpath = format!("{}{}.log", logpath, user.clone().id);
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&fullpath)
                .unwrap();

            let lines = BufReader::new(&file).lines();

            let linec_diff =
                (count_lines(std::fs::File::open(&fullpath)?)? as isize - 249) as usize;
            let mut content = if linec_diff > 0 {
                lines
                    .skip(linec_diff)
                    .map(|x| x.unwrap())
                    .collect::<Vec<String>>()
            } else {
                lines.map(|x| x.unwrap()).collect::<Vec<String>>()
            };

            let utc_time = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc);
            let time_diff = utc_time.timestamp() - self.T.unwrap_or(0) as i64;

            // Log format:
            // %u %d - %l; %o
            // %u UTC Time at the time of the log entry
            // %d Seconds since the client sent the request
            // %l [Latitude, Longitude, other]
            // %o {"key": ["values"], "key": ["values"], ...}
            let new_line = format!(
                "{} {} - {:?}; {:?}\n",
                utc_time,
                time_diff,
                self.L.as_ref().unwrap_or(&vec!["-".to_string()]),
                self.O.as_ref().unwrap_or(&HashMap::new())
            );
            content.push(new_line);

            file.write(content.join("\n").as_bytes())?;
            file.flush()?;
            Ok(())
        }
    }

    /// Returns True if the difference between the given Timestamp and now is greater than zero
    fn is_positive(timestamp: &Option<u32>) -> bool {
        let utc_time = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc);
        let time_diff = utc_time.timestamp() - timestamp.unwrap_or(0) as i64;
        if time_diff < 0 {
            return false;
        }
        true
    }

    pub fn custom_log_line(
        user: &User,
        message: String,
        logpath: &str,
    ) -> Result<(), std::io::Error> {
        let time = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc).timestamp() + 1;
        let mut ot = HashMap::new();
        ot.insert("TYPE".to_string(), vec!["SYSTEM MESSAGE".to_string()]);
        ot.insert("MESSAGE".to_string(), vec![message]);
        let pl = RequestPayload {
            T: Some(time.try_into().expect("Time went backwards")),
            Tp: None,
            Td: None,
            L: None,
            O: Some(ot),
        };
        pl.log_audit(user, logpath)?;
        Ok(())
    }

    pub fn test() -> HttpResponse {
        HttpResponse::Ok().json(ResponsePayload::new_static_message(200, "Auth Successful"))
    }
    pub fn audit(
        user: User,
        tx: Sender<(String, u32)>,
        payload: web::Json<RequestPayload>,
        logpath: &str,
    ) -> HttpResponse {
        if !is_positive(&payload.T) {
            return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(
                400,
                "Timestamp can't be from the future",
            ));
        }

        if user.state >= 10 {
            return HttpResponse::Conflict().json(ResponsePayload::new_static_message(
                409,
                "You are marked as deceased",
            ));
        }

        let timestamp: u32 = match payload.Td {
            Some(time) => {
                u32::try_from(Utc::now().timestamp()).expect("Time went backwards") + time
            }
            None => 0,
        };
        if let Err(err) = tx.send((user.id.clone(), timestamp)) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
        }

        if let Err(err) = payload.log_audit(&user, logpath) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
        };

        HttpResponse::Ok().json(ResponsePayload::status_200())
    }
    pub fn sign(
        user: User,
        db: DatabaseState,
        payload: web::Json<RequestPayload>,
        logpath: &str,
    ) -> HttpResponse {
        if !is_positive(&payload.T) {
            return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(
                400,
                "Timestamp can't be from the future",
            ));
        }

        if !match db.update_state_user(&user.id, 10) {
            Ok(val) => val,
            Err(err) => {
                log::error!("{}", err);
                return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
            }
        } {
            return HttpResponse::Conflict().json(ResponsePayload::new_static_message(
                409,
                "You are marked as deceased",
            ));
        };

        match payload.log_audit(&user, logpath) {
            Err(err) => {
                log::error!("{}", err);
                return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
            }
            _ => (),
        };

        db.kill().expect("Failed to close database!");
        HttpResponse::Ok().json(ResponsePayload::status_200())
    }
    pub fn ilive(
        user: User,
        db: DatabaseState,
        tx: Sender<(String, u32)>,
        payload: web::Json<RequestPayload>,
        logpath: &str,
    ) -> HttpResponse {
        if !is_positive(&payload.T) {
            return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(
                400,
                "Timestamp can't be from the future",
            ));
        }

        let timestamp: u32 = match payload.Td {
            Some(time) => {
                u32::try_from(Utc::now().timestamp()).expect("Time went backwards") + time
            }
            None => 0,
        };
        // Update the state of the user
        if !match db.update_state_user(&user.id, 0) {
            Ok(val) => val,
            Err(err) => {
                log::error!("{}", err);
                return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
            }
        } {
            return HttpResponse::Conflict().json(ResponsePayload::new_static_message(
                409,
                "You are marked as deceased",
            ));
        };
        // Send the expected time to the thread in main.rs to collect outtimed users
        if let Err(err) = tx.send((user.id.clone(), timestamp)) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
        }
        // Log this
        if let Err(err) = payload.log_audit(&user, logpath) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
        };

        db.kill().expect("Failed to close database!");
        HttpResponse::Ok().json(ResponsePayload::status_200())
    }

    pub fn stat(user: User, init_time: u32) -> HttpResponse {
        let now: u32 = chrono::offset::Utc::now()
            .timestamp()
            .try_into()
            .expect("Time went backwards");
        let diff = now - init_time;
        let r = ServerStatus::new("".to_string(), user.email, diff, -1);
        HttpResponse::Ok().json(ResponsePayload::new(200, ResponsePayloadTypes::Status(r)))
    }
}
