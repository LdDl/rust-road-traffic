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
use crate::lib::spatial::haversine;
use super::tracked_blob::TrackedBlob;

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

/// Tracker for BlobBBox (8D Kalman filter)
pub struct TrackerBBox<T: TrackerEngineBBox> {
    pub engine: T,
    pub objects_extra: HashMap<Uuid, ObjectExtra>,
}

pub struct ObjectExtra {
    class_name: String,
    confidence: f32,
    // Timestamps along the whole track
    pub times: Vec<f32>,
    pub estimated_velocity: f32,
    pub spatial_info: Option<SpatialInfo>,
}

impl ObjectExtra {
    pub fn get_classname(&self) -> String {
        self.class_name.clone()
    }
}

pub struct SpatialInfo {
    pub first_time: f32,
    pub first_x_projected: f32,
    pub first_y_projected: f32,
    pub last_time: f32,
    pub last_lon: f32,
    pub last_lat: f32,
    pub last_x: f32,
    pub last_y: f32,
    pub last_x_projected: f32,
    pub last_y_projected: f32,
    pub distance_traveled: f32,
    pub speed: f32,
}

impl SpatialInfo {
    pub fn new(_time: f32,  _x: f32, _y: f32, _x_projected: f32, _y_projected: f32) -> Self {
        Self {
            first_time: _time,
            first_x_projected: _x_projected,
            first_y_projected: _y_projected,
            last_time: _time,
            last_lon: -1.0,
            last_lat: -1.0,
            last_x: _x,
            last_y: _y,
            last_x_projected: _x_projected,
            last_y_projected: _y_projected,
            distance_traveled: -1.0,
            speed: -1.0,
        }
    }
    pub fn new_wgs84(_time: f32, _lon: f32, _lat: f32, _x: f32, _y: f32) -> Self {
        Self {
            first_time: _time,
            first_x_projected: -1.0,
            first_y_projected: -1.0,
            last_time: _time,
            last_lon: _lon,
            last_lat: _lat,
            last_x: _x,
            last_y: _y,
            last_x_projected: -1.0,
            last_y_projected: -1.0,
            distance_traveled: -1.0,
            speed: -1.0,
        }
    }
    // Same as update(), but calculations are done between first and last points
    // This approach helps to avoid situation when distance between two points is approx. 0
    pub fn update_avg(&mut self, _time: f32, _x: f32, _y: f32, _x_projected: f32, _y_projected: f32, pixels_per_meter: f32) {
        // It is possible to calculate speed between two points (old and new)
        let distance_pixels = ((_x_projected - self.first_x_projected).powi(2) + (_y_projected - self.first_y_projected).powi(2)).sqrt();
        let distance_meters = distance_pixels / pixels_per_meter;
        let time_diff = (_time - self.first_time).abs();
        let velocity = distance_meters / time_diff; // meters per second
        self.speed = velocity * 3.6; // convert m/s to km/h
        self.last_time = _time;
        self.last_x = _x;
        self.last_y = _y;
        self.last_x_projected = _x_projected;
        self.last_y_projected = _y_projected;
    }
    pub fn update(&mut self, _time: f32, _x: f32, _y: f32, _x_projected: f32, _y_projected: f32, pixels_per_meter: f32) {
        // It is possible to calculate speed between two points (old and new)
        let distance_pixels = ((_x_projected - self.last_x_projected).powi(2) + (_y_projected - self.last_y_projected).powi(2)).sqrt();
        let distance_meters = distance_pixels / pixels_per_meter;
        let time_diff = _time - self.last_time;
        let velocity = distance_meters / time_diff; // meters per second
        self.speed = velocity * 3.6; // convert m/s to km/h

        self.last_time = _time;
        self.last_x = _x;
        self.last_y = _y;
        self.last_x_projected = _x_projected;
        self.last_y_projected = _y_projected;
    }
    fn update_by_wgs84(&mut self, _time: f32, _lon: f32, _lat: f32, _x: f32, _y: f32) {
        // It is possible to calculate speed between two points (old and new)
        let distance = haversine(self.last_lon, self.last_lat, _lon, _lat) * 1000.0;
        let time_diff = _time - self.last_time;
        let velocity = distance / time_diff; // meters per second
        self.distance_traveled = distance;
        self.speed = velocity * 3.6; // convert m/s to km/h
        
        self.last_time = _time;
        self.last_lon = _lon;
        self.last_lat = _lat;
        self.last_x = _x;
        self.last_y = _y;
    }
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
                    let mut object_extra = ObjectExtra {
                        class_name: detections.class_names[idx].to_owned(),
                        confidence: detections.confidences[idx],
                        times: Vec::with_capacity(detection.get_max_track_len()),
                        estimated_velocity: -1.0,
                        spatial_info: None,
                    };
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
            DetectionBlobs::Simple(_) => return Err(TrackerError::BadSize("Expected BlobBBox detections for 8D tracker".to_string())),
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
                    let mut object_extra = ObjectExtra {
                        class_name: detections.class_names[idx].to_owned(),
                        confidence: detections.confidences[idx],
                        times: Vec::with_capacity(detection.get_max_track_len()),
                        estimated_velocity: -1.0,
                        spatial_info: None,
                    };
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
    fn get_tracked_objects(&self) -> HashMap<Uuid, TrackedBlob>;
    fn get_tracked_object(&self, object_id: &Uuid) -> Option<TrackedBlob>;
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
}

impl fmt::Display for dyn TrackerTrait {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TrackerTrait object")
    }
}