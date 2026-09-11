//! What the configuration API shows and accepts.
//!
//! These types are written out rather than derived from the settings file,
//! and that is the point: a setting reaches the API by being named here, so
//! anything added to `AppSettings` later stays private until someone decides
//! otherwise. It is also why the model is simply absent instead of filtered
//! out - which model runs, how it is shaped and what it can recognise belong
//! to how the device was built, not to whoever operates it.
use actix_web::{Error, HttpResponse, web};
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;

use crate::lib::logging;
use crate::rest_api::APIStorage;
use crate::rest_api::change_state::ChangeState;
use crate::settings::{AppSettings, KALMAN_FILTERS, TRACKER_TYPES};

/// Error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// What was wrong with the request
    #[schema(example = "Invalid tracker type: 'sort'. Supported: iou_naive, bytetrack")]
    pub error_text: String,
}

/// Result of a configuration change
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateConfigResponse {
    #[schema(example = "ok")]
    pub message: &'static str,
    /// Dotted paths of the settings this request actually changed
    #[schema(example = json!(["input.video_src"]))]
    pub changed: Vec<String>,
    /// Everything left unsaved or waiting for a restart, not only this
    /// request's part
    #[serde(flatten)]
    pub state: ChangeState,
}

/// The configuration as it is saved
#[derive(Debug, Serialize, ToSchema)]
pub struct ConfigView {
    pub input: InputView,
    pub tracking: TrackingView,
    pub equipment_info: EquipmentView,
    pub worker: WorkerView,
    pub redis_publisher: RedisView,
    pub verbose: VerboseView,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InputView {
    /// Video file, RTSP URL, camera index or GStreamer pipeline
    pub video_src: String,
    /// Detection runs on every N-th decoded frame
    pub process_every_nth_frame: u32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TrackingView {
    /// "iou_naive" or "bytetrack"
    #[serde(rename = "type")]
    pub typ: String,
    /// "centroid" or "bbox"
    pub kalman_filter: String,
    /// How long a track survives without detections
    pub max_lost_seconds: Option<f32>,
    /// The same limit as a frame count, used only when the seconds are not set
    pub max_no_match: Option<usize>,
    pub max_points_in_track: usize,
    pub iou_threshold: Option<f32>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EquipmentView {
    /// Identifies this installation point in everything the app publishes
    pub id: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WorkerView {
    /// Length of a statistics window
    pub reset_data_milliseconds: i64,
}

/// Where statistics are published
#[derive(Debug, Serialize, ToSchema)]
pub struct RedisView {
    pub enable: bool,
    pub host: String,
    pub port: i32,
    /// ACL user, if the server has them
    pub username: Option<String>,
    pub db_index: i32,
    pub channel_name: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VerboseView {
    /// "info" or "debug"
    pub level: Option<String>,
    pub logs_folder: Option<String>,
    pub max_file_size_mb: Option<u64>,
    pub max_files: Option<usize>,
}

impl From<&AppSettings> for ConfigView {
    fn from(settings: &AppSettings) -> Self {
        let tracking = &settings.tracking;
        let redis = &settings.redis_publisher;
        let verbose = settings.verbose.clone().unwrap_or_default();
        ConfigView {
            input: InputView {
                video_src: settings.input.video_src.clone(),
                process_every_nth_frame: settings.input.process_every_nth_frame.unwrap_or(2),
            },
            tracking: TrackingView {
                typ: tracking.typ.clone().unwrap_or_else(|| "iou_naive".into()),
                kalman_filter: tracking
                    .kalman_filter
                    .clone()
                    .unwrap_or_else(|| "centroid".into()),
                max_lost_seconds: tracking.max_lost_seconds,
                max_no_match: tracking.max_no_match,
                max_points_in_track: tracking.max_points_in_track,
                iou_threshold: tracking.iou_threshold,
            },
            equipment_info: EquipmentView {
                id: settings.equipment_info.id.clone(),
            },
            worker: WorkerView {
                reset_data_milliseconds: settings.worker.reset_data_milliseconds,
            },
            redis_publisher: RedisView {
                enable: redis.enable,
                host: redis.host.clone(),
                port: redis.port,
                username: redis.username.clone(),
                db_index: redis.db_index,
                channel_name: redis.channel_name.clone(),
                password: redis.password.clone(),
            },
            verbose: VerboseView {
                level: verbose.level,
                logs_folder: verbose.logs_folder,
                max_file_size_mb: verbose.max_file_size_mb,
                max_files: verbose.max_files,
            },
        }
    }
}

/// The settings to change. Everything is optional, and a name this app has no
/// setting for is refused rather than quietly ignored
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigPatch {
    pub input: Option<InputPatch>,
    pub tracking: Option<TrackingPatch>,
    pub equipment_info: Option<EquipmentPatch>,
    pub worker: Option<WorkerPatch>,
    pub redis_publisher: Option<RedisPatch>,
    pub verbose: Option<VerbosePatch>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InputPatch {
    pub video_src: Option<String>,
    pub process_every_nth_frame: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TrackingPatch {
    #[serde(rename = "type")]
    pub typ: Option<String>,
    pub kalman_filter: Option<String>,
    pub max_lost_seconds: Option<f32>,
    pub max_no_match: Option<usize>,
    pub max_points_in_track: Option<usize>,
    pub iou_threshold: Option<f32>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EquipmentPatch {
    pub id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkerPatch {
    pub reset_data_milliseconds: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RedisPatch {
    pub enable: Option<bool>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub username: Option<String>,
    pub db_index: Option<i32>,
    pub channel_name: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VerbosePatch {
    pub level: Option<String>,
    pub logs_folder: Option<String>,
    pub max_file_size_mb: Option<u64>,
    pub max_files: Option<usize>,
}

/// Sets a value and names the path, but only when it really differs: the
/// answer then says what changed rather than what was sent
fn set<T: PartialEq>(target: &mut T, value: Option<T>, path: &str, changed: &mut Vec<String>) {
    if let Some(value) = value {
        if *target != value {
            *target = value;
            changed.push(path.to_string());
        }
    }
}

impl ConfigPatch {
    /// Applies the patch, returning the dotted paths that actually changed
    pub fn apply(self, settings: &mut AppSettings) -> Vec<String> {
        let mut changed = Vec::new();
        if let Some(input) = self.input {
            set(
                &mut settings.input.video_src,
                input.video_src,
                "input.video_src",
                &mut changed,
            );
            set(
                &mut settings.input.process_every_nth_frame,
                input.process_every_nth_frame.map(Some),
                "input.process_every_nth_frame",
                &mut changed,
            );
        }
        if let Some(tracking) = self.tracking {
            set(
                &mut settings.tracking.typ,
                tracking.typ.map(Some),
                "tracking.type",
                &mut changed,
            );
            set(
                &mut settings.tracking.kalman_filter,
                tracking.kalman_filter.map(Some),
                "tracking.kalman_filter",
                &mut changed,
            );
            set(
                &mut settings.tracking.max_lost_seconds,
                tracking.max_lost_seconds.map(Some),
                "tracking.max_lost_seconds",
                &mut changed,
            );
            set(
                &mut settings.tracking.max_no_match,
                tracking.max_no_match.map(Some),
                "tracking.max_no_match",
                &mut changed,
            );
            set(
                &mut settings.tracking.max_points_in_track,
                tracking.max_points_in_track,
                "tracking.max_points_in_track",
                &mut changed,
            );
            set(
                &mut settings.tracking.iou_threshold,
                tracking.iou_threshold.map(Some),
                "tracking.iou_threshold",
                &mut changed,
            );
        }
        if let Some(equipment) = self.equipment_info {
            set(
                &mut settings.equipment_info.id,
                equipment.id,
                "equipment_info.id",
                &mut changed,
            );
        }
        if let Some(worker) = self.worker {
            set(
                &mut settings.worker.reset_data_milliseconds,
                worker.reset_data_milliseconds,
                "worker.reset_data_milliseconds",
                &mut changed,
            );
        }
        if let Some(redis) = self.redis_publisher {
            let current = &mut settings.redis_publisher;
            set(
                &mut current.enable,
                redis.enable,
                "redis_publisher.enable",
                &mut changed,
            );
            set(
                &mut current.host,
                redis.host,
                "redis_publisher.host",
                &mut changed,
            );
            set(
                &mut current.port,
                redis.port,
                "redis_publisher.port",
                &mut changed,
            );
            set(
                &mut current.username,
                redis.username.map(Some),
                "redis_publisher.username",
                &mut changed,
            );
            set(
                &mut current.db_index,
                redis.db_index,
                "redis_publisher.db_index",
                &mut changed,
            );
            set(
                &mut current.channel_name,
                redis.channel_name,
                "redis_publisher.channel_name",
                &mut changed,
            );
            set(
                &mut current.password,
                redis.password,
                "redis_publisher.password",
                &mut changed,
            );
        }
        if let Some(verbose) = self.verbose {
            let current = settings.verbose.get_or_insert_with(Default::default);
            set(
                &mut current.level,
                verbose.level.map(Some),
                "verbose.level",
                &mut changed,
            );
            set(
                &mut current.logs_folder,
                verbose.logs_folder.map(Some),
                "verbose.logs_folder",
                &mut changed,
            );
            set(
                &mut current.max_file_size_mb,
                verbose.max_file_size_mb.map(Some),
                "verbose.max_file_size_mb",
                &mut changed,
            );
            set(
                &mut current.max_files,
                verbose.max_files.map(Some),
                "verbose.max_files",
                &mut changed,
            );
        }
        changed
    }
}

/// What `tracking.type` and `tracking.kalman_filter` accept
#[derive(Debug, Serialize, ToSchema)]
pub struct TrackingOptions {
    /// Trackers this build can run
    #[schema(example = json!(["iou_naive", "bytetrack"]))]
    pub tracker_types: Vec<&'static str>,
    /// Kalman filters a tracker can be given
    #[schema(example = json!(["centroid", "bbox"]))]
    pub kalman_filters: Vec<&'static str>,
}

#[utoipa::path(
    get,
    tag = "Configuration",
    path = "/api/tracking/types",
    responses(
        (status = 200, description = "The values tracking settings accept", body = TrackingOptions)
    )
)]
/// The choices behind `tracking.type` and `tracking.kalman_filter`, so that a
/// caller offers what this build actually supports instead of a list of its own
pub async fn tracking_options() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().json(TrackingOptions {
        tracker_types: TRACKER_TYPES.to_vec(),
        kalman_filters: KALMAN_FILTERS.to_vec(),
    }))
}

#[utoipa::path(
    get,
    tag = "Configuration",
    path = "/api/config",
    responses(
        (status = 200, description = "The configuration as it is saved", body = ConfigView)
    )
)]
/// The settings this application can be told about. The zones have their own
/// endpoints, the Redis password is reported only as `password_set`, and the
/// model is not part of this at all
pub async fn get_config(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    let settings = data
        .app_settings
        .read()
        .expect("Settings are poisoned [RwLock]");
    Ok(HttpResponse::Ok().json(ConfigView::from(&*settings)))
}

#[utoipa::path(
    put,
    tag = "Configuration",
    path = "/api/config",
    request_body = ConfigPatch,
    responses(
        (status = 200, description = "Changed in memory; `save_toml` writes it", body = UpdateConfigResponse),
        (status = 400, description = "The request asks for something the app would not accept", body = ErrorResponse)
    )
)]
/// Changes the settings held in memory. Nothing is written here:
/// `GET /api/mutations/save_toml` is the one thing that writes the file, and a
/// restart reads the file. Only the keys present in the request are touched;
/// the answer names what this request changed and what is left to do overall
pub async fn update_config(
    data: web::Data<APIStorage>,
    patch: web::Json<ConfigPatch>,
) -> Result<HttpResponse, Error> {
    let mut settings = data
        .app_settings
        .write()
        .expect("Settings are poisoned [RwLock]");

    let mut updated = settings.clone();
    let changed = patch.into_inner().apply(&mut updated);
    if changed.is_empty() {
        drop(settings);
        return Ok(HttpResponse::Ok().json(UpdateConfigResponse {
            message: "ok",
            changed,
            state: ChangeState::of(&data),
        }));
    }
    if let Err(err) = updated.validate() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse {
            error_text: err.to_string(),
        }));
    }
    *settings = updated;

