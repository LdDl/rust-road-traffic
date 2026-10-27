use std::collections::HashMap;

use uuid::Uuid;

use crate::lib::track_observations::{PointPx, TrackObservation};
use crate::settings::LaneSpeedingRuleSettings;
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::{ViolationEvent, ViolationStatus};
use crate::violations::rules::geometry::contains_point;
use crate::violations::rules::ViolationRule;

pub struct SpeedingRule {
    rule_id: String,
    zone_id: String,
    polygon_px: Vec<PointPx>,
    threshold_kmh: f32,
    fired_tracks: HashMap<Uuid, f32>,

    state_ttl_sec: f32,
    cleanup_interval_sec: f32,
    next_cleanup_at: f32,
}

impl SpeedingRule {
    /// Creates a speeding rule from the zone geometry, speed threshold,
    /// and runtime settings used to expire reported-track records.
    pub fn from_embedded(
        zone_id: String,
        geometry: Vec<[i32; 2]>,
        settings: LaneSpeedingRuleSettings,
        runtime: RuleRuntimeConfig,
    ) -> Self {
        Self {
            rule_id: format!("{zone_id}.speeding"),
            zone_id,
            polygon_px: geometry
                .into_iter()
                .map(|pt| PointPx {
                    x: pt[0] as f32,
                    y: pt[1] as f32,
                })
                .collect(),
            threshold_kmh: settings.threshold_kmh,
            fired_tracks: HashMap::new(),
            state_ttl_sec: runtime.state_ttl_sec,
            cleanup_interval_sec: runtime.cleanup_interval_sec,
            next_cleanup_at: 0.0,
        }
    }

    /// Removes expired reported-track records when the next cleanup is due.
    /// Each stored timestamp is the track's latest observation in source time;
    /// records with a negative age are also discarded.
    fn cleanup_if_needed(&mut self, now: f32) {
        if now < self.next_cleanup_at {
            return;
        }

        let ttl = self.state_ttl_sec;

        self.fired_tracks.retain(|_, last_seen_at| {
            let age = now - *last_seen_at;
            age >= 0.0 && age <= ttl
        });

        self.next_cleanup_at = now + self.cleanup_interval_sec;
    }

    /// Builds a confirmed speeding event from the current observation
    /// and the speed that exceeded the configured threshold.
    fn make_event(&self, obs: &TrackObservation, speed_kmh: f32) -> ViolationEvent {
        ViolationEvent {
            event_id: Uuid::new_v4(),
            event_type: "traffic.speeding".to_string(),
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
            speed_kmh: Some(speed_kmh),
            rule_id: self.rule_id.clone(),
            details: format!(
                "track exceeded speed threshold inside zone '{}' ({:.2} km/h > {:.2} km/h)",
                self.zone_id, speed_kmh, self.threshold_kmh,
            ),
        }
    }
}

impl ViolationRule for SpeedingRule {
    /// Processes a track observation and emits an event when its anchor
    /// is inside the configured polygon and its valid speed strictly
    /// exceeds the configured threshold.
    ///
    /// Already reported tracks refresh their last-seen timestamp before
    /// geometry and speed checks, suppressing repeated events while
    /// observations keep their records alive.
    ///
    /// Expired records are removed before processing, allowing a track
    /// to produce another event after an observation gap exceeding the TTL.
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let now = obs.source_time_relative_sec;

        if !now.is_finite() {
            return Vec::new();
        }

        self.cleanup_if_needed(now);

        // Expire this track immediately, even between full cleanup passes.
        let expired = self
            .fired_tracks
            .get(&obs.track_id)
            .map_or(false, |last_seen_at| {
                let age = now - *last_seen_at;
                age < 0.0 || age > self.state_ttl_sec
            });

        if expired {
            self.fired_tracks.remove(&obs.track_id);
        }

        // Refresh before geometry/speed checks so an observed track
        // does not lose its deduplication record.
        if let Some(last_seen_at) = self.fired_tracks.get_mut(&obs.track_id) {
            *last_seen_at = now;
            return Vec::new();
        }

        if !contains_point(&self.polygon_px, &obs.anchor_px) {
            return Vec::new();
        }

        let speed_kmh = match obs.speed_kmh {
            Some(speed) if speed.is_finite() && speed >= 0.0 => speed,
            _ => return Vec::new(),
        };

        if speed_kmh <= self.threshold_kmh {
            return Vec::new();
        }

        self.fired_tracks.insert(obs.track_id, now);

        vec![self.make_event(obs, speed_kmh)]
    }
}