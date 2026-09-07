use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use actix_web::{Error, HttpResponse, web};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use utoipa::{IntoParams, ToSchema};

use crate::rest_api::APIStorage;

/// Never read more than this from one file: the tail is what anyone wants, and
/// a log file is allowed to be tens of megabytes
const MAX_SCAN_BYTES: u64 = 1024 * 1024;
const DEFAULT_LIMIT: usize = 200;
const MAX_LIMIT: usize = 5000;

/// One line of the log file
#[derive(Debug, Serialize, ToSchema)]
pub struct LogEntry {
    /// When it was written, RFC 3339
    pub timestamp: Option<String>,
    /// "ERROR", "WARN", "INFO" or "DEBUG"
    pub level: String,
    /// Which part of the app it came from
    pub scope: Option<String>,
    pub message: String,
    /// Whatever else the line carried
    #[schema(value_type = Object)]
    pub fields: Map<String, Value>,
}

/// Filters for the log request
#[derive(Debug, Deserialize, IntoParams)]
pub struct LogsQuery {
    /// How many of the most recent entries to return. Default 200, at most 5000
    pub limit: Option<usize>,
    /// Lowest severity to include: "error", "warn", "info" or "debug"
    pub level: Option<String>,
    /// Only entries from this scope, e.g. "capture" or "analytics"
    pub scope: Option<String>,
}

/// The most recent log entries
#[derive(Debug, Serialize, ToSchema)]
pub struct LogsResponse {
    /// The file they were read from, `null` when the app only logs to stdout
    pub file: Option<String>,
    pub returned: usize,
    /// Oldest first, so that reading them top to bottom follows the run
    pub entries: Vec<LogEntry>,
}

#[utoipa::path(
    get,
    tag = "Application",
    path = "/api/logs",
    params(LogsQuery),
    responses(
        (status = 200, description = "Most recent log entries", body = LogsResponse)
    )
)]
/// Returns the tail of the log file, so that what happened can be looked at
/// without an SSH session. The file rather than an in-memory buffer: it is
/// rotated and kept on disk, and it survives a restart, which is exactly when
/// one wants to see what came before
pub async fn logs(
    data: web::Data<APIStorage>,
    query: web::Query<LogsQuery>,
) -> Result<HttpResponse, Error> {
    let log_config = data
        .app_settings
        .read()
        .expect("Settings are poisoned [RwLock]")
        .verbose
        .clone()
        .unwrap_or_default()
        .to_log_config(&data.settings_filename);
    let Some(folder) = log_config.logs_folder else {
        return Ok(HttpResponse::Ok().json(LogsResponse {
            file: None,
            returned: 0,
            entries: Vec::new(),
        }));
    };
    let file = folder.join(crate::lib::logging::LOG_FILE_NAME);
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let min_rank = query.level.as_deref().and_then(severity_rank);
    let entries = collect(
        &file,
        log_config.max_files,
        limit,
        min_rank,
        query.scope.as_deref(),
    );
    Ok(HttpResponse::Ok().json(LogsResponse {
        file: Some(file.display().to_string()),
        returned: entries.len(),
        entries,
    }))
}

/// Walks the current log file and then the rotated ones, newest first, until
/// enough matching entries are found; returns them oldest first
fn collect(
    file: &Path,
    max_files: usize,
    limit: usize,
    min_rank: Option<u8>,
    scope: Option<&str>,
) -> Vec<LogEntry> {
    let mut collected: Vec<LogEntry> = Vec::new();
    for index in 0..=max_files {
        let path = if index == 0 {
            file.to_path_buf()
        } else {
            // rolling-file keeps the previous files as `<name>.1`, `<name>.2`, ...
            PathBuf::from(format!("{}.{}", file.display(), index))
        };
        let Ok(text) = read_tail(&path, MAX_SCAN_BYTES) else {
            continue;
        };
        let mut matching: Vec<LogEntry> = text
            .lines()
            .filter_map(parse_entry)
            .filter(|entry| matches(entry, min_rank, scope))
            .collect();
        matching.reverse();
        collected.append(&mut matching);
        if collected.len() >= limit {
            break;
        }
    }
    collected.truncate(limit);
    collected.reverse();
    collected
}

/// Reads at most `max_bytes` from the end of the file, dropping the first line
/// when it starts in the middle of one
fn read_tail(path: &Path, max_bytes: u64) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let from = length.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(from))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if from == 0 {
        return Ok(text);
    }
    Ok(match text.find('\n') {
        Some(newline) => text[newline + 1..].to_string(),
        None => String::new(),
    })
}