    // Whatever can be applied right away, is: the rest waits for a restart
    if changed.iter().any(|path| path == "verbose.level") {
        if let Some(level) = settings.verbose.as_ref().and_then(|v| v.level.as_deref()) {
            logging::set_log_level(level);
        }
    }
    if changed.iter().any(|path| path == "equipment_info.id") {
        // Read when statistics are handed out rather than kept anywhere else,
        // so writing it here is all it takes
        data.data_storage
            .write()
            .expect("DataStorage is poisoned [RWLock]")
            .id = settings.equipment_info.id.clone();
    }
    info!(
        scope = logging::SCOPE_REST_API,
        changed = ?changed,
        "Configuration changed in memory, not saved yet"
    );
    drop(settings);
    Ok(HttpResponse::Ok().json(UpdateConfigResponse {
        message: "ok",
        changed,
        state: ChangeState::of(&data),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::needs_restart;

    fn patch(text: &str) -> Result<ConfigPatch, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// A named file per test: they run in parallel and would otherwise delete
    /// each other's
    fn settings(name: &str) -> AppSettings {
        let path =
            std::env::temp_dir().join(format!("rrt_cfg_{}_{}.toml", name, std::process::id()));
        std::fs::write(
            &path,
            r#"
[input]
    video_src = "a.mp4"
[detection]
    network_weights = "m.onnx"
    conf_threshold = 0.4
    nms_threshold = 0.2
    net_classes = ["car", "bus"]
[tracking]
    type = "iou_naive"
    max_points_in_track = 100
[equipment_info]
    id = "eq-1"
[worker]
    reset_data_milliseconds = 30000
[rest_api]
    enable = true
    host = "0.0.0.0"
    back_end_port = 42001
    api_scope = "/api"
[redis_publisher]
    enable = false
    host = "localhost"
    port = 6379
    password = "secret"
    db_index = 0
    channel_name = "X"
"#,
        )
        .unwrap();
        let settings = AppSettings::new(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        settings
    }

    #[test]
    fn only_the_given_keys_are_touched_and_only_real_changes_are_named() {
        let mut s = settings("touched");
        let changed = patch(
            r#"{"input":{"video_src":"rtsp://cam"},"worker":{"reset_data_milliseconds":30000}}"#,
        )
        .unwrap()
        .apply(&mut s);
        // The worker interval was sent, but it was already that
        assert_eq!(changed, ["input.video_src"]);
        assert_eq!(s.input.video_src, "rtsp://cam");
        assert_eq!(s.worker.reset_data_milliseconds, 30000);
    }

    #[test]
    fn a_missing_section_is_created_when_it_is_written_to() {
        let mut s = settings("section");
        assert!(s.verbose.is_none());
        let changed = patch(r#"{"verbose":{"level":"debug"}}"#)
            .unwrap()
            .apply(&mut s);
        assert_eq!(changed, ["verbose.level"]);
        assert_eq!(s.verbose.unwrap().level.as_deref(), Some("debug"));
    }

    #[test]
    fn detection_and_the_server_itself_are_not_part_of_this_api() {
        // How the device detects is decided when it is built, and the app
        // cannot rebind its own port while answering on it
        for body in [
            r#"{"detection":{"network_weights":"other.onnx"}}"#,
            r#"{"detection":{"conf_threshold":0.5}}"#,
            r#"{"rest_api":{"back_end_port":8080}}"#,
        ] {
            let error = patch(body).expect_err("not part of this API");
            assert!(
                error.to_string().contains("unknown field"),
                "{body}: {error}"
            );
        }
    }

    #[test]
    fn a_misspelled_setting_is_refused() {
        let error = patch(r#"{"tracking":{"kalman_filtr":"bbox"}}"#).expect_err("typo");
        assert!(error.to_string().contains("unknown field"), "{error}");
        let error = patch(r#"{"tracing":{"level":"debug"}}"#).expect_err("typo in a section");
        assert!(error.to_string().contains("unknown field"), "{error}");
    }

    #[test]
    fn the_whole_redis_section_travels_both_ways() {
        let mut s = settings("password");
        // The password is handed out as it is: whoever reaches this API can
        // already change it and restart the app
        assert_eq!(ConfigView::from(&s).redis_publisher.password, "secret");

        let changed = patch(r#"{"redis_publisher":{"password":"new","username":"stats"}}"#)
            .unwrap()
            .apply(&mut s);
        assert_eq!(
            changed,
            ["redis_publisher.username", "redis_publisher.password"]
        );
        assert_eq!(s.redis_publisher.password, "new");
        assert_eq!(s.redis_publisher.username.as_deref(), Some("stats"));
    }

    #[test]
    fn the_view_resolves_what_the_file_left_out() {
        let s = settings("view");
        let view = ConfigView::from(&s);
        // Defaults the app would use anyway, rather than nulls the UI has to guess about
        assert_eq!(view.input.process_every_nth_frame, 2);
        assert_eq!(view.tracking.typ, "iou_naive");
        assert_eq!(view.tracking.kalman_filter, "centroid");
    }

    #[test]
    fn only_the_log_level_and_the_equipment_id_take_effect_at_once() {
        for path in [
            "input.video_src",
            "tracking.type",
            "worker.reset_data_milliseconds",
            "redis_publisher.host",
            "verbose.logs_folder",
        ] {
            assert!(needs_restart(path), "{path} is read once, at startup");
        }
        assert!(!needs_restart("verbose.level"));
        assert!(!needs_restart("equipment_info.id"));
    }
}
