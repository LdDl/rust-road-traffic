use crate::lib::logging;
use actix_cors::Cors;
use actix_web::{App, HttpServer, http, web};
use std::sync::{Arc, RwLock};
use tracing::info;

use crate::lib::data_storage::ThreadedDataStorage;
use crate::lib::mjpeg_streaming::Broadcaster;
use crate::lib::status::RuntimeStatus;
use crate::rest_api::services;
use crate::settings::AppSettings;
use std::sync::{Mutex, mpsc::Receiver};

pub struct APIStorage {
    pub data_storage: ThreadedDataStorage,
    /// Changed through `PUT /api/config`, so it is shared rather than owned:
    /// the copy the detection loop started with stays as it was until a restart
    pub app_settings: RwLock<AppSettings>,
    /// The settings this run was started with. Comparing them with the ones
    /// above is what tells a change apart from a change that has taken effect
    pub running_settings: AppSettings,
    pub settings_filename: String,
    pub mjpeg_broadcaster: web::Data<Mutex<Broadcaster>>,
    /// What the detection loop has learned about the run so far
    pub status: Arc<RuntimeStatus>,
}

#[actix_web::main]
pub async fn start_rest_api(
    server_host: String,
    server_port: i32,
    data_storage: ThreadedDataStorage,
    enable_mjpeg: bool,
    rx_frames_data: Receiver<Vec<u8>>,
    app_settings: AppSettings,
    settings_filename: &str,
    status: Arc<RuntimeStatus>,
) -> std::io::Result<()> {
    let bind_address = format!("{}:{}", server_host, server_port);
    info!(scope = logging::SCOPE_REST_API, host = %server_host, port = server_port, "REST API starting");
    let storage = APIStorage {
        data_storage: data_storage,
        running_settings: app_settings.clone(),
        app_settings: RwLock::new(app_settings),
        settings_filename: settings_filename.to_string(),
        mjpeg_broadcaster: web::Data::new(Mutex::new(Broadcaster::default())),
        status: status,
    };

    /* Enable MJPEG streaming server if needed */
    if enable_mjpeg {
        Broadcaster::spawn_reciever(storage.mjpeg_broadcaster.clone(), rx_frames_data);
    }

    let data = web::Data::new(storage);
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_headers(vec![
                http::header::ORIGIN,
                http::header::AUTHORIZATION,
                http::header::CONTENT_TYPE,
                http::header::CONTENT_LENGTH,
                http::header::ACCEPT,
                http::header::ACCEPT_ENCODING,
            ])
            .allowed_methods(vec!["GET", "POST"])
            .expose_headers(vec![http::header::CONTENT_LENGTH])
            .supports_credentials()
            .max_age(5600);
        App::new()
            .wrap(cors)
            .app_data(data.clone())
            .configure(services::init_routes(enable_mjpeg))
    })
    .bind(&bind_address)?
    .run()
    .await
}
