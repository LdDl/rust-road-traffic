use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::lib::track_observations::{BoundingBoxPx, PointProjected, PointPx};

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

    pub bbox: BoundingBoxPx,
    pub anchor_px: PointPx,
    pub image_point_px: PointPx,
    pub projected_point: Option<PointProjected>,

    pub zone_id: Option<String>,
    pub previous_zone_id: Option<String>,
    pub crossed_virtual_line: bool,

    pub speed_kmh: Option<f32>,
    pub rule_id: String,
    pub details: String,
}