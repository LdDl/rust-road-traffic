use crate::lib::logging;
use actix_cors::Cors;
use actix_web::{App, HttpResponse, HttpServer, http, web};
use std::sync::{Arc, RwLock};
use tracing::{error, info};

use crate::lib::data_storage::ThreadedDataStorage;
use crate::lib::mjpeg_streaming::Broadcaster;
use crate::lib::status::RuntimeStatus;
use crate::rest_api::errors::ErrorResponse;
use crate::rest_api::{change_state, services};
use crate::settings::AppSettings;
use std::sync::{Mutex, mpsc::Receiver};

pub struct APIStorage {
    pub data_storage: ThreadedDataStorage,
    /// Changed through `PATCH /api/config`, so it is shared rather than owned:
    /// the copy the detection loop started with stays as it was until a restart
    pub app_settings: RwLock<AppSettings>,
    /// The settings this run was started with. Comparing them with the ones
    /// above is what tells a change apart from a change that has taken effect
    pub running_settings: AppSettings,
    /// What the configuration file holds, as of the last `save_toml`: the
    /// only thing that writes it. Comparing with it is what tells a saved
    /// change apart from one that would be lost by a restart
    pub saved_settings: RwLock<AppSettings>,
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
    // The zones are read back from the running process rather than taken from
    // the file as written, so that they compare in the same form later on: a
    // legacy "lrtb" in the file is "inbound" once loaded
    let mut saved_settings = app_settings.clone();
    saved_settings.road_lanes = Some(change_state::live_road_lanes(&data_storage));
    let storage = APIStorage {
        data_storage: data_storage,
        running_settings: app_settings.clone(),
        saved_settings: RwLock::new(saved_settings),
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
            // Every method the API answers with, so that a browser's preflight
            // for anything but GET and POST is not turned away. CORS guards
            // nothing here anyway: the port is reachable directly
            .allowed_methods(vec!["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"])
            .expose_headers(vec![http::header::CONTENT_LENGTH])
            .max_age(5600);
        // What exactly went wrong goes to the log, where the detail is useful;
        // the caller is told that the body was not taken and nothing about how
        // this app is built. The settings it does take are in /api/docs
        let json_errors = web::JsonConfig::default().error_handler(|err, _| {
            error!(
                scope = logging::SCOPE_REST_API,
                error = %err,
                "Rejected a request body"
            );
            actix_web::error::InternalError::from_response(
                "",
                HttpResponse::BadRequest().json(ErrorResponse::text("invalid json")),
            )
            .into()
        });
        App::new()
            .wrap(cors)
            .app_data(data.clone())
            .app_data(json_errors)
            .configure(services::init_routes(enable_mjpeg))
    })
    .bind(&bind_address)?
    .run()
    .await
}
