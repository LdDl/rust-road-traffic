use crate::lib::cv::Rect;
use crate::lib::spatial::Point2f;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Broadcast sender used to share track observations between consumers.
/// One message per frame carries the observations of every active track, so
/// consumers evaluate rules on whole frames (the solid-line rule needs the
/// frame batch) and the channel is not woken once per track.
pub type TrackObservations = broadcast::Sender<Arc<Vec<TrackObservation>>>;

/// Broadcast receiver used by an individual track observation consumer.
pub type TrackObservationRx = broadcast::Receiver<Arc<Vec<TrackObservation>>>;

/// A snapshot of a tracked object's geometry, metadata, and zone context.
#[derive(Debug, Clone, Serialize)]
pub struct TrackObservation {
    /// Identifier of the installation producing the observation.
    pub equipment_id: String,
    /// Identifier assigned to the object by the tracker.
    pub track_id: Uuid,
    /// UTC timestamp associated with processing the observed frame.
    pub observed_at: DateTime<Utc>,
    /// Frame timestamp in seconds relative to the source timeline.
    pub source_time_relative_sec: f32,

    /// Object class associated with the track.
    pub class_name: String,
    /// Detection confidence supplied by the observation producer.
    /// This is not a confidence score for a violation.
    pub confidence: f32,

    /// Object bounding box in the video frame.
    pub bbox: Rect,
    /// Anchor point supplied by the producer, typically the bottom center
    /// of the bounding box.
    pub anchor_px: Point2f,
    /// Image-space track point used for zone geometry calculations.
    pub image_point_px: Point2f,
    /// Projected point, when available.
    pub projected_point: Option<Point2f>,
    /// Estimated speed in kilometres per hour, when available.
    pub speed_kmh: Option<f32>,

    /// Identifier of the zone associated with this observation.
    pub zone_id: Option<String>,
    /// Previous zone identifier supplied by the producer, when available.
    pub previous_zone_id: Option<String>,
    /// Whether the producer detected a direction-specific virtual-line
    /// crossing for this observation.
    pub crossed_virtual_line: bool,

    /// Number of consecutive unmatched updates reported by the tracker.
    pub no_match_times: usize,
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use chrono::Utc;

    /// Minimal observation for rule unit tests: the anchor and image point are
    /// the same, the box is 20x20 around that point.
    pub fn observation(
        track_id: Uuid,
        time: f32,
        x: f32,
        y: f32,
        zone_id: Option<&str>,
        speed_kmh: Option<f32>,
    ) -> TrackObservation {
        TrackObservation {
            equipment_id: "eq".to_string(),
            track_id,
            observed_at: Utc::now(),
            source_time_relative_sec: time,
            class_name: "car".to_string(),
            confidence: 0.9,
            bbox: Rect::new((x - 10.0).round() as i32, (y - 10.0).round() as i32, 20, 20),
            anchor_px: Point2f::new(x, y),
            image_point_px: Point2f::new(x, y),
            projected_point: None,
            speed_kmh,
            zone_id: zone_id.map(str::to_string),
            previous_zone_id: None,
            crossed_virtual_line: false,
            no_match_times: 0,
        }
    }
}
