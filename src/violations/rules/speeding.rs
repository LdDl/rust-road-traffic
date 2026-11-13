use crate::lib::spatial::Point2f;
use crate::lib::track_observations::TrackObservation;
use crate::lib::zones::geometry::polygon_contains_point;
use crate::settings::LaneSpeedingRuleSettings;
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::ViolationEvent;
use crate::violations::rules::ViolationRule;
use crate::violations::rules::state::{RuleStateStore, TrackStateEntry};

/// The store value for a reported track: when it was last seen speeding.
#[derive(Debug, Clone)]
struct LastSeenAt(f32);

impl TrackStateEntry for LastSeenAt {
    fn last_seen_at(&self) -> f32 {
        self.0
    }
}

pub struct SpeedingRule {
    rule_id: String,
    zone_id: String,
    polygon_px: Vec<Point2f>,
    threshold_kmh: f32,
    fired_tracks: RuleStateStore<LastSeenAt>,
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
                .map(|pt| Point2f::new(pt[0] as f32, pt[1] as f32))
                .collect(),
            threshold_kmh: settings.threshold_kmh,
            fired_tracks: RuleStateStore::new(runtime.state_ttl_sec, runtime.cleanup_interval_sec),
        }
    }

    /// Builds a confirmed speeding event from the current observation
    /// and the speed that exceeded the configured threshold.
    fn make_event(&self, obs: &TrackObservation, speed_kmh: f32) -> ViolationEvent {
        let mut event = ViolationEvent::from_observation(
            obs,
            "traffic.speeding",
            Some(self.zone_id.clone()),
            self.rule_id.clone(),
            format!(
                "track exceeded speed threshold inside zone '{}' ({:.2} km/h > {:.2} km/h)",
                self.zone_id, speed_kmh, self.threshold_kmh,
            ),
        );
        event.speed_kmh = Some(speed_kmh);
        event
    }
}

impl ViolationRule for SpeedingRule {
    fn process_frame(&mut self, observations: &[TrackObservation]) -> Vec<ViolationEvent> {
        let mut events = Vec::new();
        for obs in observations {
            events.extend(self.process(obs));
        }
        events
    }
}

impl SpeedingRule {
    /// Processes a track observation and emits an event when its image point
    /// (the same point zone matching used to assign `zone_id`) is inside the
    /// configured polygon and its valid speed strictly exceeds the configured
    /// threshold.
    ///
    /// Only observations from this rule's zone are considered. The
    /// deduplication record is kept while the track stays in the zone and is
    /// dropped when it leaves the zone or the polygon, so a later visit can
    /// be reported again.
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let now = obs.source_time_relative_sec;

        if !now.is_finite() {
            return Vec::new();
        }

        self.fired_tracks.cleanup_if_needed(now);

        // Expire this track immediately, even between full cleanup passes.
        self.fired_tracks.remove_expired(&obs.track_id, now);

        // A rule only applies to observations from its own zone. Otherwise an
        // observation from an overlapping or stale zone could be reported
        // against the wrong zone.
        if obs.zone_id.as_deref() != Some(self.zone_id.as_str()) {
            self.fired_tracks.remove(&obs.track_id);
            return Vec::new();
        }

        if !polygon_contains_point(&self.polygon_px, obs.image_point_px.x, obs.image_point_px.y) {
            self.fired_tracks.remove(&obs.track_id);
            return Vec::new();
        }

        let speed_kmh = match obs.speed_kmh {
            Some(speed) if speed.is_finite() && speed >= 0.0 => speed,
            _ => return Vec::new(),
        };

        if speed_kmh <= self.threshold_kmh {
            return Vec::new();
        }

        // Suppress repeats while the same speeding episode continues.
        match self.fired_tracks.entry(obs.track_id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().0 = now;
                return Vec::new();
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(LastSeenAt(now));
            }
        }

        vec![self.make_event(obs, speed_kmh)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::track_observations::test_support::observation;
    use crate::settings::LaneSpeedingRuleSettings;
    use crate::violations::coordinator::RuleRuntimeConfig;
    use uuid::Uuid;
    fn rule() -> SpeedingRule {
        SpeedingRule::from_embedded(
            "zone-a".to_string(),
            vec![[0, 0], [100, 0], [100, 100], [0, 100]],
            LaneSpeedingRuleSettings {
                threshold_kmh: 50.0,
            },
            RuleRuntimeConfig {
                state_ttl_sec: 7.0,
                cleanup_interval_sec: 1.0,
            },
        )
    }

    #[test]
    fn the_image_point_decides_whether_the_track_is_in_the_zone() {
        use crate::lib::cv::Rect;
        use crate::lib::spatial::Point2f;
        use crate::lib::track_observations::TrackObservation;
        use chrono::Utc;

        let mut speeding = rule();
        let track = Uuid::new_v4();

        let obs = |image_y: f32, anchor_y: f32| TrackObservation {
            equipment_id: "eq".to_string(),
            track_id: track,
            observed_at: Utc::now(),
            source_time_relative_sec: 1.0,
            class_name: "car".to_string(),
            confidence: 0.9,
            bbox: Rect::new(40, 40, 20, 20),
            anchor_px: Point2f::new(50.0, anchor_y),
            image_point_px: Point2f::new(50.0, image_y),
            projected_point: None,
            speed_kmh: Some(60.0),
            zone_id: Some("zone-a".to_string()),
            previous_zone_id: None,
            crossed_virtual_line: false,
            no_match_times: 0,
        };

        // The polygon is [0, 0]..[100, 100]: the image point inside fires
        // even when the anchor (bbox bottom center) is outside.
        assert_eq!(speeding.process_frame(&[obs(50.0, 150.0)]).len(), 1);

        // A fresh track whose image point is outside does not fire even
        // when the anchor is inside.
        let mut other_rule = rule();
        let other = Uuid::new_v4();
        let other_obs = |image_y: f32, anchor_y: f32| {
            let mut o = obs(image_y, anchor_y);
            o.track_id = other;
            o
        };
        assert!(
            other_rule
                .process_frame(&[other_obs(150.0, 50.0)])
                .is_empty()
        );
    }

    #[test]
    fn only_own_zone_and_new_speeding_episode() {
        let mut rule = rule();
        let track = Uuid::new_v4();

        // Another zone must not create an event even if its point is inside
        // this rule's polygon.
        assert!(
            rule.process_frame(&[observation(
                track,
                1.0,
                50.0,
                50.0,
                Some("zone-b"),
                Some(60.0)
            )])
            .is_empty()
        );
        // Own zone: first speeding event.
        assert_eq!(
            rule.process_frame(&[observation(
                track,
                1.1,
                50.0,
                50.0,
                Some("zone-a"),
                Some(60.0)
            )])
            .len(),
            1
        );
        // Still speeding in the same episode: suppressed.
        assert!(
            rule.process_frame(&[observation(
                track,
                1.2,
                50.0,
                50.0,
                Some("zone-a"),
                Some(60.0)
            )])
            .is_empty()
        );
        // Leaving the zone clears the deduplication record.
        assert!(
            rule.process_frame(&[observation(
                track,
                1.3,
                50.0,
                50.0,
                Some("zone-b"),
                Some(60.0)
            )])
            .is_empty()
        );
        // A later visit to the own zone can report again.
        assert_eq!(
            rule.process_frame(&[observation(
                track,
                1.4,
                50.0,
                50.0,
                Some("zone-a"),
                Some(60.0)
            )])
            .len(),
            1
        );
    }
}
