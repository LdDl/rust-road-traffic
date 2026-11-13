//! Solid-line crossing detection as a violation rule.
//!
//! A solid line is configured inside a zone (`rules.solid_line`), but the
//! vehicle that crosses it does not have to be inside that zone's polygon, or
//! inside any zone at all. The rule is therefore fed every observation of the
//! frame and owns its own, zone-independent, crossing state.
//!
//! One rule instance holds exactly one line, built by the coordinator from
//! the live zone. When the line is moved through the REST API the coordinator
//! replaces the rule, which resets that line's crossing state exactly like
//! the per-line reset of the previous detector.

use std::collections::hash_map::Entry;

use crate::lib::cv::Rect;
use crate::lib::spatial::{Aabb, Point2f};
use crate::lib::track_observations::TrackObservation;
use crate::lib::zones::SolidLine;
use crate::lib::zones::geometry::{
    is_intersects, line_signed_distance, segment_intersection_point, segment_intersects_rect,
};
use crate::violations::coordinator::RuleRuntimeConfig;
use crate::violations::event::ViolationEvent;
use crate::violations::rules::ViolationRule;
use crate::violations::rules::state::{RuleStateStore, TrackStateEntry};

/// Side of a cell in the solid-line bucket grid, in pixels. Kept from the
/// previous detector: the candidate test below uses the same cell lattice, so
/// a track near a line is evaluated the same way it used to be.
pub const SOLID_LINE_GRID_CELL_PX: f32 = 64.0;

/// A line spanning more cells than this used to be always checked.
const MAX_CELLS_PER_LINE: usize = 256;

/// A query spanning more cells than this used to fall back to checking
/// everything.
const MAX_CELLS_PER_QUERY: usize = 4096;

/// One configured solid line together with the zone it belongs to.
///
/// The line does not have to be geometrically inside `zone_id`; the zone is
/// used to keep the configuration and to attribute the crossing.
#[derive(Debug, Clone)]
pub struct SolidLineDefinition {
    /// Stable identifier of the line, also written into the violation.
    pub id: String,
    /// Zone this line is configured in. `None` is allowed for a line that has
    /// no zone at all; such a crossing is reported without a zone.
    pub zone_id: Option<String>,
    pub line: SolidLine,
}

impl SolidLineDefinition {
    pub fn new(id: String, zone_id: Option<String>, line: SolidLine) -> Self {
        SolidLineDefinition { id, zone_id, line }
    }

    pub fn from_zone(zone_id: String, line: SolidLine) -> Self {
        Self::new(format!("{zone_id}.solid_line"), Some(zone_id), line)
    }

    fn margin(&self) -> f32 {
        if self.line.margin_px.is_finite() {
            self.line.margin_px.max(0.0)
        } else {
            0.0
        }
    }

    fn endpoints(&self) -> (Point2f, Point2f) {
        (
            Point2f::new(self.line.line[0][0], self.line.line[0][1]),
            Point2f::new(self.line.line[1][0], self.line.line[1][1]),
        )
    }

    /// AABB of the segment expanded by `margin_px` on every side.
    pub fn expanded_aabb(&self) -> Aabb {
        let (a, b) = self.endpoints();
        Aabb::new(a.x, a.y, b.x, b.y).expanded(self.margin())
    }

    /// Fast AABB prefilter followed by the exact segment-rectangle test.
    /// `margin_px` widens the rectangle as well, because the margin is a
    /// corridor around the line rather than a property of the box.
    pub fn intersects_bbox(&self, bbox: &Rect) -> bool {
        let bbox_aabb = Aabb::from_rect(bbox);
        if !self.expanded_aabb().intersects(&bbox_aabb) {
            return false;
        }
        let (a, b) = self.endpoints();
        segment_intersects_rect(a, b, &bbox_aabb.expanded(self.margin()))
    }

    /// Signed perpendicular distance from the line through its endpoints, in
    /// pixels. Positive and negative values are opposite sides.
    fn signed_distance(&self, point: Point2f) -> f32 {
        let (a, b) = self.endpoints();
        line_signed_distance(a, b, point)
    }

    /// 1 and -1 are opposite sides, 0 is the neutral band of half-width
    /// `margin_px` around the line.
    fn classify_side(&self, point: Point2f) -> i32 {
        let distance = self.signed_distance(point);
        let margin = self.margin();
        if distance > margin {
            1
        } else if distance < -margin {
            -1
        } else {
            0
        }
    }
}

