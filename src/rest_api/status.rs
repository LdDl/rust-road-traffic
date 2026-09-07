use actix_web::{Error, HttpResponse, web};
use serde::Serialize;
use utoipa::ToSchema;

use crate::lib::logging::{self, LoggedProblem};
use crate::lib::status::{DetectionStatus, InputStatus, TrackingStatus};
use crate::rest_api::APIStorage;
use crate::settings::needs_restart;

/// Where the statistics are published, without the password
#[derive(Debug, Serialize, ToSchema)]
pub struct RedisStatus {
    pub enabled: bool,
    pub host: String,
    pub port: i32,
    pub channel: String,
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
    pub logging: LoggingStatus,
    /// The last warning or error since start, `null` if there was none
    pub last_problem: Option<LoggedProblem>,
    /// Whether the saved configuration differs from the one this run started
    /// with, i.e. whether `POST /api/mutations/restart` is due
    pub restart_required: bool,
    /// Dotted paths of the saved settings that are waiting for that restart
    #[schema(example = json!(["input.video_src"]))]
    pub pending_changes: Vec<String>,
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
    let settings = data
        .app_settings
        .read()
        .expect("Settings are poisoned [RwLock]");
    let verbose = settings.verbose.clone().unwrap_or_default();
    let log_config = verbose.to_log_config(&data.settings_filename);
    // A setting that applies live is not pending, but it is still a difference
    // worth naming, so both are reported
    let pending_changes = settings.differences_from(&data.running_settings);
    Ok(HttpResponse::Ok().json(StatusResponse {
        equipment_id: settings.equipment_info.id.clone(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: data.status.uptime_seconds(),
        input: data.status.input(),
        detection: data.status.detection(),
        tracking: data.status.tracking(),
        redis: RedisStatus {
            enabled: settings.redis_publisher.enable,
            host: settings.redis_publisher.host.clone(),
            port: settings.redis_publisher.port,
            channel: settings.redis_publisher.channel_name.clone(),
        },
        logging: LoggingStatus {
            level: log_config.level.to_string(),
            file: log_config
                .logs_folder
                .map(|folder| folder.join(logging::LOG_FILE_NAME).display().to_string()),
        },
        last_problem: logging::last_problem(),
        restart_required: pending_changes.iter().any(|path| needs_restart(path)),
        pending_changes,
    }))
}
