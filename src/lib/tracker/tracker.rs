use std::fmt;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{
    Occupied,
    Vacant
};
use uuid::Uuid;
use mot_rs::mot::{
    TrackerError,
    IoUTracker,
    ByteTracker,
    SimpleBlob,
    BlobBBox,
};

use crate::lib::detection::{Detections, DetectionBlobs, KalmanFilterType};
use super::tracked_blob::{TrackedBlob, TrackedBlobRef};
use super::object_extra::ObjectExtra;

/// Trait for SimpleBlob trackers
pub trait TrackerEngineSimple {
    fn match_objects(&mut self, detections: &mut Vec<SimpleBlob>, confidences: &[f32]) -> Result<(), TrackerError>;
    fn get_objects(&self) -> &HashMap<Uuid, SimpleBlob>;
}

/// Trait for BlobBBox trackers
pub trait TrackerEngineBBox {
    fn match_objects(&mut self, detections: &mut Vec<BlobBBox>, confidences: &[f32]) -> Result<(), TrackerError>;
    fn get_objects(&self) -> &HashMap<Uuid, BlobBBox>;
}

impl TrackerEngineSimple for IoUTracker<SimpleBlob> {
    fn match_objects(&mut self, detections: &mut Vec<SimpleBlob>, _confidences: &[f32]) -> Result<(), TrackerError> {
        self.match_objects(detections)
    }
    fn get_objects(&self) -> &HashMap<Uuid, SimpleBlob> {
        &self.objects
    }
}

impl TrackerEngineSimple for ByteTracker<SimpleBlob> {
    fn match_objects(&mut self, detections: &mut Vec<SimpleBlob>, confidences: &[f32]) -> Result<(), TrackerError> {
        self.match_objects(detections, &confidences)
    }
    fn get_objects(&self) -> &HashMap<Uuid, SimpleBlob> {
        &self.objects
    }
}

impl TrackerEngineBBox for IoUTracker<BlobBBox> {
    fn match_objects(&mut self, detections: &mut Vec<BlobBBox>, _confidences: &[f32]) -> Result<(), TrackerError> {
        self.match_objects(detections)
    }
    fn get_objects(&self) -> &HashMap<Uuid, BlobBBox> {
        &self.objects
    }
}

impl TrackerEngineBBox for ByteTracker<BlobBBox> {
    fn match_objects(&mut self, detections: &mut Vec<BlobBBox>, confidences: &[f32]) -> Result<(), TrackerError> {
        self.match_objects(detections, &confidences)
    }
    fn get_objects(&self) -> &HashMap<Uuid, BlobBBox> {
        &self.objects
    }
}

/// Tracker for SimpleBlob (Kalman filter for centroid tracking)
pub struct TrackerSimple<T: TrackerEngineSimple> {
    pub engine: T,
    pub objects_extra: HashMap<Uuid, ObjectExtra>,
}

/// Tracker for BlobBBox (Kalman filter for bbox tracking)
pub struct TrackerBBox<T: TrackerEngineBBox> {
    pub engine: T,
    pub objects_extra: HashMap<Uuid, ObjectExtra>,
}

impl<T: TrackerEngineSimple> TrackerSimple<T> {
    pub fn new_iou(max_no_match: usize, iou_threshold: f32) -> TrackerSimple<IoUTracker<SimpleBlob>> {
        TrackerSimple {
            engine: IoUTracker::new(max_no_match, iou_threshold),
            objects_extra: HashMap::new(),
        }
    }

    pub fn new_bytetrack(
        max_disappeared: usize,
        min_iou: f32,
        high_thresh: f32,
        low_thresh: f32,
        algorithm: mot_rs::mot::MatchingAlgorithm,
    ) -> TrackerSimple<ByteTracker<SimpleBlob>> {
        TrackerSimple {
            engine: ByteTracker::new(max_disappeared, min_iou, high_thresh, low_thresh, algorithm),
            objects_extra: HashMap::new(),
        }
    }

    pub fn match_objects(&mut self, detections: &mut Detections, current_second: f32) -> Result<(), TrackerError> {
        let blobs = match &mut detections.blobs {
            DetectionBlobs::Simple(b) => b,
            DetectionBlobs::BBox(_) => return Err(TrackerError::BadSize("Expected SimpleBlob detections for centroid tracker".to_string())),
        };

        self.engine.match_objects(blobs, &detections.confidences)?;

        // Update extra information for each object
        for (idx, detection) in blobs.iter().enumerate() {
            let object_id = detection.get_id();
            match self.objects_extra.entry(object_id) {
                Occupied(mut entry) => {
                    entry.get_mut().times.push(current_second);
                    if entry.get().times.len() > detection.get_max_track_len() {
                        entry.get_mut().times = entry.get_mut().times[1..].to_vec();
                    }
                }
                Vacant(entry) => {
                    let mut object_extra = ObjectExtra::new(
                        detections.class_names[idx].to_owned(),
                        detections.confidences[idx],
                        detection.get_max_track_len(),
                    );
                    object_extra.times.push(current_second);
                    entry.insert(object_extra);
                }
            }
        }

        let ref_engine_objects = &self.engine.get_objects();
        self.objects_extra.retain(|object_id, _| ref_engine_objects.contains_key(object_id));
        Ok(())
    }
}

