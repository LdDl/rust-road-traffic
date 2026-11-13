//! The one rule behind both `no_parking` and `no_stopping`: a track that
//! stays inside the zone polygon at or below the stationary speed threshold
//! for the configured duration is reported once, until it leaves the zone or
//! moves again.

use crate::lib::spatial::Point2f;
use crate::lib::track_observations::TrackObservation;
use crate::lib::zones::geometry::polygon_contains_point;
use crate::settings::LaneZoneViolationRuleSettings;
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::ViolationEvent;
use crate::violations::rules::ViolationRule;
use crate::violations::rules::state::{RuleStateStore, TrackStateEntry};

/// A stationary period must be observed continuously. A longer gap means the
/// track was not visible and the timer restarts.
const MAX_STATIONARY_OBSERVATION_GAP_SEC: f32 = 1.0;

/// Which configuration table the rule instance represents.
#[derive(Debug, Clone, Copy)]
pub enum StationaryRuleKind {
    NoParking,
    NoStopping,
}

impl StationaryRuleKind {
    fn event_type(&self) -> &'static str {
        match self {
            StationaryRuleKind::NoParking => "traffic.no_parking",
            StationaryRuleKind::NoStopping => "traffic.no_stopping",
        }
    }

    fn rule_suffix(&self) -> &'static str {
        match self {
            StationaryRuleKind::NoParking => "no_parking",
            StationaryRuleKind::NoStopping => "no_stopping",
        }
    }

    fn zone_kind(&self) -> &'static str {
        match self {
            StationaryRuleKind::NoParking => "no-parking zone",
            StationaryRuleKind::NoStopping => "no-stopping zone",
        }
    }
}

#[derive(Debug, Clone)]
struct TrackState {
    entered_at: f32,
    last_seen_at: f32,
    fired: bool,
}

impl TrackStateEntry for TrackState {
    fn last_seen_at(&self) -> f32 {
        self.last_seen_at
    }
}

pub struct StationaryZoneRule {
    rule_id: String,
    zone_id: String,
    polygon_px: Vec<Point2f>,
    min_duration_sec: f32,
    stationary_speed_kmh: f32,
    kind: StationaryRuleKind,

    tracks: RuleStateStore<TrackState>,
}

impl StationaryZoneRule {
    /// Creates a stationary rule from the zone geometry, rule settings,
    /// and runtime settings used to expire inactive track state.
    pub fn from_embedded(
        zone_id: String,
        geometry: Vec<[i32; 2]>,
        settings: LaneZoneViolationRuleSettings,
        runtime: RuleRuntimeConfig,
        kind: StationaryRuleKind,
    ) -> Self {
        Self {
            rule_id: format!("{zone_id}.{}", kind.rule_suffix()),
            zone_id,
            polygon_px: geometry
                .into_iter()
                .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
                .collect(),
            min_duration_sec: settings.min_duration_sec,
            stationary_speed_kmh: settings.stationary_speed_kmh,
            kind,
            tracks: RuleStateStore::new(runtime.state_ttl_sec, runtime.cleanup_interval_sec),
        }
    }

    /// Builds a confirmed stationary event from the current observation
    /// and the elapsed stationary period.
    fn make_event(&self, obs: &TrackObservation, duration_sec: f32) -> ViolationEvent {
        ViolationEvent::from_observation(
            obs,
            self.kind.event_type(),
            Some(self.zone_id.clone()),
            self.rule_id.clone(),
            format!(
                "track remained stationary in {} '{}' for {:.2} sec (threshold {:.2} sec)",
                self.kind.zone_kind(),
                self.zone_id,
                duration_sec,
                self.min_duration_sec,
            ),
        )
    }
}

impl ViolationRule for StationaryZoneRule {
    fn process_frame(&mut self, observations: &[TrackObservation]) -> Vec<ViolationEvent> {
        let mut events = Vec::new();
        for obs in observations {
            events.extend(self.process(obs));
        }
        events
    }
}

impl StationaryZoneRule {
    /// Processes a track observation and emits an event once the track
    /// has remained inside the polygon at or below the stationary speed
    /// threshold for the configured duration.
    ///
    /// Only observations from this rule's zone are considered. Leaving the
    /// polygon, receiving an invalid or excessive speed, or a gap longer than
    /// `MAX_STATIONARY_OBSERVATION_GAP_SEC` resets the stationary timer.
    ///
    /// After an event is emitted, further events for the same track are
    /// suppressed while it remains stationary in this zone. Leaving the zone
    /// or moving again removes the state, so a later stop can be reported.
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let now = obs.source_time_relative_sec;

        if !now.is_finite() {
            return Vec::new();
        }

        self.tracks.cleanup_if_needed(now);

        // Expire this track even between full cleanup passes.
        self.tracks.remove_expired(&obs.track_id, now);

