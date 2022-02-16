use actix_web::{web, http, App, HttpServer, HttpResponse, Responder};
use actix_cors::Cors;

use crate::lib::mjpeg_streaming::{
    broadcaster::Broadcaster
};

use std::sync::Mutex;
use std::sync::mpsc::{
    Receiver
};

#[actix_web::main]
pub async fn start_mjpeg_streaming(server_host: String, server_port: i32, rx_frames_data: Receiver<std::vec::Vec<u8>>, input_width: u32, input_height: u32) -> std::io::Result<()> {
    let bind_address = format!("{}:{}", server_host, server_port);
    println!("MJPEG Streamer is starting on host:port {}:{}", server_host, server_port);

    let broadcaster = web::Data::new(Mutex::new(Broadcaster::default()));
    Broadcaster::spawn_reciever(broadcaster.clone(), rx_frames_data, input_width, input_height);

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
            .app_data(broadcaster.clone())
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

async fn add_new_client(broadcaster: web::Data<Mutex<Broadcaster>>) -> impl Responder {
    let rx = broadcaster.lock().unwrap().add_client();
    HttpResponse::Ok()
        .header("Cache-Control", "no-store, must-revalidate")
        .header("Pragma", "no-cache")
        .header("Expires", "0")
        .header("Connection", "close")
        .header(
            "Content-Type",
            "multipart/x-mixed-replace;boundary=boundarydonotcross",
        )
        // .no_chunking()
        .streaming(rx)
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/live", web::get().to(mjpeg_page))
        .route("/live_streaming", web::get().to(add_new_client));
}