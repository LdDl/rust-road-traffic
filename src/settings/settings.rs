use std::fs;
use std::io;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use toml;
use toml_edit::{ArrayOfTables, Decor, DocumentMut, Item, Table, Value};

#[derive(Debug)]
pub enum SettingsError {
    Io(io::Error),
    Parse(toml::de::Error),
    Validation(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::Io(e) => write!(f, "I/O error: {}", e),
            SettingsError::Parse(e) => write!(f, "TOML parse error: {}", e),
            SettingsError::Validation(msg) => write!(f, "Validation error: {}", msg),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<io::Error> for SettingsError {
    fn from(e: io::Error) -> Self {
        SettingsError::Io(e)
    }
}

impl From<toml::de::Error> for SettingsError {
    fn from(e: toml::de::Error) -> Self {
        SettingsError::Parse(e)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppSettings {
    pub input: InputSettings,
    pub debug: Option<DebugSettings>,
    pub detection: DetectionSettings,
    pub tracking: TrackingSettings,
    pub equipment_info: EquipmentInfo,
    pub road_lanes: Option<Vec<RoadLanesSettings>>,
    pub worker: WorkerSettings,
    pub rest_api: RestAPISettings,
    pub redis_publisher: RedisPublisherSettings,
    pub dataset_collector: Option<DatasetCollectorSettings>,
    pub report: Option<ReportSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InputSettings {
    pub video_src: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DebugSettings {
    pub enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DetectionSettings {
    pub network_weights: String,
    pub network_cfg: Option<String>,
    pub conf_threshold: f32,
    pub nms_threshold: f32,
    pub net_width: Option<i32>,
    pub net_height: Option<i32>,
    pub net_classes: Vec<String>,
    pub target_classes: Option<Vec<String>>,
    /// Inference backend: "ort" for ONNX Runtime, "opencv" for OpenCV DNN.
    /// Default is "opencv".
    pub inference_backend: Option<String>,
    /// Print performance stats every N frames. 0 = disabled.
    #[serde(default)]
    pub perf_stats_interval: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TrackingSettings {
    // Either "bytetrack" or "iou_naive". Default is "iou_naive".
    // `typ` is what earlier versions wrote when saving from the UI
    #[serde(rename = "type", alias = "typ")]
    pub typ: Option<String>,
    pub max_points_in_track: usize,
    // Either "centroid" or "bbox". Default is "centroid"
    pub kalman_filter: Option<String>,
    // Maximum number of frames to keep tracking an object without new detections. Default is 60
    pub max_no_match: Option<usize>,
    // IoU threshold for matching detections to tracks. Default is 0.3
    pub iou_threshold: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EquipmentInfo {
    pub id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RoadLanesSettings {
    pub lane_number: u16,
    pub lane_direction: u8,
    pub geometry: Vec<[i32; 2]>,
    pub geometry_wgs84: Vec<[f32; 2]>,
    pub color_rgb: [i16; 3],
    pub virtual_line: Option<VirtualLineSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VirtualLineSettings {
    pub geometry: [[i32; 2]; 2],
    pub color_rgb: [i16; 3],
    // 'inbound' - inbound traffic (towards target side)
    // 'outbound' - outbound traffic (away from target side)
    pub direction: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkerSettings {
    pub reset_data_milliseconds: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RestAPISettings {
    pub enable: bool,
    pub host: String,
    pub back_end_port: i32,
    pub api_scope: String,
    pub mjpeg_streaming: Option<MJPEGStreamingSettings>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RedisPublisherSettings {
    pub enable: bool,
    pub host: String,
    pub port: i32,
    pub password: String,
    pub db_index: i32,
    pub channel_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MJPEGStreamingSettings {
    pub enable: bool,
    /// JPEG quality for streaming (0-100). Lower = smaller files, faster streaming.
    /// Default is 80
    #[serde(default = "default_mjpeg_quality")]
    pub quality: i32,
}

fn default_mjpeg_quality() -> i32 {
    80
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DatasetCollectorSettings {
    pub enabled: bool,
    pub output_dir: String,
    /// Label format: "yolo" for standard YOLO format (class_id center_x center_y width height)
    #[serde(default = "default_label_format")]
    pub label_format: String,
    /// Minimum number of frames a track must exist before capturing
    #[serde(default = "default_min_track_age")]
    pub min_track_age: u32,
    /// Skip objects whose bounding box touches frame edges
    #[serde(default = "default_skip_edge_objects")]
    pub skip_edge_objects: bool,
    /// Margin in pixels to consider as "edge"
    #[serde(default = "default_edge_margin_pixels")]
    pub edge_margin_pixels: u32,
    /// Maximum number of captures per unique track ID
    #[serde(default = "default_max_captures_per_track")]
    pub max_captures_per_track: u32,
    /// Frames between captures for the same track (when max_captures_per_track > 1)
    #[serde(default = "default_capture_interval")]
    pub capture_interval: u32,
}

fn default_label_format() -> String {
    "yolo".to_string()
}
fn default_min_track_age() -> u32 {
    15
}
fn default_skip_edge_objects() -> bool {
    true
}
fn default_edge_margin_pixels() -> u32 {
    5
}
fn default_max_captures_per_track() -> u32 {
    1
}
fn default_capture_interval() -> u32 {
    30
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReportSettings {
    pub enabled: bool,
    pub output_path: String,
}

use crate::lib::cv::Scalar;
use crate::lib::spatial::Point2f;
use crate::lib::spatial::epsg::lonlat_to_meters;
use crate::lib::zones::Zone;
use crate::lib::zones::{VirtualLine, VirtualLineDirection};
use std::convert::From;

/// Sets every value of `fresh` into `existing`, keeping the comments and
/// formatting attached to the keys that are already there. Tables new to the
/// document go after their parent (`nested`) or after everything (`next_position`);
/// tables parsed from the fresh dump carry positions of their own, which would
/// otherwise interleave them with the document's. With `prune`, keys that
/// `fresh` no longer has are removed: that is for generated content only
fn merge_tables(
    existing: &mut Table,
    fresh: Table,
    next_position: &mut isize,
    nested: Option<isize>,
    prune: bool,
) {
    if prune {
        let stale: Vec<String> = existing
            .iter()
            .filter(|(key, _)| !fresh.contains_key(key))
            .map(|(key, _)| key.to_string())
            .collect();
        for key in stale {
            existing.remove(&key);
        }
    }
    for (key, item) in fresh {
        match item {
            Item::Value(value) => merge_value(existing, &key, value),
            Item::Table(mut table) => match existing.get_mut(&key) {
                Some(Item::Table(target)) => {
                    let position = target.position();
                    merge_tables(target, table, next_position, position, prune)
                }
                _ => {
                    let position = nested.unwrap_or_else(|| {
                        *next_position += 1;
                        *next_position - 1
                    });
                    set_positions(&mut table, position);
                    // Header indented like the keys around it, its keys one level deeper
                    let indent = first_key_indent(existing);
                    table.decor_mut().set_prefix(indent.clone());
                    indent_keys(&mut table, &format!("{indent}{indent}"));
                    existing.insert(&key, Item::Table(table));
                }
            },
            Item::ArrayOfTables(array) => {
                merge_array_of_tables(existing, &key, array, next_position)
            }
            Item::None => {}
        }
    }
}

/// Zones are generated by the UI, so at the same index a fresh table can only
/// differ from the document's one by its values: those are set in place, with
/// the comments and indentation around them kept and keys that are gone
/// removed. Extra fresh tables are appended in the same style, extra old ones
/// dropped
fn merge_array_of_tables(
    existing: &mut Table,
    key: &str,
    fresh: ArrayOfTables,
    next_position: &mut isize,
) {
    if let Some(Item::ArrayOfTables(current)) = existing.get_mut(key) {
        let position = current.iter().find_map(Table::position);
        let indent = current
            .iter()
            .next()
            .map(first_key_indent)
            .unwrap_or_default();
        let header = current.iter().last().map(|table| table.decor().clone());
        let fresh: Vec<Table> = fresh.into_iter().collect();
        while current.len() > fresh.len() {
            current.remove(current.len() - 1);
        }
        for (index, mut table) in fresh.into_iter().enumerate() {
            match current.get_mut(index) {
                Some(target) => {
                    let position = target.position();
                    merge_tables(target, table, next_position, position, true)
                }
                None => {
                    set_positions(&mut table, position.unwrap_or(*next_position));
                    indent_keys(&mut table, &indent);
                    if let Some(header) = &header {
                        *table.decor_mut() = header.clone();
                    }
                    current.push(table);
                }
            }
        }
        return;
    }
    let mut array = fresh;
    for table in array.iter_mut() {
        set_positions(table, *next_position);
    }
    *next_position += 1;
    existing.insert(key, Item::ArrayOfTables(array));
}

fn merge_value(existing: &mut Table, key: &str, mut value: Value) {
    match existing.get_mut(key) {
        Some(Item::Value(current)) => {
            if !same_value(current, &value) {
                // Keep the spacing and trailing comment of the old value
                *value.decor_mut() = current.decor().clone();
                *current = value;
            }
        }
        // An empty list of zones serializes as `road_lanes = []`; dropping the
        // `[[road_lanes]]` tables reads back the same
        Some(slot) if slot.is_array_of_tables() => {
            if value.as_array().map_or(false, |array| array.is_empty()) {
                existing.remove(key);
            } else {
                existing.insert(key, Item::Value(value));
            }
        }
        Some(slot) => *slot = Item::Value(value),
        None => {
            // A new key goes last in its section, indented like its neighbours
            let indent = existing
                .iter()
                .last()
                .and_then(|(k, _)| existing.key(k))
                .map(|k| indentation_of(k.leaf_decor()))
                .unwrap_or_default();
            existing.insert(key, Item::Value(value));
            if let Some(mut inserted) = existing.key_mut(key) {
                inserted.leaf_decor_mut().set_prefix(indent);
            }
        }
    }
}

/// Largest table position in the document, 0 when there are no tables
fn max_position(table: &Table) -> isize {
    table
        .iter()
        .map(|(_, item)| match item {
            Item::Table(t) => t.position().unwrap_or(0).max(max_position(t)),
            Item::ArrayOfTables(a) => a
                .iter()
                .map(|t| t.position().unwrap_or(0).max(max_position(t)))
                .max()
                .unwrap_or(0),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}

/// Gives a table and all tables nested in it one position: they render
/// together, in insertion order, at that point of the document
fn set_positions(table: &mut Table, position: isize) {
    table.set_position(position);
    for (_, item) in table.iter_mut() {
        match item {
            Item::Table(t) => set_positions(t, position),
            Item::ArrayOfTables(a) => a.iter_mut().for_each(|t| set_positions(t, position)),
            _ => {}
        }
    }
}

/// Indents the keys of a generated table like the hand-written ones around it;
/// a nested table gets its header at that indent and its keys one level deeper
/// (the header's own indent lives in the table decor, not in its key)
fn indent_keys(table: &mut Table, indent: &str) {
    for (mut key, item) in table.iter_mut() {
        match item {
            Item::Table(nested) => {
                nested.decor_mut().set_prefix(indent);
                indent_keys(nested, &format!("{indent}{indent}"));
            }
            _ => key.leaf_decor_mut().set_prefix(indent),
        }
    }
}

/// Indentation of the first key of a table, "" when it has none
fn first_key_indent(table: &Table) -> String {
    table
        .iter()
        .find(|(_, item)| item.is_value())
        .and_then(|(k, _)| table.key(k))
        .map(|k| indentation_of(k.leaf_decor()))
        .unwrap_or_default()
}

/// Whitespace on the key's own line, without the comment lines above it
fn indentation_of(decor: &Decor) -> String {
    decor
        .prefix()
        .and_then(|p| p.as_str())
        .and_then(|p| p.rsplit('\n').next())
        .filter(|tail| tail.chars().all(char::is_whitespace))
        .unwrap_or("")
        .to_string()
}

/// Whether two values mean the same thing, so that an unchanged value is not
/// rewritten (and an integer written by hand is not turned into a float)
fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(x), Value::String(y)) => x.value() == y.value(),
        (Value::Integer(x), Value::Integer(y)) => x.value() == y.value(),
        (Value::Boolean(x), Value::Boolean(y)) => x.value() == y.value(),
        (Value::Datetime(x), Value::Datetime(y)) => x.value() == y.value(),
        (Value::Float(x), Value::Float(y)) => same_float(*x.value(), *y.value()),
        (Value::Integer(x), Value::Float(y)) | (Value::Float(y), Value::Integer(x)) => {
            same_float(*x.value() as f64, *y.value())
        }
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| same_value(p, q))
        }
        (Value::InlineTable(x), Value::InlineTable(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, p)| y.get(k).map_or(false, |q| same_value(p, q)))
        }
        _ => false,
    }
}

/// Settings are f32 in memory, the file holds f64 text: compare at f32 precision
fn same_float(a: f64, b: f64) -> bool {
    (a - b).abs() <= f32::EPSILON as f64 * a.abs().max(1.0)
}

impl From<&RoadLanesSettings> for Zone {
    fn from(setting: &RoadLanesSettings) -> Self {
        let geom = setting
            .geometry
            .iter()
            .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
            .collect();

        let geom_epsg4326 = setting
            .geometry_wgs84
            .iter()
            .map(|pt| Point2f::new(pt[0], pt[1]))
            .collect();

        let geom_epsg3857 = setting
            .geometry_wgs84
            .iter()
            .map(|pt| {
                let lonlat = lonlat_to_meters(pt[0], pt[1]);
                Point2f::new(lonlat.0, lonlat.1)
            })
            .collect();

        let virtual_line = match &setting.virtual_line {
            Some(vl) => {
                if vl.geometry.len() != 2 {
                    None
                } else {
                    let dir = VirtualLineDirection::from_str(&vl.direction).unwrap_or_default();
                    let a = Point2f::new(vl.geometry[0][0] as f32, vl.geometry[0][1] as f32);
                    let b = Point2f::new(vl.geometry[1][0] as f32, vl.geometry[1][1] as f32);
                    let mut line = VirtualLine::new_from_cv(a, b, dir);
                    line.set_color_rgb(vl.color_rgb[0], vl.color_rgb[1], vl.color_rgb[2]);
                    Some(line)
                }
            }
            None => None,
        };

        Zone::new(
            format!(
                "dir_{}_lane_{}",
                setting.lane_direction, setting.lane_number
            ),
            geom,
            geom_epsg4326,
            geom_epsg3857,
            Scalar::from((
                setting.color_rgb[2] as f64,
                setting.color_rgb[1] as f64,
                setting.color_rgb[0] as f64,
            )),
            setting.lane_number,
            setting.lane_direction,
            virtual_line,
        )
    }
}

impl AppSettings {
    pub fn new(filename: &str) -> Result<Self, SettingsError> {
        let toml_contents = fs::read_to_string(filename)?;
        let mut app_settings = toml::from_str::<AppSettings>(&toml_contents)?;

        // Set default values
        if app_settings.tracking.typ.is_none() {
            app_settings.tracking.typ = Some("iou_naive".to_string());
        }
        if app_settings.tracking.kalman_filter.is_none() {
            app_settings.tracking.kalman_filter = Some("centroid".to_string());
        }

        // Validate tracker type
        if let Some(ref typ) = app_settings.tracking.typ {
            match typ.as_str() {
                "iou_naive" | "bytetrack" => {}
                _ => {
                    return Err(SettingsError::Validation(format!(
                        "Invalid tracker type: '{}'. Supported: 'iou_naive', 'bytetrack'.",
                        typ
                    )));
                }
            }
        }

        // Validate kalman filter type
        if let Some(ref kf) = app_settings.tracking.kalman_filter {
            match kf.as_str() {
                "centroid" | "bbox" => {}
                _ => {
                    return Err(SettingsError::Validation(format!(
                        "Invalid kalman filter type: '{}'. Supported: 'centroid', 'bbox'.",
                        kf
                    )));
                }
            }
        }

        if app_settings.debug.is_none() {
            app_settings.debug = Some(DebugSettings { enable: false });
        }

        Ok(app_settings)
    }
    /// Writes the settings back to `filename` keeping the file's comments, key
    /// order and formatting: every value this struct carries is set in the
    /// existing document, anything else in the file is left as is, and values
    /// that did not change are not touched at all. `[[road_lanes]]` is replaced
    /// as a whole: it is produced by the UI and carries no hand-written
    /// comments. A missing file is created from a plain dump
    pub fn save(&self, filename: &str) -> Result<(), Box<dyn Error>> {
        let dump = toml::to_string(self)?;
        let output = match fs::read_to_string(filename) {
            Ok(existing) => {
                fs::copy(
                    filename,
                    filename.to_owned()
                        + &format!(".{}.bak", Utc::now().format("%Y-%m-%dT%H-%M-%S-%f")),
                )?;
                let mut document: DocumentMut = existing.parse()?;
                let mut fresh: DocumentMut = dump.parse()?;
                // Tables added to the document go after everything it already has
                let mut next_position = max_position(document.as_table()) + 1;
                merge_tables(
                    document.as_table_mut(),
                    std::mem::take(fresh.as_table_mut()),
                    &mut next_position,
                    None,
                    false,
                );
                document.to_string()
            }
            Err(_) => dump,
        };
        fs::write(filename, output)?;
        Ok(())
    }
    pub fn get_copy_no_roads(&self) -> AppSettings {
        AppSettings {
            input: self.input.clone(),
            debug: self.debug.clone(),
            detection: self.detection.clone(),
            tracking: self.tracking.clone(),
            equipment_info: self.equipment_info.clone(),
            road_lanes: Some(Vec::new()),
            worker: self.worker.clone(),
            rest_api: self.rest_api.clone(),
            redis_publisher: self.redis_publisher.clone(),
            dataset_collector: self.dataset_collector.clone(),
            report: self.report.clone(),
        }
    }
    pub fn is_report_mode(&self) -> bool {
        self.report.as_ref().map_or(false, |r| r.enabled)
    }
}

impl fmt::Display for AppSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Equipment ID: {}\n\tVideo input: {}\n\tNetwork weights:{}\n\tNetwork configuration:{:?}\n\tTracker type:{}\n\tRefresh data (millis): {}\n\tBack-end host: {}\n\tBack-end port: {}",
            self.equipment_info.id,
            self.input.video_src,
            self.detection.network_weights,
            self.detection.network_cfg,
            self.tracking
                .typ
                .as_ref()
                .unwrap_or(&"undefined".to_string()),
            self.worker.reset_data_milliseconds,
            self.rest_api.host,
            self.rest_api.back_end_port,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"# Road traffic config
[input]
    # Path or RTSP URL
    video_src = "./data/a.mp4" # trailing note
    process_every_nth_frame = 2

[detection]
    network_weights = "./m.onnx"
    conf_threshold = 0.4
    nms_threshold = 0.2
    net_width = 416
    net_height = 256
    net_classes = ["car", "bus"]
    target_classes = ["car"]

[tracking]
    # Either "bytetrack" or "iou_naive"
    type = "iou_naive"
    max_points_in_track = 100
    kalman_filter = "bbox"
    max_lost_seconds = 2

[equipment_info]
    # Generated once, identifies the installation point
    id = "old-id"

# Zones of interest
[[road_lanes]]
    lane_number = 0
    lane_direction = 0
    # left-bot, right-bot, right-top, left-top
    geometry = [[0, 0], [10, 0], [10, 10], [0, 10]]
    geometry_wgs84 = [[1.0123456789, 2.0], [1.1, 2.0], [1.1, 2.1], [1.0, 2.1]]
    color_rgb = [255, 0, 0]
    # Optional
    [road_lanes.virtual_line]
        geometry = [[0, 5], [10, 5]]
        color_rgb = [0, 255, 0]
        direction = "inbound"

[worker]
    # Period to reset analytics
    reset_data_milliseconds = 30000

[rest_api]
    enable = true
    host = "0.0.0.0"
    back_end_port = 42001
    api_scope = "/api"
    [rest_api.mjpeg_streaming]
        enable = true
        quality = 80

[redis_publisher]
    enable = false
    host = "localhost"
    port = 6379
    password = ""
    db_index = 0
    channel_name = "X"

# [dataset_collector]
#     enabled = true
"#;

    struct TempConfig(String);

    impl TempConfig {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("rrt_settings_{}_{}.toml", name, std::process::id()))
                .to_string_lossy()
                .into_owned();
            fs::write(&path, CONFIG).unwrap();
            TempConfig(path)
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            // Backups are written next to the file
            if let Some(dir) = std::path::Path::new(&self.0).parent() {
                let prefix = format!(
                    "{}.",
                    std::path::Path::new(&self.0)
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                );
                for entry in fs::read_dir(dir).unwrap().flatten() {
                    if entry.file_name().to_string_lossy().starts_with(&prefix) {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    #[test]
    fn save_keeps_comments_and_order() {
        let file = TempConfig::new("comments");
        let mut settings = AppSettings::new(&file.0).unwrap();
        settings.equipment_info.id = "new-id".to_string();
        settings.worker.reset_data_milliseconds = 60000;
        let lanes = settings.road_lanes.as_mut().unwrap();
        let mut second = lanes[0].clone();
        second.lane_number = 1;
        second.virtual_line = None;
        lanes.push(second);
        settings.save(&file.0).unwrap();

        let text = fs::read_to_string(&file.0).unwrap();
        for comment in [
            "# Road traffic config",
            "# Path or RTSP URL",
            "# trailing note",
            "# Either \"bytetrack\" or \"iou_naive\"",
            "# Generated once, identifies the installation point",
            "# Period to reset analytics",
            "# [dataset_collector]",
            "# Zones of interest",
            "# left-bot, right-bot, right-top, left-top",
            "# Optional",
        ] {
            assert!(text.contains(comment), "lost comment {comment:?}:\n{text}");
        }
        assert!(text.contains("id = \"new-id\""), "{text}");
        assert!(text.contains("reset_data_milliseconds = 60000"), "{text}");
        assert_eq!(text.matches("[[road_lanes]]").count(), 2, "{text}");
        // Untouched values keep their spelling: no float noise, ints stay ints
        assert!(text.contains("conf_threshold = 0.4\n"), "{text}");
        assert!(text.contains("max_lost_seconds = 2\n"), "{text}");
        // f32 in memory, but an unchanged coordinate keeps its digits
        assert!(text.contains("[[1.0123456789, 2.0]"), "{text}");
        // The appended zone is indented like the first and has no virtual line
        assert!(text.contains("\n    lane_number = 1\n"), "{text}");
        assert_eq!(
            text.matches("[road_lanes.virtual_line]").count(),
            1,
            "{text}"
        );
        assert!(!text.contains("[    road_lanes"), "{text}");
        // Sections stay where they were
        let pos = |s: &str| text.find(s).unwrap_or_else(|| panic!("no {s} in:\n{text}"));
        assert!(pos("[equipment_info]") < pos("[[road_lanes]]"));
        assert!(pos("[[road_lanes]]") < pos("[worker]"));
        assert!(pos("[worker]") < pos("[rest_api]"));

        let reloaded = AppSettings::new(&file.0).unwrap();
        assert_eq!(reloaded.equipment_info.id, "new-id");
        assert_eq!(reloaded.road_lanes.unwrap().len(), 2);
    }

    #[test]
    fn save_without_zones_drops_the_tables() {
        let file = TempConfig::new("nozones");
        let mut settings = AppSettings::new(&file.0).unwrap();
        settings.road_lanes = Some(Vec::new());
        settings.save(&file.0).unwrap();
        let text = fs::read_to_string(&file.0).unwrap();
        assert!(!text.contains("road_lanes"), "{text}");
        assert!(text.contains("# Period to reset analytics"), "{text}");
        let reloaded = AppSettings::new(&file.0).unwrap();
        assert!(reloaded.road_lanes.map_or(true, |l| l.is_empty()));
    }

    #[test]
    fn save_creates_a_missing_file() {
        let file = TempConfig::new("missing");
        let settings = AppSettings::new(&file.0).unwrap();
        let fresh = format!("{}.fresh.toml", file.0);
        settings.save(&fresh).unwrap();
        let reloaded = AppSettings::new(&fresh).unwrap();
        let _ = fs::remove_file(&fresh);
        assert_eq!(reloaded.equipment_info.id, "old-id");
    }
}
