pub mod handler {
    use crate::sqlite_handler::User;
    use crate::data_handler::sqlite_handler::DatabaseState;
    use crate::state_engine::state_functions::{self, Payload};

    use actix_web::{
        middleware::Logger, get, post, web, App, HttpRequest, HttpResponse, HttpServer
    };
    use chrono;
    use rand::Rng;
    use serde::Serialize;
    use std::{convert::TryInto, sync::mpsc::Sender};

    #[derive(Serialize)]
    pub enum types {
        Message(String),
        User(User),
    }

    #[derive(Serialize)]
    pub struct ResponsePayload {
        status: u16,
        content: types,
    }
    impl ResponsePayload {
        /// Create a new ResponsePayload with given Status code and given content
        pub fn new(status: u16, content: types) -> ResponsePayload {
            ResponsePayload {status, content}
        }
        /// Create a new ResponsePayload with given Status code and given Message string
        pub fn new_message(status: u16, message: String) -> ResponsePayload {
            ResponsePayload {status, content: types::Message(message)}
        }
        /// Create a new ResponsePayload with given Status code and given Message slice
        pub fn new_static_message(status: u16, message: &str) -> ResponsePayload {
            ResponsePayload {status, content: types::Message(message.to_string())}
        }
        /// Create a new ResponsePayload with Status 200 and standardized Message
        pub fn status_200() -> ResponsePayload {
            ResponsePayload {status: 200, content: types::Message("Ok".to_string())}
        }
        /// Create a new ResponsePayload with Status 400 and standardized message
        pub fn status_400() -> ResponsePayload {
            ResponsePayload {status: 400, content: types::Message("Bad Request".to_string())}
        }
        /// Create a new ResponsePayload with Status 500 and standardized message
        pub fn status_500() -> ResponsePayload {
            ResponsePayload {status: 500, content: types::Message("Internal Server Error\nPlease try again later".to_string())}
        }
        
    }

