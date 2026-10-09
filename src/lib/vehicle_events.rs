use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::lib::anpr::images::EventImages;
use crate::lib::anpr::plate_detector::PlateModels;
use crate::lib::anpr::tracking::TrackRecognition;
use crate::lib::anpr::types::{BoundingBox, PlateResult, VehicleDetection};
use crate::lib::cv::RawFrame;
use crate::lib::detection::{DetectionBlobs, Detections};
use crate::lib::publisher::redis_transport::RedisPublisher;
use crate::lib::tracker::TrackerTrait;
use crate::settings::EventImage;

pub type VehicleEvents = broadcast::Sender<Arc<VehicleEvent>>;

#[derive(Serialize)]
pub enum EventType {
    #[serde(rename = "vehicle.passed")]
    VehiclePassed,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VehiclePassed => "vehicle.passed",
        }
    }
}

#[derive(Serialize)]
pub struct VehicleEvent {
    event_id: Uuid,
    #[serde(rename = "type")]
    pub event_type: EventType,
    equipment_id: String,
    track_id: Uuid,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    vehicle: VehicleDetection,
    plate: Option<PlateResult>,
    frame_base64: Option<String>,
    frame_type: EventImage,
    frame_width: u32,
    frame_height: u32,
}

struct VehicleEventState {
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    vehicle: VehicleDetection,
    frame_size: (u32, u32),
    eligible: bool,
    recognition: TrackRecognition,
}

pub struct VehicleEventCollector {
    active: HashMap<Uuid, VehicleEventState>,
    events: VehicleEvents,
    images: EventImages,
    redis: Option<RedisPublisher>,
}

impl VehicleEventCollector {
    pub fn new(events: VehicleEvents, image: EventImage, redis: Option<RedisPublisher>) -> Self {
        Self {
            active: HashMap::new(),
            events,
            images: EventImages::new(image),
            redis,
        }
    }

    pub fn observe_detections(
        &mut self,
        detections: &Detections,
        tracker: &dyn TrackerTrait,
        observed_at: DateTime<Utc>,
        frame_size: (u32, u32),
    ) {
        for index in 0..detections.blobs.len() {
            let (id, bbox) = match &detections.blobs {
                DetectionBlobs::Simple(blobs) => (blobs[index].get_id(), blobs[index].get_bbox()),
                DetectionBlobs::BBox(blobs) => (blobs[index].get_id(), blobs[index].get_bbox()),
            };
            if tracker.get_tracked_object_ref(&id).is_none() {
                continue;
            }
            self.observe(
                id,
                observed_at,
                VehicleDetection {
                    class: detections.class_names[index].clone(),
                    confidence: detections.confidences[index],
                    bbox: BoundingBox::from_detection(bbox, frame_size.0, frame_size.1),
                },
                frame_size,
            );
        }
    }

    fn observe(
        &mut self,
        track_id: Uuid,
        observed_at: DateTime<Utc>,
        vehicle: VehicleDetection,
        frame_size: (u32, u32),
    ) {
        match self.active.entry(track_id) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                state.ended_at = observed_at;
                state.vehicle = vehicle;
                state.frame_size = frame_size;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(VehicleEventState {
                    started_at: observed_at,
                    ended_at: observed_at,
                    vehicle,
                    frame_size,
                    eligible: false,
                    recognition: TrackRecognition::default(),
                });
            }
        }
    }

    pub fn record_zone_result(&mut self, track_id: Uuid, counted_in_zone: bool) {
        if let Some(state) = self.active.get_mut(&track_id) {
            state.eligible |= counted_in_zone;
        }
    }

    pub fn detect_plates(
        &mut self,
        models: &mut PlateModels,
        frame: &RawFrame,
        observed_at: DateTime<Utc>,
    ) {
        self.images.begin_frame();
        for (track_id, state) in &mut self.active {
            if !state.eligible || state.ended_at != observed_at {
                continue;
            }
            state
                .recognition
                .observe(models, frame, &state.vehicle, *track_id, &mut self.images);
        }
    }

    pub fn publish_completed(
        &mut self,
        tracker: &dyn TrackerTrait,
        equipment_id: &str,
        mut models: Option<&mut PlateModels>,
    ) {
        let expired: Vec<Uuid> = self
            .active
            .keys()
            .filter(|id| tracker.get_tracked_object_ref(id).is_none())
            .copied()
            .collect();

        for id in expired {
            let Some(state) = self.active.remove(&id) else {
                continue;
            };
            if !state.eligible {
                continue;
            }
            let (vehicle, frame_size, plate, image) =
                match state
                    .recognition
                    .complete(models.as_deref_mut(), id, &mut self.images)
                {
                    Some(result) => (
                        result.vehicle,
                        result.frame_size,
                        result.plate,
                        result.image,
                    ),
                    None => (state.vehicle, state.frame_size, None, None),
                };
            let event = VehicleEvent {
                event_id: Uuid::new_v4(),
                event_type: EventType::VehiclePassed,
                equipment_id: equipment_id.to_owned(),
                track_id: id,
                started_at: state.started_at,
                ended_at: state.ended_at,
                vehicle,
                plate,
                frame_base64: image.map(|bytes| STANDARD.encode(bytes.as_slice())),
                frame_type: self.images.mode(),
                frame_width: frame_size.0,
                frame_height: frame_size.1,
            };
            let event = Arc::new(event);
            if let Some(publisher) = &self.redis {
                let message = event.clone();
                publisher.publish(move || serde_json::to_string(message.as_ref()));
            }
            // A broadcast with no subscribers is intentionally discarded.
            let _ = self.events.send(event);
        }
    }
}
