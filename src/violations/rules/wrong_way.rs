use crate::lib::constants::EPSILON;
use crate::lib::spatial::Point2f;
use crate::lib::track_observations::TrackObservation;
use crate::lib::zones::geometry::polygon_contains_point;
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::ViolationEvent;
use crate::violations::rules::ViolationRule;
use crate::violations::rules::state::{RuleStateStore, TrackStateEntry};

#[derive(Debug, Clone)]
struct TrackState {
    first_point: Point2f,
    last_point: Point2f,
    last_seen_at: f32,
    samples: usize,
    fired: bool,
}

impl TrackStateEntry for TrackState {
    fn last_seen_at(&self) -> f32 {
        self.last_seen_at
    }
}

pub struct WrongWayRule {
    rule_id: String,
    zone_id: String,
    polygon_px: Vec<Point2f>,
    allowed_direction_unit: [f32; 2],
    min_direction_samples: usize,
    tracks: RuleStateStore<TrackState>,
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
            Self::normalize(allowed_direction[0], allowed_direction[1]).unwrap_or((0.0, 0.0));

        Self {
            rule_id: format!("{zone_id}.wrong_way"),
            zone_id,
            polygon_px: geometry
                .into_iter()
                .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
                .collect(),
            allowed_direction_unit: [allowed_direction_unit.0, allowed_direction_unit.1],
            min_direction_samples: min_direction_samples.max(2),
            tracks: RuleStateStore::new(
                runtime.state_ttl_sec.max(0.1),
                runtime.cleanup_interval_sec.max(0.1),
            ),
        }
    }

    /// Converts a vector into a unit vector.
    /// Returns None if its computed length is non-finite or too small.
    fn normalize(vx: f32, vy: f32) -> Option<(f32, f32)> {
        let norm = (vx * vx + vy * vy).sqrt();
        if !norm.is_finite() || norm <= EPSILON {
            return None;
        }

        Some((vx / norm, vy / norm))
    }

    /// Returns the normalized displacement from one point to another.
    /// Returns None if the displacement cannot be normalized.
    fn movement_direction(from: Point2f, to: Point2f) -> Option<(f32, f32)> {
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
    fn make_event(&self, obs: &TrackObservation, movement: (f32, f32), dot: f32) -> ViolationEvent {
        ViolationEvent::from_observation(
            obs,
            "traffic.wrong_way",
            Some(self.zone_id.clone()),
            self.rule_id.clone(),
            format!(
                "track moved in forbidden direction in zone '{}' after {} samples; movement=({:.3}, {:.3}), alignment={:.3}",
                self.zone_id, self.min_direction_samples, movement.0, movement.1, dot,
            ),
        )
    }
}

impl ViolationRule for WrongWayRule {
    fn process_frame(&mut self, observations: &[TrackObservation]) -> Vec<ViolationEvent> {
        let mut events = Vec::new();
        for obs in observations {
            events.extend(self.process(obs));
        }
        events
    }
}

impl WrongWayRule {
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
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let now = obs.source_time_relative_sec;
        if !now.is_finite() {
            return Vec::new();
        }
        self.tracks.cleanup_if_needed(now);

        if obs.zone_id.as_deref() != Some(self.zone_id.as_str()) {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        if !polygon_contains_point(&self.polygon_px, obs.image_point_px.x, obs.image_point_px.y) {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        let allowed = match self.allowed_direction() {
            Some(v) => v,
            None => return Vec::new(),
        };

        let current_point = obs.anchor_px;

        let state = self.tracks.entry(obs.track_id).or_insert(TrackState {
            first_point: current_point,
            last_point: current_point,
            last_seen_at: now,
            samples: 0,
            fired: false,
        });

        if state.fired {
            state.last_point = current_point;
            state.last_seen_at = now;
            state.samples += 1;
            return Vec::new();
        }

        let previous_point = state.last_point;
        state.last_point = current_point;
        state.last_seen_at = now;
        state.samples += 1;

        if state.samples < self.min_direction_samples {
            return Vec::new();
        }

        let movement = match Self::movement_direction(state.first_point, current_point) {
            Some(v) => v,
            None => return Vec::new(),
        };

        let dot = movement.0 * allowed.0 + movement.1 * allowed.1;
        let moved_this_step = Self::movement_direction(previous_point, current_point).is_some();

        if moved_this_step && dot < -0.2 {
            state.fired = true;
            return vec![self.make_event(obs, movement, dot)];
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::track_observations::test_support::observation;
    use crate::violations::coordinator::RuleRuntimeConfig;
    use uuid::Uuid;
    fn rule() -> WrongWayRule {
        WrongWayRule::from_embedded(
            "zone-a".to_string(),
            vec![[0, 0], [100, 0], [100, 100], [0, 100]],
            [0.0, -1.0],
            3,
            RuleRuntimeConfig {
                state_ttl_sec: 7.0,
                cleanup_interval_sec: 1.0,
            },
        )
    }

    #[test]
    fn sample_threshold_is_not_off_by_one() {
        let mut rule = rule();
        let track = Uuid::new_v4();

        assert!(
            rule.process_frame(&[observation(track, 0.0, 50.0, 50.0, Some("zone-a"), None)])
                .is_empty()
        );
        // Second observation: two samples so far, threshold is three.
        assert!(
            rule.process_frame(&[observation(track, 1.0, 50.0, 60.0, Some("zone-a"), None)])
                .is_empty()
        );
        // Third observation: movement from the first point is opposite to
        // the allowed [0, -1] direction.
        assert_eq!(
            rule.process_frame(&[observation(track, 2.0, 50.0, 70.0, Some("zone-a"), None)])
                .len(),
            1
        );
    }

    #[test]
    fn invalid_time_does_not_clear_track_state() {
        let mut rule = rule();
        let track = Uuid::new_v4();

        assert!(
            rule.process_frame(&[observation(track, 0.0, 50.0, 50.0, Some("zone-a"), None)])
                .is_empty()
        );
        assert_eq!(rule.tracks.len(), 1);

        assert!(
            rule.process_frame(&[observation(
                track,
                f32::NAN,
                50.0,
                60.0,
                Some("zone-a"),
                None
            )])
            .is_empty()
        );
        assert_eq!(rule.tracks.len(), 1);
    }
}
