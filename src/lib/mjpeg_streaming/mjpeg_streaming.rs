use std::sync::{Arc, RwLock};
use actix_web::{web, http, App, HttpServer};
use actix_cors::Cors;

use crate::lib::rest_api::services;
use crate::lib::data_storage::DataStorage;

#[actix_web::main]
pub async fn start_mjpeg_streaming(server_host: String, server_port: i32) -> std::io::Result<()> {
    let bind_address = format!("{}:{}", server_host, server_port);
    println!("MJPEG Streamer is starting on host:port {}:{}", server_host, server_port);
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
            .configure(init_routes)
    })
    .bind(&bind_address)
    .unwrap_or_else(|_| panic!("Could not bind MJPEG streamer to address: {}", &bind_address))
    .run()
    .await
}

async fn mjpeg_page() -> impl Responder {
    let content = include_str!("index.html");
    return HttpResponse::Ok().header("Content-Type", "text/html").body(content);
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/live", web::get().to(mjpeg_page))
}