        // A rule only applies to observations from its own zone. Otherwise an
        // observation from an overlapping or stale zone could be reported
        // against the wrong zone.
        if obs.zone_id.as_deref() != Some(self.zone_id.as_str()) {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        if !polygon_contains_point(&self.polygon_px, obs.image_point_px.x, obs.image_point_px.y) {
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

        // A gap longer than a normal frame interval means the track was not
        // continuously visible; it must not make the stationary timer look
        // longer than the observation period really was.
        if let Some(state) = self.tracks.get_mut(&obs.track_id) {
            let gap = now - state.last_seen_at;
            state.last_seen_at = now;
            if state.fired {
                return Vec::new();
            }
            if gap > MAX_STATIONARY_OBSERVATION_GAP_SEC {
                state.entered_at = now;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::track_observations::test_support::observation;
    use crate::settings::LaneZoneViolationRuleSettings;
    use crate::violations::coordinator::RuleRuntimeConfig;
    use uuid::Uuid;

    const KINDS: [StationaryRuleKind; 2] = [
        StationaryRuleKind::NoParking,
        StationaryRuleKind::NoStopping,
    ];

    fn rule(kind: StationaryRuleKind) -> StationaryZoneRule {
        StationaryZoneRule::from_embedded(
            "zone-a".to_string(),
            vec![[0, 0], [100, 0], [100, 100], [0, 100]],
            LaneZoneViolationRuleSettings {
                min_duration_sec: 2.0,
                stationary_speed_kmh: 5.0,
            },
            RuleRuntimeConfig {
                state_ttl_sec: 7.0,
                cleanup_interval_sec: 1.0,
            },
            kind,
        )
    }

    #[test]
    fn only_own_zone_and_new_stop_after_leaving() {
        for kind in KINDS {
            let mut rule = rule(kind);
            let track = Uuid::new_v4();

            for time in [0.0, 0.5, 1.0, 1.5] {
                assert!(
                    rule.process_frame(&[observation(
                        track,
                        time,
                        50.0,
                        50.0,
                        Some("zone-a"),
                        Some(0.0)
                    )])
                    .is_empty(),
                    "{kind:?} time {time}"
                );
            }
            assert_eq!(
                rule.process_frame(&[observation(
                    track,
                    2.0,
                    50.0,
                    50.0,
                    Some("zone-a"),
                    Some(0.0)
                )])
                .len(),
                1,
                "{kind:?}"
            );
            // Moving above the stationary threshold clears the fired state.
            assert!(
                rule.process_frame(&[observation(
                    track,
                    2.1,
                    50.0,
                    50.0,
                    Some("zone-a"),
                    Some(20.0)
                )])
                .is_empty(),
                "{kind:?}"
            );
            // A later stop is reported again.
            for (time, expected) in [(2.3, 0), (2.8, 0), (3.3, 0), (3.8, 0), (4.4, 1)] {
                let events = rule.process_frame(&[observation(
                    track,
                    time,
                    50.0,
                    50.0,
                    Some("zone-a"),
                    Some(0.0),
                )]);
                assert_eq!(events.len(), expected, "{kind:?} time {time}");
            }
        }
    }

    #[test]
    fn still_stationary_is_suppressed_and_another_zone_clears_state() {
        for kind in KINDS {
            let mut rule = rule(kind);
            let track = Uuid::new_v4();

            for time in [0.0, 0.5, 1.0, 1.5] {
                assert!(
                    rule.process_frame(&[observation(
                        track,
                        time,
                        50.0,
                        50.0,
                        Some("zone-a"),
                        Some(0.0)
                    )])
                    .is_empty(),
                    "{kind:?} time {time}"
                );
            }
            assert_eq!(
                rule.process_frame(&[observation(
                    track,
                    2.0,
                    50.0,
                    50.0,
                    Some("zone-a"),
                    Some(0.0)
                )])
                .len(),
                1,
                "{kind:?}"
            );
            // Still stationary in the same stop: suppressed.
            assert!(
                rule.process_frame(&[observation(
                    track,
                    2.1,
                    50.0,
                    50.0,
                    Some("zone-a"),
                    Some(0.0)
                )])
                .is_empty(),
                "{kind:?}"
            );
            // Another zone, even with a point inside this rule's polygon, clears
            // the state instead of keeping the fired flag alive.
            assert!(
                rule.process_frame(&[observation(
                    track,
                    2.2,
                    50.0,
                    50.0,
                    Some("zone-b"),
                    Some(0.0)
                )])
                .is_empty(),
                "{kind:?}"
            );

            // A later stop in the own zone is a new event.
            for (time, expected) in [(2.3, 0), (2.8, 0), (3.3, 0), (3.8, 0), (4.4, 1)] {
                let events = rule.process_frame(&[observation(
                    track,
                    time,
                    50.0,
                    50.0,
                    Some("zone-a"),
                    Some(0.0),
                )]);
                assert_eq!(events.len(), expected, "{kind:?} time {time}");
            }
        }
    }

    #[test]
    fn a_long_observation_gap_restarts_the_timer() {
        for kind in KINDS {
            let mut rule = rule(kind);
            let track = Uuid::new_v4();

            for time in [0.0, 0.5] {
                assert!(
                    rule.process_frame(&[observation(
                        track,
                        time,
                        50.0,
                        50.0,
                        Some("zone-a"),
                        Some(0.0)
                    )])
                    .is_empty(),
                    "{kind:?} time {time}"
                );
            }
            // The track disappears for 4.5 s. The next observation must start a
            // new stationary period rather than inherit the old entered_at.
            for (time, expected) in [(5.0, 0), (5.5, 0), (6.0, 0), (6.5, 0), (7.0, 1)] {
                let events = rule.process_frame(&[observation(
                    track,
                    time,
                    50.0,
                    50.0,
                    Some("zone-a"),
                    Some(0.0),
                )]);
                assert_eq!(events.len(), expected, "{kind:?} time {time}");
            }
        }
    }
}
