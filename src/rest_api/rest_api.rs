use std::sync::{Arc, RwLock};
use actix_web::{web, http, App, HttpServer};
use actix_cors::Cors;

use crate::settings::AppSettings;
use crate::rest_api::services;
use crate::lib::data_storage::ThreadedDataStorage;
use crate::lib::mjpeg_streaming::Broadcaster;
use std::sync::{
    Mutex,
    mpsc::{
        Receiver
    }
};
use opencv::{
    core::Vector,
};

pub struct APIStorage {
    pub data_storage: ThreadedDataStorage,
    pub app_settings: AppSettings,
    pub settings_filename: String,
    pub mjpeg_broadcaster: web::Data<Mutex<Broadcaster>>
}

#[actix_web::main]
pub async fn start_rest_api(server_host: String, server_port: i32, data_storage: ThreadedDataStorage, enable_mjpeg: bool, rx_frames_data: Receiver<Vector<u8>>, app_settings: AppSettings, settings_filename: &str) -> std::io::Result<()> {
    let bind_address = format!("{}:{}", server_host, server_port);
    println!("REST API is starting on host:port {}:{}", server_host, server_port);
    let storage = APIStorage{
        data_storage: data_storage,
        app_settings: app_settings,
        settings_filename: settings_filename.to_string(),
        mjpeg_broadcaster: web::Data::new(Mutex::new(Broadcaster::default())),
    };

    /* Enable MJPEG streaming server if needed */
    if enable_mjpeg {
        Broadcaster::spawn_reciever(storage.mjpeg_broadcaster.clone(), rx_frames_data);
    }

    let data = web::Data::new(storage);
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_headers(vec![http::header::ORIGIN, http::header::AUTHORIZATION, http::header::CONTENT_TYPE, http::header::CONTENT_LENGTH, http::header::ACCEPT, http::header::ACCEPT_ENCODING])
            .allowed_methods(vec!["GET", "POST"])
            .expose_headers(vec![http::header::CONTENT_LENGTH])
            .supports_credentials()
            .max_age(5600);
        App::new()
            .wrap(cors)
            .app_data(data.clone())
            .configure(services::init_routes(enable_mjpeg))
    })
    .bind(&bind_address)
    .unwrap_or_else(|_| panic!("Could not bind server to address: {}", &bind_address))
    .run()
    .await
}

