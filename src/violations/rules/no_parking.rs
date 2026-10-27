use std::collections::HashMap;

use uuid::Uuid;

use crate::lib::track_observations::{PointPx, TrackObservation};
use crate::settings::LaneZoneViolationRuleSettings;
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::{ViolationEvent, ViolationStatus};
use crate::violations::rules::geometry::contains_point;
use crate::violations::rules::ViolationRule;

#[derive(Debug, Clone)]
struct TrackState {
    entered_at: f32,
    last_seen_at: f32,
    fired: bool,
}

pub struct NoParkingRule {
    rule_id: String,
    zone_id: String,
    polygon_px: Vec<PointPx>,
    min_duration_sec: f32,
    stationary_speed_kmh: f32,

    tracks: HashMap<Uuid, TrackState>,

    state_ttl_sec: f32,
    cleanup_interval_sec: f32,
    next_cleanup_at: f32,
}

impl NoParkingRule {
    /// Creates a no-parking rule from the zone geometry, rule settings,
    /// and runtime settings used to expire inactive track state.
    pub fn from_embedded(
        zone_id: String,
        geometry: Vec<[i32; 2]>,
        settings: LaneZoneViolationRuleSettings,
        runtime: RuleRuntimeConfig,
    ) -> Self {
        Self {
            rule_id: format!("{zone_id}.no_parking"),
            zone_id,
            polygon_px: geometry
                .into_iter()
                .map(|pt| PointPx {
                    x: pt[0] as f32,
                    y: pt[1] as f32,
                })
                .collect(),
            min_duration_sec: settings.min_duration_sec,
            stationary_speed_kmh: settings.stationary_speed_kmh,
            tracks: HashMap::new(),
            state_ttl_sec: runtime.state_ttl_sec,
            cleanup_interval_sec: runtime.cleanup_interval_sec,
            next_cleanup_at: 0.0,
        }
    }

    /// Removes expired track state when the next cleanup is due.
    /// Track age is measured from its last observation in source time;
    /// entries with a negative age are also discarded.
    fn cleanup_if_needed(&mut self, now: f32) {
        if now < self.next_cleanup_at {
            return;
        }

        let ttl = self.state_ttl_sec;

        self.tracks.retain(|_, state| {
            let age = now - state.last_seen_at;
            age >= 0.0 && age <= ttl
        });

        self.next_cleanup_at = now + self.cleanup_interval_sec;
    }

    /// Builds a confirmed no-parking event from the current observation
    /// and the elapsed stationary period.
    fn make_event(&self, obs: &TrackObservation, duration_sec: f32) -> ViolationEvent {
        ViolationEvent {
            event_id: Uuid::new_v4(),
            event_type: "traffic.no_parking".to_string(),
            status: ViolationStatus::Confirmed,
            equipment_id: obs.equipment_id.clone(),
            track_id: obs.track_id,
            observed_at: obs.observed_at,
            source_time_relative_sec: obs.source_time_relative_sec,
            class_name: obs.class_name.clone(),
            confidence: obs.confidence,
            bbox: obs.bbox.clone(),
            anchor_px: obs.anchor_px.clone(),
            image_point_px: obs.image_point_px.clone(),
            projected_point: obs.projected_point.clone(),
            zone_id: Some(self.zone_id.clone()),
            previous_zone_id: obs.previous_zone_id.clone(),
            crossed_virtual_line: obs.crossed_virtual_line,
            speed_kmh: obs.speed_kmh,
            rule_id: self.rule_id.clone(),
            details: format!(
                "track remained stationary in no-parking zone '{}' for {:.2} sec (threshold {:.2} sec)",
                self.zone_id, duration_sec, self.min_duration_sec,
            ),
        }
    }
}

impl ViolationRule for NoParkingRule {
    /// Processes a track observation and emits an event once the track
    /// has remained inside the polygon at or below the stationary speed
    /// threshold for the configured duration.
    ///
    /// Before an event is emitted, leaving the polygon or receiving an
    /// invalid or excessive speed resets the stationary timer.
    /// Observation gaps within the state TTL do not reset the timer.
    ///
    /// After an event is emitted, further events for the same track are
    /// suppressed while observations keep its state alive.
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let now = obs.source_time_relative_sec;

        if !now.is_finite() {
            return Vec::new();
        }

        self.cleanup_if_needed(now);

        // Expire this track even between full cleanup passes.
        let expired = self.tracks.get(&obs.track_id).map_or(false, |state| {
            let age = now - state.last_seen_at;
            age < 0.0 || age > self.state_ttl_sec
        });

        if expired {
            self.tracks.remove(&obs.track_id);
        }

        // Refresh reported tracks before any early return.
        // A visible track must not lose its deduplication record.
        if let Some(state) = self.tracks.get_mut(&obs.track_id) {
            state.last_seen_at = now;

            if state.fired {
                return Vec::new();
            }
        }

        if !contains_point(&self.polygon_px, &obs.image_point_px) {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        let speed_kmh = match obs.speed_kmh {
            Some(speed) if speed.is_finite() && speed >= 0.0 => speed,
            _ => {
                // Unknown speed cannot confirm continuous stationary time.
                self.tracks.remove(&obs.track_id);
                return Vec::new();
            }
        };

        if speed_kmh > self.stationary_speed_kmh {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        let state = self.tracks.entry(obs.track_id).or_insert(TrackState {
            entered_at: now,
            last_seen_at: now,
            fired: false,
        });

        state.last_seen_at = now;

        let duration_sec = now - state.entered_at;

        if duration_sec < self.min_duration_sec {
            return Vec::new();
        }

        state.fired = true;

        vec![self.make_event(obs, duration_sec)]
    }
}