/// Turns a log line into an entry, pulling the message and the scope out of the
/// fields and leaving the rest of them alone. A line that is not our own JSON
/// is skipped
fn parse_entry(line: &str) -> Option<LogEntry> {
    let mut value: Value = serde_json::from_str(line).ok()?;
    let object = value.as_object_mut()?;
    let timestamp = object
        .get("timestamp")
        .and_then(Value::as_str)
        .map(str::to_string);
    let level = object.get("level").and_then(Value::as_str)?.to_string();
    let mut fields = match object.remove("fields") {
        Some(Value::Object(fields)) => fields,
        _ => Map::new(),
    };
    let message = fields
        .remove("message")
        .and_then(|message| message.as_str().map(str::to_string))
        .unwrap_or_default();
    let scope = fields
        .remove("scope")
        .and_then(|scope| scope.as_str().map(str::to_string));
    Some(LogEntry {
        timestamp,
        level,
        scope,
        message,
        fields,
    })
}

fn matches(entry: &LogEntry, min_rank: Option<u8>, scope: Option<&str>) -> bool {
    if let Some(min) = min_rank {
        if severity_rank(&entry.level).unwrap_or(0) < min {
            return false;
        }
    }
    match scope {
        Some(wanted) => entry.scope.as_deref() == Some(wanted),
        None => true,
    }
}

/// How severe a level is, so that "warn" can mean "warnings and worse"
fn severity_rank(level: &str) -> Option<u8> {
    match level.trim().to_ascii_lowercase().as_str() {
        "debug" | "trace" => Some(0),
        "info" => Some(1),
        "warn" | "warning" => Some(2),
        "error" => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn line(level: &str, scope: &str, message: &str) -> String {
        format!(
            r#"{{"timestamp":"2026-09-07T19:00:00Z","level":"{level}","fields":{{"message":"{message}","scope":"{scope}","extra":7}}}}"#
        )
    }

    fn write_log(path: &Path, lines: &[String]) {
        let mut file = File::create(path).unwrap();
        for l in lines {
            writeln!(file, "{l}").unwrap();
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("rrt_logs_{}_{}", name, std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn newest_entries_come_last_and_fields_are_kept() {
        let dir = TempDir::new("tail");
        let file = dir.0.join("app.log");
        write_log(
            &file,
            &[
                line("INFO", "startup", "one"),
                line("WARN", "capture", "two"),
                line("ERROR", "capture", "three"),
            ],
        );
        let entries = collect(&file, 2, 10, None, None);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            ["one", "two", "three"]
        );
        assert_eq!(entries[0].scope.as_deref(), Some("startup"));
        assert_eq!(
            entries[0].fields.get("extra").and_then(Value::as_i64),
            Some(7)
        );
    }

    #[test]
    fn limit_keeps_the_most_recent() {
        let dir = TempDir::new("limit");
        let file = dir.0.join("app.log");
        let lines: Vec<String> = (0..10)
            .map(|i| line("INFO", "processing", &format!("m{i}")))
            .collect();
        write_log(&file, &lines);
        let entries = collect(&file, 2, 3, None, None);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            ["m7", "m8", "m9"]
        );
    }

    #[test]
    fn level_and_scope_filter() {
        let dir = TempDir::new("filter");
        let file = dir.0.join("app.log");
        write_log(
            &file,
            &[
                line("INFO", "capture", "info-capture"),
                line("WARN", "capture", "warn-capture"),
                line("ERROR", "redis", "error-redis"),
            ],
        );
        let warnings = collect(&file, 2, 10, severity_rank("warn"), None);
        assert_eq!(
            warnings
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            ["warn-capture", "error-redis"]
        );
        let capture = collect(&file, 2, 10, None, Some("capture"));
        assert_eq!(
            capture
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            ["info-capture", "warn-capture"]
        );
    }

    #[test]
    fn rotated_files_fill_the_tail_and_missing_ones_are_skipped() {
        let dir = TempDir::new("rotated");
        let file = dir.0.join("app.log");
        write_log(&file, &[line("INFO", "processing", "newest")]);
        write_log(
            &PathBuf::from(format!("{}.2", file.display())),
            &[line("INFO", "processing", "oldest")],
        );
        // `.1` is deliberately absent
        let entries = collect(&file, 3, 10, None, None);
        assert_eq!(
            entries
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>(),
            ["oldest", "newest"]
        );
    }

    #[test]
    fn a_partial_first_line_is_dropped() {
        let dir = TempDir::new("partial");
        let file = dir.0.join("app.log");
        write_log(
            &file,
            &[
                line("INFO", "startup", "first"),
                line("INFO", "startup", "second"),
            ],
        );
        // Starts 20 bytes in, i.e. in the middle of the first line
        let length = std::fs::metadata(&file).unwrap().len();
        let text = read_tail(&file, length - 20).unwrap();
        assert!(!text.contains("first"), "{text}");
        assert!(text.contains("second"), "{text}");
    }

    #[test]
    fn lines_that_are_not_ours_are_skipped() {
        let dir = TempDir::new("garbage");
        let file = dir.0.join("app.log");
        write_log(
            &file,
            &[
                "not json at all".to_string(),
                r#"{"level":"INFO"}"#.to_string(),
                line("INFO", "startup", "good"),
            ],
        );
        let entries = collect(&file, 1, 10, None, None);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].message, "good");
    }
}
