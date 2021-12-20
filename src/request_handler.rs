pub mod handler {
    use crate::data_handler::sqlite_handler::DatabaseState;
    use crate::state_engine::state_functions::{self, Payload};

    use actix_files;
    use actix_session::{CookieSession, Session};
    use actix_web::{
        get, middleware::Logger, post, web, App, Error, HttpRequest, HttpResponse, HttpServer,
    };
    use chrono;
    use std::{convert::TryInto, sync::mpsc::Sender};

    // Serve index.html via root request
    #[get("/")]
    async fn auto_index() -> Result<actix_files::NamedFile, Error> {
        Ok(actix_files::NamedFile::open("web-open/index.html")?)
    }
    // Serve login/registration
    #[post("/auth")]
    async fn sign_in(
        req: HttpRequest,
        body: String,
        session: Session,
    ) -> Result<HttpResponse, Error> {
        let email_start = match body.find("Email=") {
            Some(val) => val + 6,
            None => return Ok(HttpResponse::BadRequest().body("400 - No Email provided")),
        };
        let email_end = body[email_start..].find(" ").unwrap_or(body.len());
        let email = body[email_start..email_end].replace("%40", "@");

        // Check if Email is already registered
        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone())
            .expect("Failed to connect to Database!");
        match match db.get_by_email(&email) {
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
                return actix_files::NamedFile::open("web-hidden/register.html")?
                    .into_response(&req);
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
        let token_start = match body.find("Auth-Token=") {
            Some(val) => val + 11,
            None => return Ok(HttpResponse::BadRequest().body("400 - No Auth-Token provided")),
        };
        let token_end = body[token_start..].find(" ").unwrap_or(body.len());
        let token = body[token_start..token_end].to_string();

        let email = session.get::<String>("Email")?.unwrap_or("".to_string());

        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone())
            .expect("Failed to connect to Database!");
        match match db.get_by_email(&email) {
            Ok(val) => val,
            Err(err) => {
                return Ok(HttpResponse::InternalServerError()
                    .body(format!("500 - Failed to interact with Database\n{}", err)))
            }
        } {
            Some(user) => {
                if user.id == token {
                    session.set("Auth-Token", token)?;
                    return Ok(HttpResponse::MovedPermanently()
                        .header("Location", "/dashboard")
                        .finish());
                } else {
                    println!("TOKEN: {}", token);
                    return Ok(HttpResponse::MovedPermanently()
                        .header("Location", "/")
                        .finish());
                }
            }
            None => {
                println!("EMAIL: {}", email);
                return Ok(HttpResponse::MovedPermanently()
                    .header("Location", "/")
                    .finish());
            }
        }
    }
    // Serve dashboard
    #[get("/dashboard")]
    async fn dashboard(req: HttpRequest, session: Session) -> Result<HttpResponse, Error> {
        // Check if user is eligible
        // This just checks if the fields Email and Auth-Token are set (should be safe enough)
        let eligible = session.get::<String>("Email")?.is_some()
            && session.get::<String>("AuthToken")?.is_some();
        if !eligible {
            return Ok(HttpResponse::MovedPermanently()
                .header("Location", "/")
                .finish());
        }
        return actix_files::NamedFile::open("web-hidden/dashboard.html")?.into_response(&req);
    }
    // Serve static files in web
    #[get("/*")]
    async fn index(req: HttpRequest) -> Result<HttpResponse, Error> {
        let home_redirect = ["/auth", "/login", "/verify"];
        for path in home_redirect {
            if path == req.path() {
                return Ok(HttpResponse::MovedPermanently()
                    .header("Location", "/")
                    .finish());
            }
        }
        actix_files::NamedFile::open(format!("web-open/{}", req.path()))?.into_response(&req)
    }

    // Serve API Backend
    #[post("/api")]
    async fn callback(req: HttpRequest, info: web::Json<Payload>) -> HttpResponse {
        // Parse the Auth header from the request and return 401 if the header is not present or not readable.
        let auth_id = match match req.headers().get("Auth") {
            Some(auth) => auth.to_str().ok(),
            None => return HttpResponse::Unauthorized().body("401 - No Auth Token Provided"),
        } {
            Some(auth) => auth,
            None => return HttpResponse::Unauthorized().body("401 - No Auth Token Provided"),
        };

        let ntype = match req.headers().get("Notification-Type") {
            Some(val) => val.to_str().unwrap_or("0"),
            None => "0",
        };

        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone())
            .expect("Failed to connect to Database!");
        let user = match db.get_by_id(&auth_id.to_string()) {
            Ok(val) => match val {
                Some(u) => u,
                None => return HttpResponse::Unauthorized().body("401 - Auth Token Invalid"),
            },
            Err(_) => {
                return HttpResponse::InternalServerError()
                    .body("500 - Internal Error. Please Try Again Later")
            }
        };

        match ntype {
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

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                .service(auto_index)
                .service(index)
                .service(sign_in)
                .service(login)
                .service(dashboard)
                .service(callback)
                .wrap(
                    CookieSession::private(&[0; 32])
                        .name("session-data")
                        .secure(true)
                        .expires_in(57600),
                )
                .wrap(Logger::new("%{r}a - [%tUTC] %r | %s %b "))
        })
        .workers(6)
        .bind("127.0.0.1:3030")?
        .run()
        .await
    }
}
