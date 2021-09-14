pub mod state_functions {
    #![allow(non_snake_case)]

    use crate::data_handler::sqlite_handler::{
        DatabaseState, User,
    };

    use std::{
        convert::{TryFrom, TryInto},
        sync::mpsc::{Sender},
        fs::{OpenOptions},
        io::{BufReader, prelude::*},
        collections::HashMap,
    };
    use chrono::{DateTime, Local, Utc};
    use linecount::count_lines;
    use actix_web::{HttpResponse, web};
    use serde_derive::{Serialize, Deserialize};
    use serde_json;

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
            let logpath = "/extern/prog/rust/dmnb_server_relais/auditlogs/";
            let fullpath = format!("{}{}.log", logpath, user.clone().id);
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(&fullpath)
                .unwrap();

            let lines = BufReader::new(&file).lines();

            let linec_diff = (count_lines(std::fs::File::open(&fullpath)?)? as isize - 249) as usize;
            let mut content = if  linec_diff > 0 {
                lines
                    .skip(linec_diff)
                    .map(|x| x.unwrap())
                    .collect::<Vec<String>>()
            } else {
                lines
                    .map(|x| x.unwrap())
                    .collect::<Vec<String>>()
            };

            let utc_time = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc);
            let time_diff = utc_time.timestamp() - self.T.unwrap_or(0) as i64;

            // Log format:
            // %u %d - %l; %o
            // %u UTC Time at the time of the log entry
            // %d Seconds since the client sent the request
            // %l [Latitude, Longitude, other]
            // %o {"key": ["values"], "key": ["values"], ...} 
            let new_line = format!("{} {} - {:?}; {:?}\n", utc_time, time_diff, self.L.as_ref().unwrap_or(&vec!["-".to_string()]), self.O.as_ref().unwrap_or(&HashMap::new()));
            content.push(new_line);

            file.write(content.join("\n").as_bytes())?;
            file.flush()?;
            //fs::write(fullpath, content.join("\n"))?;
            Ok(())
        }
    }
    fn is_positive(timestamp: &Option<u32>) -> bool{
        let utc_time = DateTime::<Utc>::from_utc(Local::now().naive_utc(), Utc);
        let time_diff = utc_time.timestamp() - timestamp.unwrap_or(0) as i64;
        if time_diff < 0 {
            return false
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
            O: Some(ot)
        };
        pl.log_audit(user)?;
        Ok(())
    }

    pub fn test() -> HttpResponse {
        HttpResponse::Ok().body("200")
    }
    pub fn audit(user: User, tx: Sender<(String, u32)>, payload: web::Json<Payload>) -> HttpResponse {
        if !is_positive(&payload.T) {
            return HttpResponse::BadRequest().body("400 - Timestamp can't come from the future")
        }

        let timestamp: u32 = match payload.Td {
            Some(time) => u32::try_from(chrono::offset::Utc::now().timestamp()).expect("Time went backwards") + time,
            None => 0,
        };
        if let Err(err) = tx.send((user.id.clone(), timestamp)) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().body("500 - Failed to Sync User Expiring Time")
        }

        if let Err(err) = payload.log_audit(&user) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().body("500 - Failed to Log the Request")
        };

        HttpResponse::Ok().body("200 - OK")
    }
    pub fn sign(user: User, db: DatabaseState, payload: web::Json<Payload>) -> HttpResponse {
        if !is_positive(&payload.T) {
            return HttpResponse::BadRequest().body("400 - Timestamp can't come from the future")
        }

        match payload.log_audit(&user) {
            Err(err) => {
                log::error!("{}", err);
                return HttpResponse::InternalServerError().body("500 - Failed to Log the Request")
            },
            _ => (),
        };

        if !match db.update_state(&user.id, 10) {
            Ok(val) => val,
            Err(err) => {
                log::error!("{}", err);
                return HttpResponse::InternalServerError().body("500 - Failed to Update Database")
            },
        } {
            return HttpResponse::Conflict().body("409 - You are marked as deceased")
        };


        
        db.kill().expect("Failed to close database!");
        HttpResponse::Ok().body("200")
    }
    pub fn ilive(user: User, db: DatabaseState, tx: Sender<(String, u32)>, payload: web::Json<Payload>) -> HttpResponse {
        if !is_positive(&payload.T) {
            return HttpResponse::BadRequest().body("400 - Timestamp can't come from the future")
        }

        let timestamp: u32 = match payload.Td {
            Some(time) => u32::try_from(chrono::offset::Utc::now().timestamp()).expect("Time went backwards") + time,
            None => 0,
        };
        if let Err(err) = tx.send((user.id.clone(), timestamp)) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().body("500 - Failed to Sync User Expiring Time")
        }

        if let Err(err) = payload.log_audit(&user) {
            log::error!("{}", err);
            return HttpResponse::InternalServerError().body("500 - Failed to Log the Request")
        };

        if !match db.update_state(&user.id, 0) {
            Ok(val) => val,
            Err(err) => {
                log::error!("{}", err);
                return HttpResponse::InternalServerError().body("500 - Failed to Update Database")
            },
        } {
            return HttpResponse::Conflict().body("409 - You are marked as deceased")
        };


        
        db.kill().expect("Failed to close database!");
        HttpResponse::Ok().body("200")
    }
    pub fn stat() -> HttpResponse {
        HttpResponse::Ok().body("200")
    }
}