impl<T: TrackerEngineBBox> TrackerBBox<T> {
    pub fn new_iou(max_no_match: usize, iou_threshold: f32) -> TrackerBBox<IoUTracker<BlobBBox>> {
        TrackerBBox {
            engine: IoUTracker::new(max_no_match, iou_threshold),
            objects_extra: HashMap::new(),
        }
    }

    pub fn new_bytetrack(
        max_disappeared: usize,
        min_iou: f32,
        high_thresh: f32,
        low_thresh: f32,
        algorithm: mot_rs::mot::MatchingAlgorithm,
    ) -> TrackerBBox<ByteTracker<BlobBBox>> {
        TrackerBBox {
            engine: ByteTracker::new(max_disappeared, min_iou, high_thresh, low_thresh, algorithm),
            objects_extra: HashMap::new(),
        }
    }

    pub fn match_objects(&mut self, detections: &mut Detections, current_second: f32) -> Result<(), TrackerError> {
        let blobs = match &mut detections.blobs {
            DetectionBlobs::BBox(b) => b,
            DetectionBlobs::Simple(_) => return Err(TrackerError::BadSize("Expected BlobBBox detections for bbox tracker".to_string())),
        };

        self.engine.match_objects(blobs, &detections.confidences)?;

        // Update extra information for each object
        for (idx, detection) in blobs.iter().enumerate() {
            let object_id = detection.get_id();
            match self.objects_extra.entry(object_id) {
                Occupied(mut entry) => {
                    entry.get_mut().times.push(current_second);
                    if entry.get().times.len() > detection.get_max_track_len() {
                        entry.get_mut().times = entry.get_mut().times[1..].to_vec();
                    }
                }
                Vacant(entry) => {
                    let mut object_extra = ObjectExtra::new(
                        detections.class_names[idx].to_owned(),
                        detections.confidences[idx],
                        detection.get_max_track_len(),
                    );
                    object_extra.times.push(current_second);
                    entry.insert(object_extra);
                }
            }
        }

        let ref_engine_objects = &self.engine.get_objects();
        // Remove obsolete objects
        self.objects_extra.retain(|object_id, _| ref_engine_objects.contains_key(object_id));
        Ok(())
    }
}

impl<T: TrackerEngineSimple + fmt::Display> fmt::Display for TrackerSimple<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.engine)
    }
}

/// Create tracker based on tracker type and Kalman filter type
pub fn new_tracker_from_type(tracker_type: &str, kalman_filter: KalmanFilterType) -> Box<dyn TrackerTrait> {
    match (tracker_type, kalman_filter) {
        ("iou_naive", KalmanFilterType::BBox) => Box::new(TrackerBBox::<IoUTracker<BlobBBox>>::new_iou(15, 0.3)),
        ("bytetrack", KalmanFilterType::BBox) => Box::new(TrackerBBox::<ByteTracker<BlobBBox>>::new_bytetrack(
            15, 0.3, 0.7, 0.3, mot_rs::mot::MatchingAlgorithm::Hungarian
        )),
        ("iou_naive", KalmanFilterType::Centroid) => Box::new(TrackerSimple::<IoUTracker<SimpleBlob>>::new_iou(15, 0.3)),
        ("bytetrack", KalmanFilterType::Centroid) => Box::new(TrackerSimple::<ByteTracker<SimpleBlob>>::new_bytetrack(
            15, 0.3, 0.7, 0.3, mot_rs::mot::MatchingAlgorithm::Hungarian
        )),
        (_, KalmanFilterType::BBox) => {
            println!("Unknown tracker type '{}', falling back to iou_naive", tracker_type);
            Box::new(TrackerBBox::<IoUTracker<BlobBBox>>::new_iou(15, 0.3))
        }
        _ => {
            println!("Unknown tracker type '{}', falling back to iou_naive", tracker_type);
            Box::new(TrackerSimple::<IoUTracker<SimpleBlob>>::new_iou(15, 0.3))
        }
    }
}

