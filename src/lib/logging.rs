//! NDJSON logging to stdout and to a rotated file, configured by `[verbose]`
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::{info, warn};
use tracing_subscriber::{
    EnvFilter, Registry, fmt, layer::SubscriberExt, reload, util::SubscriberInitExt,
};

pub const LOG_FILE_NAME: &str = "rust-road-traffic.log";

/// `scope` field of every log line: which part of the app is talking
pub const SCOPE_STARTUP: &str = "startup";
pub const SCOPE_CAPTURE: &str = "capture";
pub const SCOPE_PROCESSING: &str = "processing";
pub const SCOPE_ANALYTICS: &str = "analytics";
pub const SCOPE_REDIS: &str = "redis";
pub const SCOPE_REST_API: &str = "rest_api";
pub const SCOPE_REPORT: &str = "report";
pub const SCOPE_DATASET: &str = "dataset";
/// Verbosity thresholds: "info" shows error, warn and info lines, "debug" adds debug
pub const LEVELS: [&str; 2] = ["info", "debug"];
pub const DEFAULT_LEVEL: &str = "info";
pub const DEFAULT_MAX_FILE_SIZE_MB: u64 = 50;
pub const DEFAULT_MAX_FILES: usize = 9;

static RELOAD_HANDLE: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

/// Resolved logging configuration, see `VerboseSettings`
pub struct LogConfig {
    /// One of `LEVELS`
    pub level: &'static str,
    /// The level as written in the config when it was not one of `LEVELS`
    pub unknown_level: Option<String>,
    /// Folder for the log file, `None` for stdout only
    pub logs_folder: Option<PathBuf>,
    pub max_file_size_bytes: u64,
    pub max_files: usize,
}

/// Sets up logging: NDJSON lines to stdout and, when a usable folder is
/// configured, to `<logs_folder>/rust-road-traffic.log` rotated daily or at
/// `max_file_size_bytes` with `max_files` old files kept. The level applies to
/// this crate, dependencies log at warn and above; `RUST_LOG` overrides both.
/// A folder that cannot be used never stops the app: it logs to stdout only
/// and says so. The returned guard must live until the end of main: dropping
/// it flushes the file writer
#[must_use]
pub fn init_logger(config: &LogConfig) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(filter_directive(config.level)));
    let (filter_layer, handle) = reload::Layer::new(filter);
    let _ = RELOAD_HANDLE.set(handle);

    let mut guard = None;
    let mut file_layer = None;
    let mut folder_problem = None;
    let mut log_file = None;
    if let Some(folder) = &config.logs_folder {
        match open_log_file(folder, config) {
            Ok(appender) => {
                let (writer, worker_guard) = tracing_appender::non_blocking(appender);
                guard = Some(worker_guard);
                log_file = Some(folder.join(LOG_FILE_NAME));
                file_layer = Some(
                    fmt::layer()
                        .json()
                        .with_target(false)
                        .with_thread_ids(false)
                        .with_thread_names(false)
                        .with_file(false)
                        .with_line_number(false)
                        .with_ansi(false)
                        .with_writer(writer),
                );
            }
            Err(problem) => folder_problem = Some(problem),
        }
    }

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(
            fmt::layer()
                .json()
                .with_target(false)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_ansi(false)
                .with_writer(std::io::stdout),
        )
        .with(file_layer)
        .init();

    if let Some(level) = &config.unknown_level {
        warn!(
            scope = SCOPE_STARTUP,
            level,
            using = config.level,
            "Unknown verbose level, expected one of {:?}",
            LEVELS
        );
    }
    match (&config.logs_folder, folder_problem) {
        (Some(folder), Some(problem)) => warn!(
            scope = SCOPE_STARTUP,
            folder = %folder.display(),
            problem,
            "Can't write log files there, logging to stdout only"
        ),
        _ => {}
    }
    info!(
        scope = SCOPE_STARTUP,
        level = config.level,
        file = log_file.as_ref().map(|p| p.display().to_string()),
        max_file_size_mb = config.max_file_size_bytes / (1024 * 1024),
        max_files = config.max_files,
        "Logging initialized"
    );
    guard
}

/// Makes sure the folder exists, is a directory and the log file in it can be
/// opened for writing, then builds the rotating appender
fn open_log_file(
    folder: &Path,
    config: &LogConfig,
) -> Result<rolling_file::BasicRollingFileAppender, String> {
    std::fs::create_dir_all(folder).map_err(|e| format!("can't create folder: {e}"))?;
    if !folder.is_dir() {
        return Err("not a directory".to_string());
    }
    let file = folder.join(LOG_FILE_NAME);
    std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&file)
        .map_err(|e| format!("can't open {}: {e}", file.display()))?;
    rolling_file::BasicRollingFileAppender::new(
        &file,
        rolling_file::RollingConditionBasic::new()
            .daily()
            .max_size(config.max_file_size_bytes),
        config.max_files,
    )
    .map_err(|e| format!("can't set up rotation: {e}"))
}

/// Changes the verbosity at runtime. `false` when the level is unknown or the
/// logger is not initialized
pub fn set_log_level(level: &str) -> bool {
    let Some(level) = normalize_level(level) else {
        return false;
    };
    match RELOAD_HANDLE.get() {
        Some(handle) => handle
            .reload(EnvFilter::new(filter_directive(level)))
            .is_ok(),
        None => false,
    }
}

/// Canonical spelling of a level, `None` when it is not one of `LEVELS`
pub fn normalize_level(level: &str) -> Option<&'static str> {
    let level = level.trim().to_ascii_lowercase();
    LEVELS.iter().copied().find(|known| *known == level)
}

fn filter_directive(level: &str) -> String {
    format!("rust_road_traffic={level},warn")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(folder: Option<PathBuf>) -> LogConfig {
        LogConfig {
            level: "info",
            unknown_level: None,
            logs_folder: folder,
            max_file_size_bytes: 1024,
            max_files: 1,
        }
    }

    #[test]
    fn levels_are_normalized() {
        assert_eq!(normalize_level(" Debug "), Some("debug"));
        assert_eq!(normalize_level("info"), Some("info"));
        assert_eq!(normalize_level("error"), None);
        assert_eq!(normalize_level("trace"), None);
    }

    #[test]
    fn set_level_before_init_is_refused() {
        assert!(!set_log_level("nonsense"));
    }

    #[test]
    fn unusable_folders_are_reported_not_fatal() {
        let file_as_folder =
            std::env::temp_dir().join(format!("rrt_log_file_{}", std::process::id()));
        std::fs::write(&file_as_folder, b"x").unwrap();
        let problem = open_log_file(&file_as_folder, &config(None)).unwrap_err();
        assert!(!problem.is_empty());
        let _ = std::fs::remove_file(&file_as_folder);

        let nowhere = PathBuf::from("/dev/null/logs");
        assert!(open_log_file(&nowhere, &config(None)).is_err());
    }

    #[test]
    fn usable_folder_is_created() {
        let folder = std::env::temp_dir().join(format!("rrt_logs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        assert!(open_log_file(&folder, &config(None)).is_ok());
        assert!(folder.join(LOG_FILE_NAME).is_file());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
