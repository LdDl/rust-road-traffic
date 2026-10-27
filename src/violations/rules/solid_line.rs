use std::collections::HashMap;

use uuid::Uuid;

use crate::lib::track_observations::{PointPx, TrackObservation};
use crate::settings::LaneSolidLineRuleSettings;
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::{ViolationEvent, ViolationStatus};
use crate::violations::rules::ViolationRule;

#[derive(Debug, Clone)]
struct TrackState {
    last_anchor: PointPx,
    last_side: i32,
    last_seen_at: f32,
    fired: bool,
}

pub struct SolidLineRule {
    rule_id: String,
    zone_id: Option<String>,
    line_start: PointPx,
    line_end: PointPx,
    margin_px: f32,
    tracks: HashMap<Uuid, TrackState>,

    state_ttl_sec: f32,
    cleanup_interval_sec: f32,
    next_cleanup_at: f32,
}

impl SolidLineRule {
    /// Creates a solid-line crossing rule from the configured endpoints,
    /// side-classification threshold, and runtime cleanup settings.
    pub fn from_embedded(
        zone_id: String,
        settings: LaneSolidLineRuleSettings,
        runtime: RuleRuntimeConfig,
    ) -> Self {
        Self {
            rule_id: format!("{zone_id}.solid_line"),
            zone_id: Some(zone_id),
            line_start: PointPx {
                x: settings.line[0][0],
                y: settings.line[0][1],
            },
            line_end: PointPx {
                x: settings.line[1][0],
                y: settings.line[1][1],
            },
            margin_px: settings.margin_px,
            tracks: HashMap::new(),
            state_ttl_sec: runtime.state_ttl_sec,
            cleanup_interval_sec: runtime.cleanup_interval_sec,
            next_cleanup_at: 0.0,
        }
    }

    /// Returns the signed 2D cross product of vectors AB and AC,
    /// equivalent to twice the signed area of triangle ABC.
    /// Its sign identifies the side of the directed line AB containing C.
    fn signed_area2(a: &PointPx, b: &PointPx, c: &PointPx) -> f32 {
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }

    /// Classifies a point relative to the configured directed line.
    /// Returns 1 or -1 for opposite sides and 0 within the neutral band.
    ///
    /// The current implementation compares the raw cross product directly
    /// with margin_px; it does not normalize it to a pixel distance.
    fn classify_side(&self, p: &PointPx) -> i32 {
        let value = Self::signed_area2(&self.line_start, &self.line_end, p);
        if value > self.margin_px {
            1
        } else if value < -self.margin_px {
            -1
        } else {
            0
        }
    }

