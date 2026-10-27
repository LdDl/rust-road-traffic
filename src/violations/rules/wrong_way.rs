use std::collections::HashMap;

use uuid::Uuid;

use crate::lib::track_observations::{PointPx, TrackObservation};
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::{ViolationEvent, ViolationStatus};
use crate::violations::rules::geometry::contains_point;
use crate::violations::rules::ViolationRule;

#[derive(Debug, Clone)]
struct TrackState {
    first_point: PointPx,
    last_point: PointPx,
    last_seen_at: f32,
    samples: usize,
    fired: bool,
}

pub struct WrongWayRule {
    rule_id: String,
    zone_id: String,
    polygon_px: Vec<PointPx>,
    allowed_direction_unit: [f32; 2],
    min_direction_samples: usize,
    state_ttl_sec: f32,
    cleanup_interval_sec: f32,
    next_cleanup_at: f32,
    tracks: HashMap<Uuid, TrackState>,
}

impl WrongWayRule {
    /// Creates a wrong-way rule from the zone geometry, allowed direction,
    /// sample threshold, and runtime cleanup settings.
    ///
    /// The allowed direction is normalized, with an invalid vector stored
    /// as zero. The sample threshold is clamped to at least two, and the
    /// runtime intervals are clamped to at least 0.1 seconds.
    pub fn from_embedded(
        zone_id: String,
        geometry: Vec<[i32; 2]>,
        allowed_direction: [f32; 2],
        min_direction_samples: usize,
        runtime: RuleRuntimeConfig,
    ) -> Self {
        let allowed_direction_unit =
            Self::normalize(allowed_direction[0], allowed_direction[1])
                .unwrap_or((0.0, 0.0));

        Self {
            rule_id: format!("{zone_id}.wrong_way"),
            zone_id,
            polygon_px: geometry
                .into_iter()
                .map(|pt| PointPx {
                    x: pt[0] as f32,
                    y: pt[1] as f32,
                })
                .collect(),
            allowed_direction_unit: [allowed_direction_unit.0, allowed_direction_unit.1],
            min_direction_samples: min_direction_samples.max(2),
            state_ttl_sec: runtime.state_ttl_sec.max(0.1),
            cleanup_interval_sec: runtime.cleanup_interval_sec.max(0.1),
            next_cleanup_at: 0.0,
            tracks: HashMap::new(),
        }
    }

    /// Removes track state older than the configured TTL when cleanup is due.
    /// Age is measured from each track's last observation in source time.
    fn cleanup_if_needed(&mut self, now: f32) {
        if now < self.next_cleanup_at {
            return;
        }

        let ttl = self.state_ttl_sec;
        self.tracks
            .retain(|_, state| now - state.last_seen_at <= ttl);

        self.next_cleanup_at = now + self.cleanup_interval_sec;
    }

    /// Converts a vector into a unit vector.
    /// Returns None if its computed length is non-finite or too small.
    fn normalize(vx: f32, vy: f32) -> Option<(f32, f32)> {
        let norm = (vx * vx + vy * vy).sqrt();
        if !norm.is_finite() || norm <= f32::EPSILON {
            return None;
        }

        Some((vx / norm, vy / norm))
    }

    /// Returns the normalized displacement from one point to another.
    /// Returns None if the displacement cannot be normalized.
    fn movement_direction(from: &PointPx, to: &PointPx) -> Option<(f32, f32)> {
        Self::normalize(to.x - from.x, to.y - from.y)
    }

    /// Returns the normalized allowed direction.
    /// Returns None when the stored direction is invalid or zero.
    fn allowed_direction(&self) -> Option<(f32, f32)> {
        Self::normalize(
            self.allowed_direction_unit[0],
            self.allowed_direction_unit[1],
        )
    }

    /// Builds a confirmed wrong-way event using the current observation,
    /// normalized movement direction, and its alignment with the allowed
    /// direction. The details include the configured sample threshold.
    fn make_event(
        &self,
        obs: &TrackObservation,
        movement: (f32, f32),
        dot: f32,
    ) -> ViolationEvent {
        ViolationEvent {
            event_id: Uuid::new_v4(),
            event_type: "traffic.wrong_way".to_string(),
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
                "track moved in forbidden direction in zone '{}' after {} samples; movement=({:.3}, {:.3}), alignment={:.3}",
                self.zone_id,
                self.min_direction_samples,
                movement.0,
                movement.1,
                dot,
            ),
        }
    }
}

impl ViolationRule for WrongWayRule {
    /// Processes an observation within the configured zone and polygon.
    /// Once the sample counter reaches the configured threshold, compares
    /// displacement from the first anchor to the current anchor with the
    /// allowed direction.
    ///
    /// Emits an event when the current step has a normalizable displacement
    /// and the overall direction alignment is below -0.2.
    ///
    /// Already reported tracks refresh their state without emitting again.
    /// A zone mismatch or a point outside the polygon removes track state,
    /// allowing a subsequent visit to be evaluated independently.
    ///
    /// The current implementation initializes samples to one and increments
    /// it during the same call, so the first observation is counted twice.
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let now = obs.source_time_relative_sec;
        self.cleanup_if_needed(now);

        if obs.zone_id.as_deref() != Some(self.zone_id.as_str()) {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        if !contains_point(&self.polygon_px, &obs.image_point_px) {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        let allowed = match self.allowed_direction() {
            Some(v) => v,
            None => return Vec::new(),
        };

        let current_point = obs.anchor_px.clone();

        let state = self.tracks.entry(obs.track_id).or_insert(TrackState {
            first_point: current_point.clone(),
            last_point: current_point.clone(),
            last_seen_at: now,
            samples: 1,
            fired: false,
        });

        if state.fired {
            state.last_point = current_point;
            state.last_seen_at = now;
            state.samples += 1;
            return Vec::new();
        }

        let previous_point = state.last_point.clone();
        state.last_point = current_point.clone();
        state.last_seen_at = now;
        state.samples += 1;

        if state.samples < self.min_direction_samples {
            return Vec::new();
        }

        let movement = match Self::movement_direction(&state.first_point, &current_point) {
            Some(v) => v,
            None => return Vec::new(),
        };

        let dot = movement.0 * allowed.0 + movement.1 * allowed.1;
        let moved_this_step =
            Self::movement_direction(&previous_point, &current_point).is_some();

        if moved_this_step && dot < -0.2 {
            state.fired = true;
            return vec![self.make_event(obs, movement, dot)];
        }

        Vec::new()
    }
}