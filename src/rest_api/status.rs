use actix_web::{Error, HttpResponse, web};
use serde::Serialize;
use utoipa::ToSchema;

use crate::lib::logging::{self, LoggedProblem};
use crate::lib::status::{DetectionStatus, InputStatus, TrackingStatus};
use crate::rest_api::APIStorage;
use crate::rest_api::change_state::ChangeState;
use crate::settings::EventImage;

/// Where the statistics are published, without the password
#[derive(Debug, Serialize, ToSchema)]
pub struct RedisStatus {
    pub enabled: bool,
    pub host: String,
    pub port: i32,
    pub channel: String,
    pub vehicle_events: RedisVehicleEventsStatus,
}

/// Vehicle-event publication configured at startup, without credentials
#[derive(Debug, Serialize, ToSchema)]
pub struct RedisVehicleEventsStatus {
    pub enabled: bool,
    pub host: String,
    pub port: i32,
    pub db_index: i32,
    pub channel: String,
    pub separate_connection: bool,
}

/// Recognition configured at startup
#[derive(Debug, Serialize, ToSchema)]
pub struct AnprStatus {
    pub enabled: bool,
    pub image: EventImage,
}

/// Where the log goes and how much of it
#[derive(Debug, Serialize, ToSchema)]
pub struct LoggingStatus {
    /// "info" or "debug"
    pub level: String,
    /// Path of the log file, `null` when logging only to stdout
    pub file: Option<String>,
}

/// Everything the application can say about itself right now
#[derive(Debug, Serialize, ToSchema)]
pub struct StatusResponse {
    /// Identifies this installation point
    pub equipment_id: String,
    /// Version of the running binary
    pub version: String,
    /// Seconds since this process started. A restart resets it
    pub uptime_seconds: u64,
    pub input: InputStatus,
    pub detection: DetectionStatus,
    pub tracking: TrackingStatus,
    pub redis: RedisStatus,
    pub anpr: AnprStatus,
    pub logging: LoggingStatus,
    /// The last warning or error since start, `null` if there was none
    pub last_problem: Option<LoggedProblem>,
    /// What is unsaved and what waits for a restart, the same block every
    /// changing request answers with
    #[serde(flatten)]
    pub changes: ChangeState,
}

#[utoipa::path(
    get,
    tag = "Application",
    path = "/api/status",
    responses(
        (status = 200, description = "State of the running application", body = StatusResponse)
    )
)]
/// Tells what the application is doing: which source it reads and whether
/// frames are getting through, what it detects with, how it tracks, where it
/// publishes and logs, and what went wrong last. Meant to answer the questions
/// that would otherwise need an SSH session
pub async fn status(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    // Taken before the settings below: it locks them itself
    let changes = ChangeState::of(&data);
    let settings = data
        .app_settings
        .read()
        .expect("Settings are poisoned [RwLock]");
    let verbose = settings.verbose.clone().unwrap_or_default();
    let log_config = verbose.to_log_config(&data.settings_filename);
    let running = &data.running_settings;
    let redis = &running.redis_publisher;
    let event_connection = redis
        .vehicle_events
        .connection
        .clone()
        .unwrap_or_else(|| redis.connection());
    Ok(HttpResponse::Ok().json(StatusResponse {
        equipment_id: settings.equipment_info.id.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: data.status.uptime_seconds(),
        input: data.status.input(),
        detection: data.status.detection(),
        tracking: data.status.tracking(),
        redis: RedisStatus {
            enabled: redis.enable,
            host: redis.host.clone(),
            port: redis.port,
            channel: redis.channel_name.clone(),
            vehicle_events: RedisVehicleEventsStatus {
                enabled: redis.vehicle_events.enable,
                host: event_connection.host,
                port: event_connection.port,
                db_index: event_connection.db_index,
                channel: redis.vehicle_events.channel_name.clone(),
                separate_connection: redis.vehicle_events.connection.is_some(),
            },
        },
        anpr: AnprStatus {
            enabled: running.anpr.as_ref().is_some_and(|anpr| anpr.enable),
            image: running.anpr.as_ref().map(|anpr| anpr.image).unwrap_or_default(),
        },
        logging: LoggingStatus {
            level: log_config.level.to_string(),
            file: log_config
                .logs_folder
                .map(|folder| folder.join(logging::LOG_FILE_NAME).display().to_string()),
        },
        last_problem: logging::last_problem(),
        changes,
    }))
}
