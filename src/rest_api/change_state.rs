//! Whether what the app holds is saved, and whether what is saved is running.
//!
//! Three copies of the configuration exist while the app runs: the one it was
//! started with, the one last written to the file, and the one edited in
//! memory through the API. Zones are split the same way: the ones the running
//! process uses and the ones last written. Only `save_toml` writes the file, a
//! restart reads it, and every answer that changes any copy reports the whole
//! picture rather than its own part, so a client never has to remember what an
//! earlier request left pending.
use serde::Serialize;
use utoipa::ToSchema;

use crate::lib::data_storage::ThreadedDataStorage;
use crate::rest_api::APIStorage;
use crate::settings::{
    AppSettings, RoadLanesSettings, VirtualLineSettings, ZONES_KEY, needs_restart,
};

/// What is unsaved and what waits for a restart
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChangeState {
    /// Something in memory differs from the configuration file:
    /// `GET /api/mutations/save_toml` writes it, and a restart without it
    /// loses it
    pub save_required: bool,
    /// Dotted paths of what differs from the file; `road_lanes` stands for the
    /// zones
    #[schema(example = json!(["input.video_src", "road_lanes"]))]
    pub unsaved_changes: Vec<String>,
    /// Some change takes effect only after `POST /api/mutations/restart`, and
    /// only once it is saved, since a restart reads the file
    pub restart_required: bool,
    /// Dotted paths of what waits for that restart
    #[schema(example = json!(["input.video_src"]))]
    pub pending_changes: Vec<String>,
}

impl ChangeState {
    pub fn compute(
        current: &AppSettings,
        saved: &AppSettings,
        running: &AppSettings,
        live_zones: &[RoadLanesSettings],
    ) -> Self {
        let mut unsaved = current.differences_from(saved);
        if !same_zones(live_zones, saved.road_lanes.as_deref().unwrap_or(&[])) {
            unsaved.push(ZONES_KEY.to_string());
        }
        unsaved.sort();
        // Zones apply the moment they change, and the live settings do too, so
        // neither is waiting for a restart
        let pending: Vec<String> = current
            .differences_from(running)
            .into_iter()
            .filter(|path| needs_restart(path))
            .collect();
        ChangeState {
            save_required: !unsaved.is_empty(),
            unsaved_changes: unsaved,
            restart_required: !pending.is_empty(),
            pending_changes: pending,
        }
    }

    /// Takes the locks it needs by itself, so it must not be called while any
    /// of them is held
    pub fn of(data: &APIStorage) -> Self {
        let live_zones = live_road_lanes(&data.data_storage);
        let current = data
            .app_settings
            .read()
            .expect("Settings are poisoned [RwLock]");
        let saved = data
            .saved_settings
            .read()
            .expect("Saved settings are poisoned [RwLock]");
        Self::compute(&current, &saved, &data.running_settings, &live_zones)
    }
}

/// The zones the running process uses, in the form the configuration file
/// keeps them
pub fn live_road_lanes(data_storage: &ThreadedDataStorage) -> Vec<RoadLanesSettings> {
    let ds_guard = data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard
        .zones
        .read()
        .expect("Spatial data is poisoned [RWLock]");
    let mut road_lanes: Vec<RoadLanesSettings> = zones
        .values()
        .map(|zone_guarded| {
            let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            RoadLanesSettings {
                // BGR -> RGB
                color_rgb: [
                    zone.color[2] as i16,
                    zone.color[1] as i16,
                    zone.color[0] as i16,
                ],
                geometry: zone
                    .get_pixel_coordinates()
                    .iter()
                    .map(|pt| [pt.x as i32, pt.y as i32])
                    .collect(),
                geometry_wgs84: zone
                    .get_spatial_coordinates_epsg4326()
                    .iter()
                    .map(|pt| [pt.x, pt.y])
                    .collect(),
                lane_direction: zone.road_lane_direction,
                lane_number: zone.road_lane_num,
                virtual_line: zone
                    .get_virtual_line()
                    .as_ref()
                    .map(|vl| VirtualLineSettings {
                        geometry: vl.line,
                        // BGR -> RGB
                        color_rgb: [vl.color[0] as i16, vl.color[1] as i16, vl.color[2] as i16],
                        direction: vl.direction.to_string(),
                    }),
            }
        })
        .collect();
    // Zones live in a HashMap; a fixed order keeps the saved file stable
    road_lanes.sort_by_key(|lane| (lane.lane_direction, lane.lane_number));
    road_lanes
}