#[derive(Debug, Clone)]
struct TrackState {
    /// Last centre that was outside the neutral `margin_px` band. A vehicle
    /// crossing the band over several frames still has a point on each side to
    /// compare, so a thick margin does not hide the crossing.
    last_non_neutral_center: Point2f,
    last_non_neutral_side: i32,
    last_seen_at: f32,
    fired: bool,
}

impl TrackStateEntry for TrackState {
    fn last_seen_at(&self) -> f32 {
        self.last_seen_at
    }
}

/// Where the centre of each track was on the previous frame: the swept area
/// between the two catches a fast crossing that never sampled the line.
#[derive(Debug, Clone)]
struct LastCenter {
    point: Point2f,
    last_seen_at: f32,
}

impl TrackStateEntry for LastCenter {
    fn last_seen_at(&self) -> f32 {
        self.last_seen_at
    }
}

/// Zone-independent solid-line rule for one configured line.
///
/// State is kept per track so the same crossing is reported once; the rule is
/// replaced (state reset) when the coordinator sees the line changed, and
/// stale entries are removed after `state_ttl_sec`.
pub struct SolidLineRule {
    line: SolidLineDefinition,
    tracks: RuleStateStore<TrackState>,
    last_centers: RuleStateStore<LastCenter>,
}

impl SolidLineRule {
    pub fn new(line: SolidLineDefinition, runtime: RuleRuntimeConfig) -> Self {
        let state_ttl_sec = if runtime.state_ttl_sec.is_finite() {
            runtime.state_ttl_sec.max(0.0)
        } else {
            0.0
        };
        let cleanup_interval_sec = if runtime.cleanup_interval_sec.is_finite() {
            runtime.cleanup_interval_sec.max(0.0)
        } else {
            0.0
        };
        SolidLineRule {
            line,
            tracks: RuleStateStore::new(state_ttl_sec, cleanup_interval_sec),
            last_centers: RuleStateStore::new(state_ttl_sec, cleanup_interval_sec),
        }
    }

    /// Evaluates one observation against the line and returns the crossing
    /// event, when this observation completes one.
    fn process_observation(
        &mut self,
        obs: &TrackObservation,
        center: Point2f,
        now: f32,
    ) -> Option<ViolationEvent> {
        if !self.line.line.is_valid() {
            return None;
        }

        let side = self.line.classify_side(center);
        let touches = self.line.intersects_bbox(&obs.bbox);
        let (line_start, line_end) = self.line.endpoints();

        self.tracks.remove_expired(&obs.track_id, now);

        let state = self.tracks.entry(obs.track_id).or_insert(TrackState {
            last_non_neutral_center: center,
            last_non_neutral_side: side,
            last_seen_at: now,
            fired: false,
        });
        state.last_seen_at = now;

        if state.fired {
            if side != 0 {
                state.last_non_neutral_center = center;
                state.last_non_neutral_side = side;
            }
            return None;
        }

        // Compare against the last point outside the neutral band, not against
        // the immediately previous frame: with a wide margin the previous side
        // is often 0, and the crossing would otherwise be lost.
        let previous_side = state.last_non_neutral_side;
        let previous_center = state.last_non_neutral_center;
        let changed_side = previous_side != 0 && side != 0 && previous_side != side;
        let path_crosses = changed_side
            && is_intersects(
                previous_center.x,
                previous_center.y,
                center.x,
                center.y,
                line_start.x,
                line_start.y,
                line_end.x,
                line_end.y,
            );
        let crossed = changed_side && (touches || path_crosses);

        if side != 0 {
            state.last_non_neutral_center = center;
            state.last_non_neutral_side = side;
        }

        if !crossed {
            return None;
        }
        state.fired = true;

        let intersection =
            segment_intersection_point(previous_center, center, line_start, line_end)
                .unwrap_or(center);

        Some(self.make_event(obs, previous_side, side, intersection))
    }

