pub mod state_functions {
    use actix_web::{HttpResponse, web};
    use serde_derive::{Serialize, Deserialize};
    use std::collections::HashMap;

    #[derive(Serialize, Deserialize)]
    pub struct Payload {
        T: Option<usize>,
        Tp: Option<usize>,
        Td: Option<usize>,
        L: Option<Vec<String>>,
        O: Option<HashMap<String, Vec<String>>>,
    }

    pub fn test() -> HttpResponse {
        HttpResponse::Ok().body("200")
    }
    pub fn audit(payload: web::Json<Payload>) -> HttpResponse {
        HttpResponse::Ok().body("200")
    }
    pub fn sign(payload: web::Json<Payload>) -> HttpResponse {
        HttpResponse::Ok().body("200")
    }
    pub fn ilive(payload: web::Json<Payload>) -> HttpResponse {
        HttpResponse::Ok().body("200")
    }
    pub fn stat() -> HttpResponse {
        HttpResponse::Ok().body("200")
    }
}