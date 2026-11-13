use crate::lib::cv::Rect;
use crate::lib::spatial::Point2f;
use crate::lib::track_observations::TrackObservation;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ViolationStatus {
    Confirmed,
}

#[derive(Debug, Clone, Serialize)]
pub struct ViolationEvent {
    pub event_id: Uuid,
    pub event_type: String,
    pub status: ViolationStatus,

    pub equipment_id: String,
    pub track_id: Uuid,

    pub observed_at: DateTime<Utc>,
    pub source_time_relative_sec: f32,

    pub class_name: String,
    pub confidence: f32,

    pub bbox: Rect,
    pub anchor_px: Point2f,
    pub image_point_px: Point2f,
    pub projected_point: Option<Point2f>,

    pub zone_id: Option<String>,
    pub previous_zone_id: Option<String>,
    pub crossed_virtual_line: bool,

    pub speed_kmh: Option<f32>,
    /// Solid line that was crossed, when the event comes from one.
    /// Other rules leave it out of the JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solid_line_id: Option<String>,
    /// Point where the track crossed the solid line, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intersection_px: Option<Point2f>,
    pub rule_id: String,
    pub details: String,
}

impl ViolationEvent {
    /// Builds a confirmed event that mirrors the observation. `event_type` and
    /// `rule_id` identify the rule; `details` carries the rule's explanation.
    /// Rules override specific fields (for example `speed_kmh` or the solid
    /// line) after construction.
    pub fn from_observation(
        obs: &TrackObservation,
        event_type: &str,
        zone_id: Option<String>,
        rule_id: String,
        details: String,
    ) -> Self {
        ViolationEvent {
            event_id: Uuid::new_v4(),
            event_type: event_type.to_string(),
            status: ViolationStatus::Confirmed,
            equipment_id: obs.equipment_id.clone(),
            track_id: obs.track_id,
            observed_at: obs.observed_at,
            source_time_relative_sec: obs.source_time_relative_sec,
            class_name: obs.class_name.clone(),
            confidence: obs.confidence,
            bbox: obs.bbox,
            anchor_px: obs.anchor_px,
            image_point_px: obs.image_point_px,
            projected_point: obs.projected_point,
            zone_id,
            previous_zone_id: obs.previous_zone_id.clone(),
            crossed_virtual_line: obs.crossed_virtual_line,
            speed_kmh: obs.speed_kmh,
            solid_line_id: None,
            intersection_px: None,
            rule_id,
            details,
        }
    }

    /// Attaches the solid line the event comes from and the crossing point.
    pub fn for_solid_line(mut self, line_id: String, intersection: Point2f) -> Self {
        self.solid_line_id = Some(line_id);
        self.intersection_px = Some(intersection);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::cv::Rect;
    use crate::lib::spatial::Point2f;
    use crate::lib::track_observations::TrackObservation;
    use chrono::TimeZone;

    fn observation() -> TrackObservation {
        TrackObservation {
            equipment_id: "eq-1".to_string(),
            track_id: Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap(),
            observed_at: Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
            source_time_relative_sec: 12.5,
            class_name: "car".to_string(),
            confidence: 0.5,
            bbox: Rect::new(10, 20, 30, 40),
            anchor_px: Point2f::new(25.0, 60.0),
            image_point_px: Point2f::new(25.0, 40.0),
            projected_point: Some(Point2f::new(1.5, 2.5)),
            speed_kmh: Some(88.0),
            zone_id: Some("zone-1".to_string()),
            previous_zone_id: None,
            crossed_virtual_line: true,
            no_match_times: 0,
        }
    }

    /// The REST/event JSON contract: field names and shapes are what the
    /// consumers expect. The random event id is excluded from the snapshot.
    #[test]
    fn event_json_shape_is_stable() {
        let event = ViolationEvent::from_observation(
            &observation(),
            "traffic.solid_line_crossing",
            Some("zone-1".to_string()),
            "zone-1.solid_line".to_string(),
            "crossed".to_string(),
        )
        .for_solid_line("zone-1.solid_line".to_string(), Point2f::new(3.0, 4.0));

        let mut value = serde_json::to_value(&event).expect("serializable");
        value
            .as_object_mut()
            .expect("object")
            .remove("event_id")
            .expect("event id present");

        assert_eq!(
            value,
            serde_json::json!({
                "event_type": "traffic.solid_line_crossing",
                "status": "confirmed",
                "equipment_id": "eq-1",
                "track_id": "11111111-2222-3333-4444-555555555555",
                "observed_at": "2026-01-02T03:04:05Z",
                "source_time_relative_sec": 12.5,
                "class_name": "car",
                "confidence": 0.5,
                "bbox": { "x": 10, "y": 20, "width": 30, "height": 40 },
                "anchor_px": { "x": 25.0, "y": 60.0 },
                "image_point_px": { "x": 25.0, "y": 40.0 },
                "projected_point": { "x": 1.5, "y": 2.5 },
                "zone_id": "zone-1",
                "previous_zone_id": null,
                "crossed_virtual_line": true,
                "speed_kmh": 88.0,
                "solid_line_id": "zone-1.solid_line",
                "intersection_px": { "x": 3.0, "y": 4.0 },
                "rule_id": "zone-1.solid_line",
                "details": "crossed",
            })
        );
    }

    /// Rules without a solid line keep those fields out of the JSON.
    #[test]
    fn non_solid_line_events_omit_solid_line_fields() {
        let event = ViolationEvent::from_observation(
            &observation(),
            "traffic.speeding",
            Some("zone-1".to_string()),
            "zone-1.speeding".to_string(),
            "too fast".to_string(),
        );

        let value = serde_json::to_value(&event).expect("serializable");
        let object = value.as_object().expect("object");
        assert!(!object.contains_key("solid_line_id"), "{value}");
        assert!(!object.contains_key("intersection_px"), "{value}");
    }
}
