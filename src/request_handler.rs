pub mod handler {
    use crate::data_handler::sqlite_handler::DatabaseState;
    use crate::state_engine::state_functions::{self, Payload};

    use actix_files;
    use actix_session::{CookieSession, Session};
    use actix_web::{
        get, middleware::Logger, post, web, App, Error, HttpRequest, HttpResponse, HttpServer,
    };
    use chrono;
    use rand::Rng;
    use std::{convert::TryInto, sync::mpsc::Sender};

    // HELPERS
    /* This will be removed as it is part of the web-frontend
    fn redirect_dashboard_sign_in(session: &Session) -> Result<Option<HttpResponse>, Error> {
        let session_logged_in = session.get::<String>("Email")?.is_some()
            && session.get::<String>("Auth-Token")?.is_some();
        if session_logged_in {
            return Ok(Some(
                HttpResponse::MovedPermanently()
                    .header("Cache-Control", "no-store")
                    .header("Location", "/dashboard")
                    .finish(),
            ));
        } else {
            Ok(None)
        }
    }
    */
    // Serve index.html via root request
    /* This will be removed as the frontend will be seperated
    #[get("/")]
    async fn auto_index(req: HttpRequest, session: Session) -> Result<HttpResponse, Error> {
        // Check if user is already logged in
        if let Some(early_redirect) = redirect_dashboard_sign_in(&session)? {
            return Ok(early_redirect);
        };
        actix_files::NamedFile::open("web-open/index.html")?.into_response(&req)
    }
    // Serve login/registration
    #[post("/auth")]
    async fn sign_in(
        req: HttpRequest,
        body: String,
        session: Session,
    ) -> Result<HttpResponse, Error> {
        // Check if user is already logged in
        if let Some(early_redirect) = redirect_dashboard_sign_in(&session)? {
            return Ok(early_redirect);
        };

        let email_start = match body.find("Email=") {
            Some(val) => val + 6,
            None => return Ok(HttpResponse::BadRequest().body("400 - No Email provided")),
        };
        let email_end = body[email_start..].find(" ").unwrap_or(body.len());
        let email = body[email_start..email_end].replace("%40", "@");

        // Check if Email is already registered
        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone())
            .expect("Failed to connect to Database!");
        match match db.get_user_by_email(&email) {
            Ok(val) => val,
            Err(err) => {
                return Ok(HttpResponse::InternalServerError()
                    .body(format!("500 - Failed to interact with Database\n{}", err)))
            }
        } {
            Some(_) => {
                session.set("Email", email)?;
                return actix_files::NamedFile::open("web-hidden/login.html")?.into_response(&req);
            }
            None => {
                session.set("Email", email)?;
                /*
                return actix_files::NamedFile::open("web-hidden/register.html")?
                    .into_response(&req);
                */
                return Ok(HttpResponse::MovedPermanently()
                    .header("Cache-Control", "no-store")
                    .header("Location", "/register")
                    .finish());
            }
        };
    }
    // Serve login
    #[post("/login")]
    async fn login(
        req: HttpRequest,
        body: String,
        session: Session,
    ) -> Result<HttpResponse, Error> {
        // Check if user is already logged in
        if let Some(early_redirect) = redirect_dashboard_sign_in(&session)? {
            return Ok(early_redirect);
        };
        let token_start = match body.find("Auth-Token=") {
            Some(val) => val + 11,
            None => return Ok(HttpResponse::BadRequest().body("400 - No Auth-Token provided")),
        };
        let token_end = body[token_start..].find(" ").unwrap_or(body.len());
        let token = body[token_start..token_end].to_string();

        let email = session.get::<String>("Email")?.unwrap_or("".to_string());

        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone())
            .expect("Failed to connect to Database!");
        match match db.get_user_by_email(&email) {
            Ok(val) => val,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError()
                    .body("500 - Failed to interact with Database"))
            }
        } {
            Some(user) => {
                if user.id == token {
                    session.set("Auth-Token", token)?;
                    return Ok(HttpResponse::MovedPermanently()
                        .header("Cache-Control", "no-store")
                        .header("Location", "/dashboard")
                        .finish());
                } else {
                    return Ok(HttpResponse::MovedPermanently()
                        .header("Cache-Control", "no-store")
                        .header("Location", "/")
                        .finish());
                }
            }
            None => {
                return Ok(HttpResponse::MovedPermanently()
                    .header("Cache-Control", "no-store")
                    .header("Location", "/")
                    .finish());
            }
        }
    }
    // Serve register
    #[get("/register")]
    async fn register(req: HttpRequest, session: Session) -> Result<HttpResponse, Error> {
        let email = session.get::<String>("Email")?.unwrap_or("".to_string());
        if email == "".to_string() {
            return Ok(HttpResponse::BadRequest().body("400 - Email invalid"))
        }
        let db = DatabaseState::init_with_table_name(
            req.app_data::<AppState>().unwrap().db_path.clone(),
            "verification".to_string(),
        )
        .expect("Failed to connect to Database!");
        let found = match db.generate_verification(email, false) {
            Ok(val) => val,
            Err(_) => {
                return Ok(HttpResponse::InternalServerError()
                    .body("500 - Failed to interact with Database"))
            }
        };
        let entry = match found {
            Some(r) => r,
            None => {
                return Ok(
                    HttpResponse::Conflict().body("409 - Email already for verification submitted")
                );
            }
        };

        return Ok(HttpResponse::Ok().body(format!(
            "Email: {}\nCode: {}\nLink: <a href=\"/verify/{}\" />",
            entry.email, entry.code, entry.code
        )));
    }
    #[get("/verify/{code}")]
    async fn verify(req: HttpRequest) -> Result<HttpResponse, Error> {
        let code: u64 = req.match_info().get("code").unwrap().parse().unwrap();
        let user_db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone())
            .expect("Failed to connect to Database!");
        let veri_db = DatabaseState::init_with_table_name(
            req.app_data::<AppState>().unwrap().db_path.clone(),
            "verification".to_string(),
        )
        .expect("Failed to connect to Database!");
        if let Some(email) = veri_db.verify_verification_code(code).unwrap() {
            let user = user_db.new_user(&email).unwrap();
            return Ok(HttpResponse::Ok().body(format!("{}", user)));
        } else {
            return Ok(HttpResponse::Unauthorized().body("401 - Verification code invalid"))
        }
    }
    // Serve dashboard
    #[get("/dashboard")]
    async fn dashboard(req: HttpRequest, session: Session) -> Result<HttpResponse, Error> {
        // Check if user is eligible
        // This just checks if the fields Email and Auth-Token are set (should be safe enough)
        if !redirect_dashboard_sign_in(&session)?.is_some() {
            return Ok(HttpResponse::MovedPermanently()
                .header("Cache-Control", "no-store")
                .header("Location", "/")
                .finish());
        };
        return actix_files::NamedFile::open("web-hidden/dashboard.html")?.into_response(&req);
    }
    // Serve logout
    #[get("/clear")]
    async fn logout(session: Session) -> Result<HttpResponse, Error> {
        session.clear();
        return Ok(HttpResponse::MovedPermanently()
            .header("Cache-Control", "no-store")
            .header("Location", "/")
            .finish());
    }
    // Serve static files in web
    #[get("/")]
    async fn index(req: HttpRequest) -> Result<HttpResponse, Error> {
        let home_redirect = ["/index", "/auth", "/login"];
        for path in home_redirect {
            if req.path().starts_with(path) {
                return Ok(HttpResponse::MovedPermanently()
                    .header("Location", "/")
                    .finish());
            }
        }
        actix_files::NamedFile::open(format!("web-open/{}", req.path()))?.into_response(&req)
    }
    */

    
    // Serve User-Settings Settings API
    #[post("/settings")]
    async fn settngs(req: HttpRequest, info: web::Json<Payload>) -> HttpResponse {
        let user_id = match match req.headers().get("User-Token") {
            Some(auth) => auth.to_str().ok(),
            None => return HttpResponse::Unauthorized().body("401 - No User Token Provided"),
        } {
            Some(auth) => auth,
            None => return HttpResponse::Unauthorized().body("401 - No User Token Provided"),
        };

        let mtype = match req.headers().get("Message-Type") {
            Some(val) => val.to_str().unwrap_or("0"),
            None => "0",
        };

        let db = DatabaseState::init_with_table_name(req.app_data::<AppState>().unwrap().db_path.clone(), "settings").expect("Failed to connect to Database!");
        
        // TODO: Work here
    }
    // Serve Data API Backend
    #[post("/api")]
    async fn callback(req: HttpRequest, info: web::Json<Payload>) -> HttpResponse {
        // Parse the Auth header from the request and return 401 if the header is not present or not readable.
        let auth_id = match match req.headers().get("Auth-Token") {
            Some(auth) => auth.to_str().ok(),
            None => return HttpResponse::Unauthorized().body("401 - No Auth Token Provided"),
        } {
            Some(auth) => auth,
            None => return HttpResponse::Unauthorized().body("401 - No Auth Token Provided"),
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
                None => return HttpResponse::Unauthorized().body("401 - Auth Token Invalid"),
            },
            Err(_) => {
                return HttpResponse::InternalServerError()
                    .body("500 - Internal Error. Please Try Again Later")
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
            _ => return HttpResponse::NotFound().body("404"),
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
        println!("COOKIE PRIVATE KEY: {:?}", arr);

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                // Handle Root requests
                .service(auto_index)
                // Handle Sign-In, Login, Registraion, Verification and Logout
                .service(sign_in)
                .service(login)
                .service(register)
                .service(verify)
                .service(logout)
                // Handle Dashboard for logged in users
                .service(dashboard)
                // Handle all get requests
                .service(index)
                // Handle API Requests
                .service(callback)
                .wrap(
                    CookieSession::private(&arr)
                        .name("session-data")
                        .secure(true)
                        .max_age(1200),
                )
                .wrap(Logger::new("%{r}a - [%tUTC] %r | %s %b "))
        })
        .workers(6)
        .bind("127.0.0.1:3030")?
        .run()
        .await
    }
}