/// Zones compared as a set: the file keeps them in the order they were
/// written, the running process in no order at all
fn same_zones(a: &[RoadLanesSettings], b: &[RoadLanesSettings]) -> bool {
    fn canonical(zones: &[RoadLanesSettings]) -> Vec<String> {
        let mut rendered: Vec<String> = zones
            .iter()
            .filter_map(|zone| serde_json::to_string(zone).ok())
            .collect();
        rendered.sort();
        rendered
    }
    canonical(a) == canonical(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(name: &str) -> AppSettings {
        let path =
            std::env::temp_dir().join(format!("rrt_state_{}_{}.toml", name, std::process::id()));
        std::fs::write(
            &path,
            r#"
[input]
    video_src = "rtsp://cam/stream"
[detection]
    network_weights = "m.onnx"
    conf_threshold = 0.4
    nms_threshold = 0.2
    net_classes = ["car"]
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
    password = ""
    db_index = 0
    channel_name = "X"
"#,
        )
        .unwrap();
        let settings = AppSettings::new(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        settings
    }

    fn zone(direction: u8, lane: u16) -> RoadLanesSettings {
        RoadLanesSettings {
            lane_number: lane,
            lane_direction: direction,
            geometry: vec![[0, 0], [10, 0], [10, 10], [0, 10]],
            geometry_wgs84: vec![[1.0, 2.0], [1.1, 2.0], [1.1, 2.1], [1.0, 2.1]],
            color_rgb: [255, 0, 0],
            virtual_line: None,
        }
    }

    #[test]
    fn nothing_to_do_right_after_start() {
        let running = settings("start");
        let mut saved = running.clone();
        saved.road_lanes = Some(vec![zone(0, 0)]);
        let state = ChangeState::compute(&running, &saved, &running, &[zone(0, 0)]);
        assert!(!state.save_required && !state.restart_required, "{state:?}");
    }

    #[test]
    fn a_setting_changed_in_memory_needs_saving_and_then_a_restart() {
        let running = settings("changed");
        let saved = running.clone();
        let mut current = running.clone();
        current.input.video_src = "rtsp://cam".to_string();
        let state = ChangeState::compute(&current, &saved, &running, &[]);
        assert_eq!(state.unsaved_changes, ["input.video_src"]);
        assert_eq!(state.pending_changes, ["input.video_src"]);
        assert!(state.save_required && state.restart_required);

        // Once saved, only the restart is left
        let state = ChangeState::compute(&current, &current, &running, &[]);
        assert!(!state.save_required, "{state:?}");
        assert!(state.restart_required, "{state:?}");
    }

    #[test]
    fn a_live_setting_needs_saving_but_no_restart() {
        let running = settings("live");
        let mut current = running.clone();
        current.equipment_info.id = "eq-2".to_string();
        let state = ChangeState::compute(&current, &running, &running, &[]);
        assert_eq!(state.unsaved_changes, ["equipment_info.id"]);
        assert!(state.save_required);
        assert!(!state.restart_required, "{state:?}");
    }

    #[test]
    fn changed_zones_need_saving_but_no_restart() {
        let running = settings("zones");
        let mut saved = running.clone();
        saved.road_lanes = Some(vec![zone(0, 0)]);
        let state = ChangeState::compute(&running, &saved, &running, &[zone(0, 0), zone(0, 1)]);
        assert_eq!(state.unsaved_changes, ["road_lanes"]);
        assert!(state.save_required);
        assert!(!state.restart_required, "{state:?}");
    }

    #[test]
    fn the_order_of_zones_does_not_count_as_a_change() {
        assert!(same_zones(
            &[zone(0, 0), zone(1, 0)],
            &[zone(1, 0), zone(0, 0)]
        ));
        assert!(!same_zones(&[zone(0, 0)], &[zone(0, 0), zone(1, 0)]));
    }

    #[test]
    fn everything_pending_is_reported_not_only_the_last_change() {
        let running = settings("cumulative");
        let mut current = running.clone();
        current.input.video_src = "rtsp://cam".to_string();
        current.verbose.get_or_insert_with(Default::default).level = Some("debug".to_string());
        let state = ChangeState::compute(&current, &running, &running, &[]);
        assert_eq!(state.unsaved_changes, ["input.video_src", "verbose.level"]);
        assert_eq!(state.pending_changes, ["input.video_src"]);
    }
}