    /// Whether the swept query may touch the line: the same cell-lattice test
    /// the bucket grid of the previous detector applied, ported for a single
    /// line. It is deliberately looser than an exact AABB intersection: a
    /// track near the line establishes its side baseline before the box
    /// actually reaches the line, which is what the crossing detection
    /// depends on.
    fn query_may_touch(&self, query: &Aabb) -> bool {
        if !query.is_finite() {
            return false;
        }
        let Some((query_min_cx, query_max_cx)) =
            cell_range(query.min_x, query.max_x, SOLID_LINE_GRID_CELL_PX)
        else {
            return false;
        };
        let Some((query_min_cy, query_max_cy)) =
            cell_range(query.min_y, query.max_y, SOLID_LINE_GRID_CELL_PX)
        else {
            return false;
        };

        let span_x = (query_max_cx - query_min_cx) as i64 + 1;
        let span_y = (query_max_cy - query_min_cy) as i64 + 1;
        if span_x > MAX_CELLS_PER_QUERY as i64
            || span_y > MAX_CELLS_PER_QUERY as i64
            || span_x.saturating_mul(span_y) > MAX_CELLS_PER_QUERY as i64
        {
            // A gigantic query was answered by looking at everything.
            return true;
        }

        let line_aabb = self.line.expanded_aabb();
        if !line_aabb.is_finite() {
            // A non-finite line went into the always-checked list.
            return true;
        }
        let Some((line_min_cx, line_max_cx)) =
            cell_range(line_aabb.min_x, line_aabb.max_x, SOLID_LINE_GRID_CELL_PX)
        else {
            return true;
        };
        let Some((line_min_cy, line_max_cy)) =
            cell_range(line_aabb.min_y, line_aabb.max_y, SOLID_LINE_GRID_CELL_PX)
        else {
            return true;
        };
        let line_span_x = (line_max_cx - line_min_cx) as i64 + 1;
        let line_span_y = (line_max_cy - line_min_cy) as i64 + 1;
        if line_span_x > MAX_CELLS_PER_LINE as i64
            || line_span_y > MAX_CELLS_PER_LINE as i64
            || line_span_x.saturating_mul(line_span_y) > MAX_CELLS_PER_LINE as i64
        {
            // A huge line was always checked.
            return true;
        }

        ranges_overlap(query_min_cx, query_max_cx, line_min_cx, line_max_cx)
            && ranges_overlap(query_min_cy, query_max_cy, line_min_cy, line_max_cy)
    }

    fn make_event(
        &self,
        obs: &TrackObservation,
        previous_side: i32,
        side: i32,
        intersection: Point2f,
    ) -> ViolationEvent {
        ViolationEvent::from_observation(
            obs,
            "traffic.solid_line_crossing",
            self.line.zone_id.clone(),
            self.line.id.clone(),
            format!(
                "track bbox crossed solid line '{}': side {} -> {}",
                self.line.id, previous_side, side
            ),
        )
        .for_solid_line(self.line.id.clone(), intersection)
    }

    /// Test hook: how many track states are alive right now.
    #[cfg(test)]
    fn state_count(&self) -> usize {
        self.tracks.len()
    }
}

/// Converts a coordinate span into an inclusive cell range. `None` means the
/// span is not finite or does not fit into `i32` cell coordinates.
fn cell_range(min: f32, max: f32, cell_size: f32) -> Option<(i32, i32)> {
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    let min_cell = (min / cell_size).floor();
    let max_cell = (max / cell_size).floor();
    if !min_cell.is_finite() || !max_cell.is_finite() {
        return None;
    }
    if min_cell < i32::MIN as f32 || max_cell > i32::MAX as f32 {
        return None;
    }
    Some((min_cell as i32, max_cell as i32))
}

/// Whether two inclusive cell ranges share at least one cell.
fn ranges_overlap(a_min: i32, a_max: i32, b_min: i32, b_max: i32) -> bool {
    a_min <= b_max && b_min <= a_max
}

