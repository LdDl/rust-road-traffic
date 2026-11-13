use std::collections::{HashMap, HashSet};

use crate::lib::constants::EPSILON;
use std::sync::{Arc, Mutex, RwLock};

use crate::lib::track_observations::TrackObservation;
use crate::lib::zones::zones::Zone;
use crate::settings::ViolationsSettings;
use crate::violations::event::ViolationEvent;
use crate::violations::rules::ViolationRule;
use crate::violations::rules::speeding::SpeedingRule;
use crate::violations::rules::stationary::{StationaryRuleKind, StationaryZoneRule};
use crate::violations::rules::wrong_way::WrongWayRule;
use crate::violations::solid_line::{SolidLineDefinition, SolidLineRule};

#[derive(Debug, Clone, Copy)]
pub struct RuleRuntimeConfig {
    pub state_ttl_sec: f32,
    pub cleanup_interval_sec: f32,
}

/// Rules of one zone, kept together so the zone's rules can be replaced as a
/// unit when the zone changes.
struct RuleEntry {
    zone_id: String,
    rules: Vec<Box<dyn ViolationRule>>,
}

pub struct ViolationCoordinator {
    /// The zones the process runs, shared with the detection loop and the
    /// REST API. Rules are built from these live objects, never from a
    /// startup snapshot: a zone moved or re-ruled through the API is seen on
    /// the next frame without a restart.
    zones: Arc<RwLock<HashMap<String, Mutex<Zone>>>>,
    runtime: RuleRuntimeConfig,
    rules: Vec<RuleEntry>,
    /// Serialized zone context per zone id, so a change is noticed without
    /// rebuilding rules every frame.
    signatures: HashMap<String, String>,
}

impl ViolationCoordinator {
    /// Creates a coordinator without rules; the first `process_frame` call
    /// builds them from the live zones.
    pub fn new(
        zones: Arc<RwLock<HashMap<String, Mutex<Zone>>>>,
        config: ViolationsSettings,
    ) -> Self {
        ViolationCoordinator {
            zones,
            runtime: RuleRuntimeConfig {
                state_ttl_sec: config.state_ttl_sec,
                cleanup_interval_sec: config.cleanup_interval_sec,
            },
            rules: Vec::new(),
            signatures: HashMap::new(),
        }
    }

    /// Passes the observations of one frame to every configured rule and
    /// collects their events in rule iteration order (zone ids sorted, rules
    /// in a fixed per-zone order), so events of a frame are deterministic.
    pub fn process_frame(&mut self, observations: &[TrackObservation]) -> Vec<ViolationEvent> {
        self.refresh_rules_if_needed();

        let mut out = Vec::new();
        for entry in self.rules.iter_mut() {
            for rule in entry.rules.iter_mut() {
                out.extend(rule.process_frame(observations));
            }
        }
        out
    }

    /// Rebuilds the rule list when the live zones changed since the last
    /// refresh. Rules of unchanged zones keep their track state; rules of
    /// changed or removed zones are dropped, and new zones get their rules.
    fn refresh_rules_if_needed(&mut self) {
        let zones = self.zones.read().expect("Zones are poisoned [RwLock]");

        let mut new_signatures: HashMap<String, String> = HashMap::new();
        for (zone_id, zone_guarded) in zones.iter() {
            let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            new_signatures.insert(zone_id.clone(), zone_signature(&zone));
        }

        // Keep only the rules of zones that are still there, unchanged.
        self.rules.retain(|entry| {
            new_signatures.get(&entry.zone_id) == self.signatures.get(&entry.zone_id)
        });
        self.signatures = new_signatures;

        // Build rules for zones this refresh does not have yet, in sorted
        // order so the event order does not depend on map iteration.
        let existing: HashSet<String> = self.rules.iter().map(|e| e.zone_id.clone()).collect();
        let mut missing: Vec<String> = zones
            .keys()
            .filter(|zone_id| !existing.contains(*zone_id))
            .cloned()
            .collect();
        missing.sort();

        for zone_id in missing {
            let Some(zone_guarded) = zones.get(&zone_id) else {
                continue;
            };
            let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            let rules = build_rules_for_zone(&zone, &self.runtime);
            if !rules.is_empty() {
                self.rules.push(RuleEntry { zone_id, rules });
            }
        }
    }
}

