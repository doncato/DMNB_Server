pub mod state_functions {
    #![allow(non_snake_case)]

    use crate::data_handler::sqlite_handler::{DatabaseState, User};
    use crate::request_handler::handler::{ResponsePayload, Types};

    use actix_web::{web, HttpResponse};
    use chrono::{DateTime, Local, Utc};
    use linecount::count_lines;
    use serde_derive::{Deserialize, Serialize};
    use serde_json;
    use std::{
        collections::HashMap,
        convert::{TryFrom, TryInto},
        fs::OpenOptions,
        io::{prelude::*, BufReader},
        sync::mpsc::Sender,
    };
    use sysinfo::{System, SystemExt};

    #[derive(Serialize, Deserialize)]
    pub struct Payload {
        T: Option<u32>,
        Tp: Option<u32>,
        Td: Option<u32>,
        L: Option<Vec<String>>,
        O: Option<HashMap<String, Vec<String>>>,
    }
    impl Payload {
        /// Tries to create a Payload from a given string
        /// the string has to be encoded as JSON otherwise this function will panic!
        pub fn from_json(content: &String) -> Payload {
            serde_json::from_str(content).unwrap()
        }
        fn log_audit(&self, user: &User) -> Result<(), std::io::Error> {
            // TODO: Read this path from a config file!
            let logpath = "./auditlogs/";
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
            //fs::write(fullpath, content.join("\n"))?;
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

    pub fn custom_log_line(user: &User, message: String) -> Result<(), std::io::Error> {
        let time = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc).timestamp() + 1;
        let mut ot = HashMap::new();
        ot.insert("TYPE".to_string(), vec!["SYSTEM MESSAGE".to_string()]);
        ot.insert("MESSAGE".to_string(), vec![message]);
        let pl = Payload {
            T: Some(time.try_into().expect("Time went backwards")),
            Tp: None,
            Td: None,
            L: None,
            O: Some(ot),
        };
        pl.log_audit(user)?;
        Ok(())
    }

    pub fn test() -> HttpResponse {
        HttpResponse::Ok().json(ResponsePayload::new_static_message(200, "Auth Successful"))
    }
    pub fn audit(
        user: User,
        tx: Sender<(String, u32)>,
        payload: web::Json<Payload>,
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

        if let Err(err) = payload.log_audit(&user) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
        };

        HttpResponse::Ok().json(ResponsePayload::status_200())
    }
    pub fn sign(user: User, db: DatabaseState, payload: web::Json<Payload>) -> HttpResponse {
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

        match payload.log_audit(&user) {
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
        payload: web::Json<Payload>,
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
        if let Err(err) = payload.log_audit(&user) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().json(ResponsePayload::status_500());
        };

        db.kill().expect("Failed to close database!");
        HttpResponse::Ok().json(ResponsePayload::status_200())
    }

    #[derive(Serialize, Deserialize)]
    pub struct ServerStatus {
        Hostname: String,
        Description: String,
        Account: String,
        Uptime: u32,
        Maintenace: i64,
    }
    impl ServerStatus {
        fn new(
            Description: String,
            Account_Email: String,
            Uptime: u32,
            Maintenace: i64,
        ) -> ServerStatus {
            ServerStatus {
                Hostname: {
                    let s = System::new();
                    s.host_name().unwrap_or("".to_string())
                },
                Description,
                Account: Account_Email,
                Uptime,
                Maintenace,
            }
        }
    }

    pub fn stat(user: User, init_time: u32) -> HttpResponse {
        let now: u32 = chrono::offset::Utc::now()
            .timestamp()
            .try_into()
            .expect("Time went backwards");
        let diff = now - init_time;
        let r = ServerStatus::new("".to_string(), user.email, diff, -1);
        HttpResponse::Ok().json(ResponsePayload::new(200, Types::Status(r)))
    }
}