    /// Returns the axis-aligned bounding box of segment AB
    /// as (min_x, min_y, max_x, max_y).
    fn bbox_for_segment(a: &PointPx, b: &PointPx) -> (f32, f32, f32, f32) {
        (a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
    }

    /// Checks whether the bounding boxes of segments AB and CD overlap.
    /// Touching box boundaries are treated as overlapping.
    fn boxes_overlap(a: &PointPx, b: &PointPx, c: &PointPx, d: &PointPx) -> bool {
        let (aminx, aminy, amaxx, amaxy) = Self::bbox_for_segment(a, b);
        let (cminx, cminy, cmaxx, cmaxy) = Self::bbox_for_segment(c, d);

        !(amaxx < cminx || cmaxx < aminx || amaxy < cminy || cmaxy < aminy)
    }

    /// Checks whether P lies within the coordinate bounds of segment AB,
    /// allowing a small tolerance. The caller must check collinearity.
    fn on_segment(a: &PointPx, b: &PointPx, p: &PointPx) -> bool {
        let eps = 1e-3_f32;
        p.x >= a.x.min(b.x) - eps
            && p.x <= a.x.max(b.x) + eps
            && p.y >= a.y.min(b.y) - eps
            && p.y <= a.y.max(b.y) + eps
    }

    /// Checks whether finite segments AB and CD intersect.
    /// Handles proper crossings, endpoint contact, and collinear overlap
    /// using bounding-box rejection and cross-product tests.
    fn segments_intersect(a: &PointPx, b: &PointPx, c: &PointPx, d: &PointPx) -> bool {
        if !Self::boxes_overlap(a, b, c, d) {
            return false;
        }

        let ab_c = Self::signed_area2(a, b, c);
        let ab_d = Self::signed_area2(a, b, d);
        let cd_a = Self::signed_area2(c, d, a);
        let cd_b = Self::signed_area2(c, d, b);

        let eps = 1e-3_f32;

        if ab_c.abs() <= eps && Self::on_segment(a, b, c) {
            return true;
        }
        if ab_d.abs() <= eps && Self::on_segment(a, b, d) {
            return true;
        }
        if cd_a.abs() <= eps && Self::on_segment(c, d, a) {
            return true;
        }
        if cd_b.abs() <= eps && Self::on_segment(c, d, b) {
            return true;
        }

        let ab_cross = (ab_c > 0.0 && ab_d < 0.0) || (ab_c < 0.0 && ab_d > 0.0);
        let cd_cross = (cd_a > 0.0 && cd_b < 0.0) || (cd_a < 0.0 && cd_b > 0.0);

        ab_cross && cd_cross
    }

    /// Checks whether the observation belongs to the configured zone.
    /// A rule without a zone restriction accepts observations from any zone.
    fn zone_matches(&self, obs: &TrackObservation) -> bool {
        match (&self.zone_id, &obs.zone_id) {
            (Some(expected), Some(actual)) => expected == actual,
            (Some(_), None) => false,
            (None, _) => true,
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

    /// Builds a confirmed solid-line crossing event using the current
    /// observation and the supplied crossing details.
    fn make_event(&self, obs: &TrackObservation, details: String) -> ViolationEvent {
        ViolationEvent {
            event_id: Uuid::new_v4(),
            event_type: "traffic.solid_line_crossing".to_string(),
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
            zone_id: obs.zone_id.clone(),
            previous_zone_id: obs.previous_zone_id.clone(),
            crossed_virtual_line: obs.crossed_virtual_line,
            speed_kmh: obs.speed_kmh,
            rule_id: self.rule_id.clone(),
            details,
        }
    }
}

impl ViolationRule for SolidLineRule {
    /// Processes a track observation and checks whether the movement
    /// from its previous anchor to its current anchor crosses the
    /// configured segment and changes between opposite non-neutral sides.
    ///
    /// Expired state is discarded before comparing trajectory points.
    /// Unreported history is reset on a zone mismatch or invalid anchor.
    /// Already reported tracks remain suppressed while observations
    /// keep their state alive, including observations from other zones.
    fn process(&mut self, obs: &TrackObservation) -> Vec<ViolationEvent> {
        let now = obs.source_time_relative_sec;

        if !now.is_finite() {
            return Vec::new();
        }

        self.cleanup_if_needed(now);

        // Do not connect a new observation to stale trajectory history.
        let expired = self.tracks.get(&obs.track_id).map_or(false, |state| {
            let age = now - state.last_seen_at;
            age < 0.0 || age > self.state_ttl_sec
        });

        if expired {
            self.tracks.remove(&obs.track_id);
        }

        // Preserve deduplication for an already reported track,
        // including observations from another zone.
        if let Some(state) = self.tracks.get_mut(&obs.track_id) {
            state.last_seen_at = now;

            if state.fired {
                return Vec::new();
            }
        }

        // Unreported trajectory history must not bridge different zones.
        if !self.zone_matches(obs) {
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        let current_anchor = obs.anchor_px.clone();

        if !current_anchor.x.is_finite() || !current_anchor.y.is_finite() {
            // Break unreported trajectory history on invalid coordinates.
            self.tracks.remove(&obs.track_id);
            return Vec::new();
        }

        let current_side = self.classify_side(&current_anchor);

        let state = self.tracks.entry(obs.track_id).or_insert_with(|| {
            TrackState {
                last_anchor: current_anchor.clone(),
                last_side: current_side,
                last_seen_at: now,
                fired: false,
            }
        });

        let prev_anchor = state.last_anchor.clone();
        let prev_side = state.last_side;

        let crossed_segment = Self::segments_intersect(
            &prev_anchor,
            &current_anchor,
            &self.line_start,
            &self.line_end,
        );

        let changed_side =
            prev_side != 0 && current_side != 0 && prev_side != current_side;

        let should_emit = crossed_segment && changed_side;

        state.last_anchor = current_anchor;
        state.last_side = current_side;
        state.last_seen_at = now;

        if should_emit {
            state.fired = true;

            return vec![self.make_event(
                obs,
                format!(
                    "track crossed configured solid line segment: side {} -> {}",
                    prev_side, current_side
                ),
            )];
        }

        Vec::new()
    }
}