impl ViolationRule for SolidLineRule {
    /// Evaluates every observation of the frame against the line.
    ///
    /// The returned events belong to the frame; the coordinator publishes them.
    fn process_frame(&mut self, observations: &[TrackObservation]) -> Vec<ViolationEvent> {
        let now = observations
            .iter()
            .map(|obs| obs.source_time_relative_sec)
            .filter(|time| time.is_finite())
            .fold(f32::NEG_INFINITY, f32::max);
        if now.is_finite() {
            self.tracks.cleanup_if_needed(now);
            self.last_centers.cleanup_if_needed(now);
        }

        let mut events = Vec::new();
        for obs in observations {
            let now = obs.source_time_relative_sec;
            if !now.is_finite() {
                continue;
            }

            let bbox = Aabb::from_rect(&obs.bbox);
            if !bbox.is_finite() {
                continue;
            }
            let center = bbox.center();

            // A fast track can jump from one side of the line to the other
            // between two processed frames without touching the line in either
            // sampled box. The swept area still crosses it, and the path
            // crossing check catches that; a track that stayed far away keeps
            // its state untouched, so it can age out by the TTL.
            let query = match self.last_centers.get(&obs.track_id) {
                Some(previous) => bbox.union(&Aabb::point(previous.point)),
                None => bbox,
            };

            if self.query_may_touch(&query) {
                if let Some(event) = self.process_observation(obs, center, now) {
                    events.push(event);
                }
            }

            match self.last_centers.entry(obs.track_id) {
                Entry::Occupied(mut entry) => {
                    entry.get_mut().point = center;
                    entry.get_mut().last_seen_at = now;
                }
                Entry::Vacant(entry) => {
                    entry.insert(LastCenter {
                        point: center,
                        last_seen_at: now,
                    });
                }
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::cv::Rect;
    use crate::lib::spatial::Point2f;
    use chrono::Utc;
    use uuid::Uuid;

    fn observation(
        track_id: Uuid,
        time: f32,
        center_x: f32,
        center_y: f32,
        width: i32,
        height: i32,
        zone_id: Option<&str>,
    ) -> TrackObservation {
        let left = (center_x - width as f32 / 2.0).round() as i32;
        let top = (center_y - height as f32 / 2.0).round() as i32;
        TrackObservation {
            equipment_id: "eq".to_string(),
            track_id,
            observed_at: Utc::now(),
            source_time_relative_sec: time,
            class_name: "car".to_string(),
            confidence: 0.9,
            bbox: Rect::new(left, top, width, height),
            anchor_px: Point2f::new(center_x, center_y + height as f32 / 2.0),
            image_point_px: Point2f::new(center_x, center_y),
            projected_point: None,
            speed_kmh: None,
            zone_id: zone_id.map(str::to_string),
            previous_zone_id: None,
            crossed_virtual_line: false,
            no_match_times: 0,
        }
    }

    fn horizontal_line(identifier: &str, y: f32, margin: f32) -> SolidLineDefinition {
        SolidLineDefinition::new(
            identifier.to_string(),
            Some("zone-1".to_string()),
            SolidLine::new([[0.0, y], [200.0, y]], margin),
        )
    }

    fn runtime() -> RuleRuntimeConfig {
        RuleRuntimeConfig {
            state_ttl_sec: 10.0,
            cleanup_interval_sec: 1.0,
        }
    }

    fn rule(line: SolidLineDefinition) -> SolidLineRule {
        SolidLineRule::new(line, runtime())
    }

    #[test]
    fn line_bbox_intersection_respects_margin() {
        let bbox = Rect::new(50, 85, 20, 10);
        let with_margin = horizontal_line("line", 100.0, 10.0);
        let without_margin = horizontal_line("line", 100.0, 0.0);

        assert!(with_margin.intersects_bbox(&bbox));
        assert!(!without_margin.intersects_bbox(&bbox));
        assert!(!with_margin.intersects_bbox(&Rect::new(300, 85, 20, 10)));
    }

    #[test]
    fn crossing_is_detected_outside_any_zone() {
        let line = horizontal_line("zone-1.solid_line", 100.0, 5.0);
        let track = Uuid::new_v4();
        let mut rule = rule(line);

        let before = observation(track, 1.0, 100.0, 80.0, 20, 20, None);
        assert!(rule.process_frame(&[before]).is_empty());

        let after = observation(track, 1.1, 100.0, 120.0, 20, 20, None);
        let events = rule.process_frame(&[after]);

        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.solid_line_id.as_deref(), Some("zone-1.solid_line"));
        assert_eq!(event.zone_id.as_deref(), Some("zone-1"));
        assert_eq!(event.track_id, track);
        let intersection = event.intersection_px.as_ref().expect("intersection");
        assert!((intersection.y - 100.0).abs() < 1.0, "{intersection:?}");
    }

    #[test]
    fn a_line_without_a_zone_reports_no_zone() {
        let line = SolidLineDefinition::new(
            "global.solid_line".to_string(),
            None,
            SolidLine::new([[0.0, 100.0], [200.0, 100.0]], 5.0),
        );
        let track = Uuid::new_v4();
        let mut rule = rule(line);

        rule.process_frame(&[observation(track, 1.0, 100.0, 80.0, 20, 20, None)]);
        let events = rule.process_frame(&[observation(track, 1.1, 100.0, 120.0, 20, 20, None)]);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].zone_id, None);
        assert_eq!(
            events[0].solid_line_id.as_deref(),
            Some("global.solid_line")
        );
    }