/// Serialized context of a zone: everything a rule reads from it. A change in
/// any of it replaces the zone's rules.
fn zone_signature(zone: &Zone) -> String {
    serde_json::to_string(&(
        zone.get_pixel_coordinates(),
        zone.get_virtual_line()
            .as_ref()
            .map(|line| (line.line, line.direction.to_string())),
        zone.get_rules(),
    ))
    .unwrap_or_default()
}

/// The rules a zone's current configuration implies, in a fixed order:
/// speeding, no_stopping, no_parking, wrong_way, solid_line.
fn build_rules_for_zone(zone: &Zone, runtime: &RuleRuntimeConfig) -> Vec<Box<dyn ViolationRule>> {
    let zone_id = zone.get_id();
    let rules = zone.get_rules();
    let geometry: Vec<[i32; 2]> = zone
        .get_pixel_coordinates()
        .iter()
        .map(|pt| [pt.x as i32, pt.y as i32])
        .collect();

    let mut out: Vec<Box<dyn ViolationRule>> = Vec::new();

    if let Some(rule) = &rules.speeding {
        out.push(Box::new(SpeedingRule::from_embedded(
            zone_id.clone(),
            geometry.clone(),
            rule.clone(),
            *runtime,
        )));
    }

    if let Some(rule) = &rules.no_stopping {
        out.push(Box::new(StationaryZoneRule::from_embedded(
            zone_id.clone(),
            geometry.clone(),
            rule.clone(),
            *runtime,
            StationaryRuleKind::NoStopping,
        )));
    }

    if let Some(rule) = &rules.no_parking {
        out.push(Box::new(StationaryZoneRule::from_embedded(
            zone_id.clone(),
            geometry.clone(),
            rule.clone(),
            *runtime,
            StationaryRuleKind::NoParking,
        )));
    }

    if let Some(rule) = &rules.wrong_way {
        if let Some(allowed_direction) = zone.get_virtual_line().and_then(|virtual_line| {
            allowed_direction_from_virtual_line(virtual_line.line, virtual_line.direction)
        }) {
            out.push(Box::new(WrongWayRule::from_embedded(
                zone_id.clone(),
                geometry.clone(),
                allowed_direction,
                rule.min_direction_samples,
                *runtime,
            )));
        }
    }

    if let Some(solid_line) = zone.get_solid_line().filter(|line| line.is_valid()) {
        out.push(Box::new(SolidLineRule::new(
            SolidLineDefinition::from_zone(zone_id.clone(), solid_line),
            *runtime,
        )));
    }

    out
}

