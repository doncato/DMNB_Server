pub mod handler {
    use crate::data_handler::sqlite_handler::{
        DatabaseState,
    };
    use crate::state_engine::state_functions::{
        self, Payload,
    };

    use std::{
        sync::mpsc::{Sender},
    };
    use actix_web::{
        middleware::Logger,
        get, post,
        App, web,
        HttpServer, HttpResponse, HttpRequest,
        Responder,
    };
    
    #[get("/")]
    async fn index() -> impl Responder {
        HttpResponse::Ok().body("200")
    }

    #[post("/api")]
    async fn callback(req: HttpRequest, info: web::Json<Payload>) -> HttpResponse {
        // Parse the Auth header from the request and return 401 if the header is not present or not readable.
        let auth_id = match match req.headers().get("Auth") {
            Some(auth) => auth.to_str().ok(),
            None => return HttpResponse::Unauthorized().body("401"),
        } {
            Some(auth) => auth,
            None => return HttpResponse::Unauthorized().body("401"),
        };

        let ntype = match req.headers().get("Notification-Type") {
            Some(val) => val.to_str().unwrap_or("0"),
            None => "0",
        };

        let db = DatabaseState::init(req.app_data::<AppState>().unwrap().db_path.clone()).expect("Failed to connect to Database!");
        let user = match db.get_by_id(&auth_id.to_string()) {
            Ok(val) => match val {
                Some(u) => u,
                None => return HttpResponse::Unauthorized().body("401"),
            },
            Err(_) => return HttpResponse::InternalServerError().body("500"),
        };

        match ntype {
            "0" => return state_functions::test(),
            "1" => return state_functions::audit(user, req.app_data::<AppState>().unwrap().tx.clone(), info),
            "2" => return state_functions::sign(user, db, info),
            "3" => return state_functions::ilive(user, db, req.app_data::<AppState>().unwrap().tx.clone(), info),
            "4" => return state_functions::stat(),
             _  => return HttpResponse::NotFound().body("404"),
        }
    }
    
    #[derive(Debug, Clone)]
    pub struct AppState {
        db_path: String,
        tx: Sender<(String, u32)>
    }
    #[actix_web::main]
    pub async fn run(database_path: String, time_state_transmitter: Sender<(String, u32)>) -> std::io::Result<()> {
        // Init Database
        let state = AppState { db_path: database_path, tx: time_state_transmitter };

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())

                .service(index)
                .service(callback)

                .wrap(Logger::new("%{r}a - [%tUTC] %r | %s %b "))
            })
            .workers(6)
            .bind("127.0.0.1:3030")?
            .run()
            .await
    }
}