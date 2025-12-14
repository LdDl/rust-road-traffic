use opencv::{
    core::Rect as RectCV,
};

use mot_rs::mot::{SimpleBlob, BlobBBox};
use mot_rs::utils::{
    Rect, Point
};

use std::collections::HashSet;
use std::str::FromStr;

/// Kalman filter type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KalmanFilterType {
    /// Centroid Kalman filter (x, y, vx, vy) - centroid tracking
    Centroid,
    /// BBox Kalman filter (x, y, w, h, vx, vy, vw, vh) - full bbox tracking
    BBox,
}

impl Default for KalmanFilterType {
    fn default() -> Self {
        KalmanFilterType::Centroid
    }
}

impl FromStr for KalmanFilterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "centroid" => Ok(KalmanFilterType::Centroid),
            "bbox" => Ok(KalmanFilterType::BBox),
            _ => Err(format!("Invalid kalman filter type: '{}'. Supported: 'centroid', 'bbox'", s)),
        }
    }
}

/// Enum to hold either SimpleBlob (centroid tracking) or BlobBBox (centroid+bbox tracking) detections
#[derive(Debug, Clone)]
pub enum DetectionBlobs {
    Simple(Vec<SimpleBlob>),
    BBox(Vec<BlobBBox>),
}

impl DetectionBlobs {
    pub fn len(&self) -> usize {
        match self {
            DetectionBlobs::Simple(blobs) => blobs.len(),
            DetectionBlobs::BBox(blobs) => blobs.len(),
        }
    }
}

#[derive(Debug)]
pub struct Detections {
    pub blobs: DetectionBlobs,
    pub class_names: Vec<String>,
    pub confidences: Vec<f32>,
}

/// Helper to check if detection should be filtered out. Returns Some(classname) if valid, None if filtered.
#[inline]
fn filter_detection(class_id: usize, net_classes: &Vec<String>, target_classes: &HashSet<String>) -> Option<String> {
    if class_id >= net_classes.len() {
        return None;
    }
    let classname = net_classes[class_id].clone();
    if target_classes.len() > 0 && !target_classes.contains(&classname) {
        return None;
    }
    Some(classname)
}

pub fn process_yolo_detections(nms_bboxes: &Vec<RectCV>, nms_classes_ids: Vec<usize>, nms_confidences: Vec<f32>, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, net_classes: &Vec<String>, target_classes: &HashSet<String>, dt: f32, kalman_filter: KalmanFilterType) -> Detections {
    if (nms_bboxes.len() != nms_classes_ids.len()) || (nms_bboxes.len() != nms_confidences.len()) || (nms_classes_ids.len() != nms_confidences.len()) {
        println!("BBoxes len: {}, Classed IDs len: {}, Confidences len: {}", nms_bboxes.len(), nms_classes_ids.len(), nms_confidences.len());
        return Detections {
            blobs: DetectionBlobs::Simple(vec![]),
            class_names: vec![],
            confidences: vec![]
        };
    }

    // Single pass: filter and create blobs directly into typed vec
    let mut class_names: Vec<String> = Vec::with_capacity(nms_classes_ids.len());
    let mut confidences: Vec<f32> = Vec::with_capacity(nms_confidences.len());
    match kalman_filter {
        KalmanFilterType::BBox => {
            let mut blobs: Vec<BlobBBox> = Vec::with_capacity(nms_bboxes.len());
            for (i, bbox) in nms_bboxes.iter().enumerate() {
                if let Some(classname) = filter_detection(nms_classes_ids[i], net_classes, target_classes) {
                    class_names.push(classname);
                    confidences.push(nms_confidences[i]);
                    blobs.push(BlobBBox::new_with_dt(
                        Rect::new(bbox.x as f32, bbox.y as f32, bbox.width as f32, bbox.height as f32),
                        dt
                    ));
                }
            }
            Detections { blobs: DetectionBlobs::BBox(blobs), class_names, confidences }
        }
        KalmanFilterType::Centroid => {
            let mut blobs: Vec<SimpleBlob> = Vec::with_capacity(nms_bboxes.len());
            for (i, bbox) in nms_bboxes.iter().enumerate() {
                if let Some(classname) = filter_detection(nms_classes_ids[i], net_classes, target_classes) {
                    class_names.push(classname);
                    confidences.push(nms_confidences[i]);
                    let center_x = bbox.x as f32 + bbox.width as f32 / 2.0;
                    let bottom_center_y = bbox.y as f32 + bbox.height as f32;
                    blobs.push(SimpleBlob::new_with_center_dt(
                        Point::new(center_x, bottom_center_y),
                        Rect::new(bbox.x as f32, bbox.y as f32, bbox.width as f32, bbox.height as f32),
                        dt
                    ));
                }
            }
            Detections { blobs: DetectionBlobs::Simple(blobs), class_names, confidences }
        }
    }
}