/// Computes the allowed unit direction perpendicular to the virtual line.
/// Outbound uses the normal (-dy, dx), while inbound uses its opposite.
///
/// The result depends on endpoint order: swapping the endpoints reverses
/// the returned direction for the same inbound/outbound setting.
/// Returns None if the computed line length is non-finite or too small.
fn allowed_direction_from_virtual_line(
    geometry: [[i32; 2]; 2],
    direction: crate::lib::zones::VirtualLineDirection,
) -> Option<[f32; 2]> {
    let ax = geometry[0][0] as f32;
    let ay = geometry[0][1] as f32;
    let bx = geometry[1][0] as f32;
    let by = geometry[1][1] as f32;

    let dx = bx - ax;
    let dy = by - ay;

    let length = (dx * dx + dy * dy).sqrt();
    if !length.is_finite() || length <= EPSILON {
        return None;
    }

    let left_normal = [-dy / length, dx / length];

    let vector = match direction {
        crate::lib::zones::VirtualLineDirection::Inbound => [-left_normal[0], -left_normal[1]],
        crate::lib::zones::VirtualLineDirection::Outbound => left_normal,
    };

    Some(vector)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::data_storage::new_datastorage;
    use crate::lib::track_observations::test_support::observation;
    use crate::settings::{LaneRulesSettings, LaneSpeedingRuleSettings, RoadLanesSettings};
    use uuid::Uuid;

    fn lane_with_speeding(threshold_kmh: f32, id: &str) -> RoadLanesSettings {
        RoadLanesSettings {
            id: Some(id.to_string()),
            lane_number: 0,
            lane_direction: 0,
            geometry: vec![[0, 0], [100, 0], [100, 100], [0, 100]],
            geometry_wgs84: vec![[1.0, 2.0], [1.1, 2.0], [1.1, 2.1], [1.0, 2.1]],
            color_rgb: [255, 0, 0],
            virtual_line: None,
            rules: LaneRulesSettings {
                speeding: Some(LaneSpeedingRuleSettings { threshold_kmh }),
                ..LaneRulesSettings::default()
            },
        }
    }

    #[test]
    fn rule_geometry_follows_the_live_zone() {
        let storage = new_datastorage("eq".to_string());
        storage
            .write()
            .unwrap()
            .insert_zone(Zone::from(&lane_with_speeding(50.0, "zone-1")))
            .unwrap();
        let zones = storage.read().unwrap().zones.clone();
        let mut coordinator = ViolationCoordinator::new(
            zones,
            ViolationsSettings {
                enabled: true,
                state_ttl_sec: 7.0,
                cleanup_interval_sec: 1.0,
            },
        );
        let track = Uuid::new_v4();

        // Speeding inside the polygon fires.
        assert_eq!(
            coordinator
                .process_frame(&[observation(
                    track,
                    1.0,
                    50.0,
                    50.0,
                    Some("zone-1"),
                    Some(60.0)
                )])
                .len(),
            1
        );
        // The same episode is suppressed.
        assert!(
            coordinator
                .process_frame(&[observation(
                    track,
                    1.1,
                    50.0,
                    50.0,
                    Some("zone-1"),
                    Some(60.0)
                )])
                .is_empty()
        );

        // The zone is moved through the API path: the polygon no longer
        // covers (50, 50). The rules must follow on the next frame, without a
        // restart, instead of matching the stale startup snapshot.
        {
            let zones_map = storage.read().unwrap().zones.clone();
            let zones_read = zones_map.read().unwrap();
            let mut zone = zones_read
                .get("zone-1")
                .expect("zone exists")
                .lock()
                .expect("zone lock");
            zone.update_pixel_map([[200, 200], [300, 200], [300, 300], [200, 300]]);
        }
        let other = Uuid::new_v4();
        assert!(
            coordinator
                .process_frame(&[observation(
                    other,
                    2.0,
                    50.0,
                    50.0,
                    Some("zone-1"),
                    Some(60.0)
                )])
                .is_empty()
        );

        // A point inside the new polygon fires again.
        assert_eq!(
            coordinator
                .process_frame(&[observation(
                    other,
                    2.1,
                    250.0,
                    250.0,
                    Some("zone-1"),
                    Some(60.0)
                )])
                .len(),
            1
        );
    }

    #[test]
    fn rule_set_follows_the_live_zone_rules() {
        let storage = new_datastorage("eq".to_string());
        let mut lane = lane_with_speeding(50.0, "zone-1");
        lane.rules = LaneRulesSettings::default();
        storage
            .write()
            .unwrap()
            .insert_zone(Zone::from(&lane))
            .unwrap();
        let zones = storage.read().unwrap().zones.clone();
        let mut coordinator = ViolationCoordinator::new(
            zones,
            ViolationsSettings {
                enabled: true,
                state_ttl_sec: 7.0,
                cleanup_interval_sec: 1.0,
            },
        );
        let track = Uuid::new_v4();

        // No rules configured: nothing fires.
        assert!(
            coordinator
                .process_frame(&[observation(
                    track,
                    1.0,
                    50.0,
                    50.0,
                    Some("zone-1"),
                    Some(60.0)
                )])
                .is_empty()
        );

        // The zone gains a speeding rule while running.
        {
            let zones_map = storage.read().unwrap().zones.clone();
            let zones_read = zones_map.read().unwrap();
            let mut zone = zones_read
                .get("zone-1")
                .expect("zone exists")
                .lock()
                .expect("zone lock");
            zone.set_rules(LaneRulesSettings {
                speeding: Some(LaneSpeedingRuleSettings {
                    threshold_kmh: 50.0,
                }),
                ..LaneRulesSettings::default()
            });
        }
        assert_eq!(
            coordinator
                .process_frame(&[observation(
                    track,
                    1.1,
                    50.0,
                    50.0,
                    Some("zone-1"),
                    Some(60.0)
                )])
                .len(),
            1
        );
    }
}