    // Serve Register API
    #[post("/api/register")]
    async fn register(req: HttpRequest) -> HttpResponse {
        let email = match match req.headers().get("Email") {
            Some(val) => val.to_str().ok(),
            None => return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(400, "No Email provided")),
        } {
            Some(email) => email,
            None => return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(400, "No Email provided")),
        };
        // Check if Email is already in user database
        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone()).expect("Failed to connect to database!");
        if let Ok(user) = db.get_user_by_email(&email.to_string()) {
            if user.is_some() {
                return HttpResponse::Conflict().json(ResponsePayload::new_static_message(409, "Email already registered"))
            }
        }

        let veri_db = DatabaseState::init_with_table_name(
            req.app_data::<AppState>().unwrap().db_path.clone(),
            "verification".to_string(),
        )
        .expect("Failed to connect to Database!");
        if let Ok(obj) = veri_db.generate_verification_code(email.to_string(), true) {
            if let Some(code) = obj {
                println!("{}", code);
                // TODO: Send code via email
                return HttpResponse::Ok().json(ResponsePayload::new_static_message(200, "Awaiting verification"))
            } else {
                return HttpResponse::Conflict().json(ResponsePayload::new_static_message(409, "Email already submitted"))
            }
        } else {
            return HttpResponse::InternalServerError().json(ResponsePayload::status_500())
        }
    }
    // Serve Verification Endpoint
    #[get("/api/verify/{email}/{code}")]
    async fn verify(req: HttpRequest) -> HttpResponse {
        let email: String = req.match_info().get("email").unwrap().parse().unwrap();
        let code: u64 = req.match_info().get("code").unwrap().parse().unwrap();

        let veri_db = DatabaseState::init_with_table_name(
            req.app_data::<AppState>().unwrap().db_path.clone(),
            "verification".to_string(),
        ).expect("Failed to connect to Database!");

        if let Ok(Some(verify_obj)) = veri_db.get_verification_by_email(&email)  {
            if verify_obj.code == code {
                ()
            } else {
                return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(401, "Email and/or Code Invalid"))
            }
        } else {
            return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(401, "Email and/or Code Invalid"))
        }
        // It is now verfied that email and code are corresponding
        // Next it is verified whether or not the code is valid.
        // If it it's removed from the verification db and a new user is generated
        if let Ok(Some(found_email)) = veri_db.verify_verification_code(code) {
            let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone()).expect("Failed to connect to Database!");
            if let Ok(user) = db.new_user(&found_email) { // Idk why I use found_email over email here. However it shouldn't make any difference
                return HttpResponse::Ok().json(ResponsePayload::new(200, types::User(user)))
            } else {
                return HttpResponse::InternalServerError().json(ResponsePayload::status_500())
            }
        } else {
            return HttpResponse::BadRequest().json(ResponsePayload::new_static_message(401, "Email and/or Code Invalid"))
        }
    }
    // Serve User-Settings API
    #[post("/api/settings")]
    async fn settings(req: HttpRequest, info: web::Json<Payload>) -> HttpResponse {
        let user_id = match match req.headers().get("User-Token") {
            Some(auth) => auth.to_str().ok(),
            None => return HttpResponse::Unauthorized().json(ResponsePayload::new_static_message(401, "No User Token Provided")),
        } {
            Some(auth) => auth,
            None => return HttpResponse::Unauthorized().json(ResponsePayload::new_static_message(401, "No User Token Provided")),
        };

        let mtype = match req.headers().get("Message-Type") {
            Some(val) => val.to_str().unwrap_or("A"),
            None => "A",
        };

        let db = DatabaseState::init_with_table_name(
            req.app_data::<AppState>().unwrap().db_path.clone(),
            "verification".to_string()
        ).expect("Failed to connect to database!");

        match mtype {
            "A" => return HttpResponse::Ok().body("200 - Nothing Happened"),
            "B" => return HttpResponse::Ok().body("200 - Dummy..."),
            "C" => return HttpResponse::Ok().body("200 - Dummy..."),
            "D" => return HttpResponse::Ok().body("200 - Dummy..."),
            "E" => return HttpResponse::Ok().body("200 - Dummy..."),
            "F" => return HttpResponse::Ok().body("200 - Dummy..."),
            _ => return HttpResponse::NotFound().json(ResponsePayload::new_static_message(404, "Message Type Invalid")),
        }
        
        // TODO: Work here
    }
    // Serve Account State API
    #[post("/api/infos")]
    async fn callback(req: HttpRequest, info: web::Json<Payload>) -> HttpResponse {
        // Parse the Auth header from the request and return 401 if the header is not present or not readable.
        let auth_id = match match req.headers().get("Auth-Token") {
            Some(auth) => auth.to_str().ok(),
            None => return HttpResponse::Unauthorized().json(ResponsePayload::new_static_message(401, "No Auth Token Provided")),
        } {
            Some(auth) => auth,
            None => return HttpResponse::Unauthorized().json(ResponsePayload::new_static_message(401, "No Auth Token Provided")),
        };

        let mtype = match req.headers().get("Message-Type") {
            Some(val) => val.to_str().unwrap_or("0"),
            None => "0",
        };

        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone())
            .expect("Failed to connect to Database!");
        let user = match db.get_user_by_id(&auth_id.to_string()) {
            Ok(val) => match val {
                Some(u) => u,
                None => return HttpResponse::Unauthorized().json(ResponsePayload::new_static_message(401, "Auth Token Invalid")),
            },
            Err(_) => {
                return HttpResponse::InternalServerError()
                .json(ResponsePayload::status_500())
            }
        };

        match mtype {
            "0" => return state_functions::test(),
            "1" => {
                return state_functions::audit(
                    user,
                    req.app_data::<AppState>().unwrap().tx.clone(),
                    info,
                )
            }
            "2" => return state_functions::sign(user, db, info),
            "3" => {
                return state_functions::ilive(
                    user,
                    db,
                    req.app_data::<AppState>().unwrap().tx.clone(),
                    info,
                )
            }
            "4" => {
                return state_functions::stat(user, req.app_data::<AppState>().unwrap().init_time)
            }
            _ => return HttpResponse::NotFound().json(ResponsePayload::new_static_message(404, "Message Type Invalid")),
        }
    }

    #[derive(Debug, Clone)]
    pub struct AppState {
        db_path: String,
        tx: Sender<(String, u32)>,
        init_time: u32,
    }
    #[actix_web::main]
    pub async fn run(
        database_path: String,
        time_state_transmitter: Sender<(String, u32)>,
    ) -> std::io::Result<()> {
        // Init Database
        let state = AppState {
            db_path: database_path,
            tx: time_state_transmitter,
            init_time: chrono::offset::Utc::now()
                .timestamp()
                .try_into()
                .expect("Time went backwards"),
        };
        // u8 Array for private cokkie session
        let mut rng = rand::thread_rng();
        let mut arr = [0; 32];
        for e in arr.iter_mut() {
            *e = rng.gen()
        }
        log::debug!("COOKIE PRIVATE KEY: {:?}", arr);

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                .service(register)
                .service(verify)
                .service(settings)
                .service(callback)
                .wrap(Logger::new("%{r}a - [%tUTC] %r | %s %b "))
        })
        .workers(6)
        .bind("127.0.0.1:3030")?
        .run()
        .await
    }
}
