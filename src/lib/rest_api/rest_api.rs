use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use actix_web::{web, http, App, HttpServer};
use actix_cors::Cors;

use crate::lib::rest_api::services;
use crate::lib::polygons::ConvexPolygon;
use crate::lib::polygons::PolygonID;

#[actix_web::main]
pub async fn start_rest_api(server_host: String, server_port: i32, data_storage: Arc<RwLock<HashMap<PolygonID, Mutex<ConvexPolygon>>>>) -> std::io::Result<()> {
    let bind_address = format!("{}:{}", server_host, server_port);
    println!("REST API is starting on host:port {}:{}", server_host, server_port);
    let data = web::Data::new(data_storage);
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_headers(vec![http::header::ORIGIN, http::header::AUTHORIZATION, http::header::CONTENT_TYPE, http::header::CONTENT_LENGTH, http::header::ACCEPT, http::header::ACCEPT_ENCODING])
            .allowed_methods(vec!["GET"])
            .expose_headers(vec![http::header::CONTENT_LENGTH])
            .supports_credentials()
            .max_age(5600);
        App::new()
            .wrap(cors)
            .app_data(data.clone())
            .configure(services::init_routes)
    })
    .bind(&bind_address)
    .unwrap_or_else(|_| panic!("Could not bind server to address: {}", &bind_address))
    .run()
    .await
}