    #[test]
    fn one_track_reports_a_crossing_once() {
        let line = horizontal_line("line", 100.0, 5.0);
        let track = Uuid::new_v4();
        let mut rule = rule(line);

        rule.process_frame(&[observation(track, 1.0, 100.0, 80.0, 20, 20, None)]);
        assert_eq!(
            rule.process_frame(&[observation(track, 1.1, 100.0, 120.0, 20, 20, None)])
                .len(),
            1
        );
        // Going back over the same line must not produce a second event.
        assert!(
            rule.process_frame(&[observation(track, 1.2, 100.0, 80.0, 20, 20, None)])
                .is_empty()
        );
    }

    #[test]
    fn margin_can_keep_a_nearby_track_neutral() {
        let track = Uuid::new_v4();

        let wide = horizontal_line("wide", 100.0, 50.0);
        let mut wide_rule = rule(wide);
        wide_rule.process_frame(&[observation(track, 1.0, 100.0, 80.0, 20, 20, None)]);
        assert!(
            wide_rule
                .process_frame(&[observation(track, 1.1, 100.0, 120.0, 20, 20, None)])
                .is_empty()
        );

        let narrow = horizontal_line("narrow", 100.0, 5.0);
        let mut narrow_rule = rule(narrow);
        narrow_rule.process_frame(&[observation(track, 1.0, 100.0, 80.0, 20, 20, None)]);
        assert_eq!(
            narrow_rule
                .process_frame(&[observation(track, 1.1, 100.0, 120.0, 20, 20, None)])
                .len(),
            1
        );
    }

    #[test]
    fn crossing_through_the_margin_band_is_detected() {
        let line = horizontal_line("line", 100.0, 10.0);
        let track = Uuid::new_v4();
        let mut rule = rule(line);

        // Above the line, outside the band.
        rule.process_frame(&[observation(track, 1.0, 100.0, 80.0, 20, 20, None)]);
        // Inside the band: the side is 0 on both of these frames.
        rule.process_frame(&[observation(track, 1.1, 100.0, 95.0, 20, 20, None)]);
        rule.process_frame(&[observation(track, 1.2, 100.0, 105.0, 20, 20, None)]);
        // Out of the band on the other side. The comparison is against the
        // last non-neutral point, so this is one crossing.
        let events = rule.process_frame(&[observation(track, 1.3, 100.0, 120.0, 20, 20, None)]);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].solid_line_id.as_deref(), Some("line"));
    }

    #[test]
    fn moving_a_line_starts_a_fresh_rule_with_fresh_state() {
        let track = Uuid::new_v4();

        // The coordinator replaces the rule when the line moves: a new rule
        // instance carries no crossing state and no swept-path history, like
        // the previous detector's per-line reset of the crossing state.
        let mut before = rule(horizontal_line("line", 100.0, 5.0));
        before.process_frame(&[observation(track, 1.0, 100.0, 80.0, 20, 20, None)]);
        assert_eq!(
            before
                .process_frame(&[observation(track, 1.1, 100.0, 120.0, 20, 20, None)])
                .len(),
            1
        );
        assert_eq!(before.state_count(), 1);

        // New rule with the moved line: state starts empty.
        let mut after = rule(horizontal_line("line", 300.0, 5.0));
        assert_eq!(after.state_count(), 0);

        // The first frame near the moved line establishes the baseline side
        // (the track is above the line), with no event.
        assert!(
            after
                .process_frame(&[observation(track, 1.3, 100.0, 250.0, 20, 20, None)])
                .is_empty()
        );
        // Moving over it is a real crossing of the new geometry.
        assert_eq!(
            after
                .process_frame(&[observation(track, 1.4, 100.0, 400.0, 20, 20, None)])
                .len(),
            1
        );
    }
}