/// Common trait for all tracker types (both SimpleBlob and BlobBBox based)
pub trait TrackerTrait {
    fn match_objects(&mut self, detections: &mut Detections, current_second: f32) -> Result<(), TrackerError>;
    fn get_objects_extra(&self) -> &HashMap<Uuid, ObjectExtra>;
    fn get_object_extra_mut(&mut self, object_id: &Uuid) -> Option<&mut ObjectExtra>;
    /// Returns tracked objects as TrackedBlob enum (works for both centroid and bbox tracking)
    /// DEPRECATED: Use iter_tracked_objects() for zero-copy iteration
    fn get_tracked_objects(&self) -> HashMap<Uuid, TrackedBlob>;
    /// DEPRECATED: Use get_tracked_object_ref() for zero-copy lookup
    fn get_tracked_object(&self, object_id: &Uuid) -> Option<TrackedBlob>;
    /// Zero-copy iteration over tracked objects - avoids cloning trajectory history
    fn iter_tracked_objects(&self) -> Box<dyn Iterator<Item = (Uuid, TrackedBlobRef<'_>)> + '_>;
    /// Zero-copy lookup of a single tracked object by ID
    fn get_tracked_object_ref(&self, object_id: &Uuid) -> Option<TrackedBlobRef<'_>>;
    /// Returns a human-readable description of the tracker configuration
    fn description(&self) -> String;
}

impl<T: TrackerEngineSimple> TrackerTrait for TrackerSimple<T> {
    fn match_objects(&mut self, detections: &mut Detections, current_second: f32) -> Result<(), TrackerError> {
        self.match_objects(detections, current_second)
    }

    fn get_objects_extra(&self) -> &HashMap<Uuid, ObjectExtra> {
        &self.objects_extra
    }

    fn get_object_extra_mut(&mut self, object_id: &Uuid) -> Option<&mut ObjectExtra> {
        self.objects_extra.get_mut(object_id)
    }

    fn get_tracked_objects(&self) -> HashMap<Uuid, TrackedBlob> {
        self.engine.get_objects().iter()
            .map(|(id, blob)| (*id, TrackedBlob::Simple(blob.clone())))
            .collect()
    }

    fn get_tracked_object(&self, object_id: &Uuid) -> Option<TrackedBlob> {
        self.engine.get_objects().get(object_id)
            .map(|blob| TrackedBlob::Simple(blob.clone()))
    }

    fn iter_tracked_objects(&self) -> Box<dyn Iterator<Item = (Uuid, TrackedBlobRef<'_>)> + '_> {
        Box::new(
            self.engine.get_objects().iter()
                .map(|(id, blob)| (*id, TrackedBlobRef::Simple(blob)))
        )
    }

    fn get_tracked_object_ref(&self, object_id: &Uuid) -> Option<TrackedBlobRef<'_>> {
        self.engine.get_objects().get(object_id)
            .map(|blob| TrackedBlobRef::Simple(blob))
    }

    fn description(&self) -> String {
        let type_name = std::any::type_name::<T>();
        let engine_name = type_name
            .split('<').next().unwrap_or(type_name)  // Get part before generic <
            .split("::").last().unwrap_or("unknown"); // Get last path segment
        format!("Centroid tracker (4D Kalman: x, y, vx, vy) with {} engine", engine_name)
    }
}

impl<T: TrackerEngineBBox> TrackerTrait for TrackerBBox<T> {
    fn match_objects(&mut self, detections: &mut Detections, current_second: f32) -> Result<(), TrackerError> {
        self.match_objects(detections, current_second)
    }

    fn get_objects_extra(&self) -> &HashMap<Uuid, ObjectExtra> {
        &self.objects_extra
    }

    fn get_object_extra_mut(&mut self, object_id: &Uuid) -> Option<&mut ObjectExtra> {
        self.objects_extra.get_mut(object_id)
    }

    fn get_tracked_objects(&self) -> HashMap<Uuid, TrackedBlob> {
        self.engine.get_objects().iter()
            .map(|(id, blob)| (*id, TrackedBlob::BBox(blob.clone())))
            .collect()
    }

    fn get_tracked_object(&self, object_id: &Uuid) -> Option<TrackedBlob> {
        self.engine.get_objects().get(object_id)
            .map(|blob| TrackedBlob::BBox(blob.clone()))
    }

    fn iter_tracked_objects(&self) -> Box<dyn Iterator<Item = (Uuid, TrackedBlobRef<'_>)> + '_> {
        Box::new(
            self.engine.get_objects().iter()
                .map(|(id, blob)| (*id, TrackedBlobRef::BBox(blob)))
        )
    }

    fn get_tracked_object_ref(&self, object_id: &Uuid) -> Option<TrackedBlobRef<'_>> {
        self.engine.get_objects().get(object_id)
            .map(|blob| TrackedBlobRef::BBox(blob))
    }

    fn description(&self) -> String {
        let type_name = std::any::type_name::<T>();
        let engine_name = type_name
            .split('<').next().unwrap_or(type_name)  // Get part before generic <
            .split("::").last().unwrap_or("unknown"); // Get last path segment
        format!("BBox tracker (8D Kalman: x, y, w, h, vx, vy, vw, vh) with {} engine", engine_name)
    }
}

impl fmt::Display for dyn TrackerTrait {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}