use opencv::{
    prelude::*,
    core::Mat,
    core::Vector,
    core::Rect as RectCV,
    dnn::nms_boxes
};

use chrono::{
    DateTime,
    Utc
};

use mot_rs::mot::SimpleBlob;
use mot_rs::utils::{
    Rect, Point
};

use std::collections::HashSet;

#[derive(Debug)]
pub struct Detections {
    pub blobs: Vec<SimpleBlob>,
    pub class_names: Vec<String>,
    pub confidences: Vec<f32>,
}

pub fn process_yolo_detections(nms_bboxes: &Vec<RectCV>, nms_classes_ids: Vec<usize>, nms_confidences: Vec<f32>, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, net_classes: &Vec<String>, target_classes: &HashSet<String>, dt: f32) -> Detections {
    if (nms_bboxes.len() != nms_classes_ids.len()) || (nms_bboxes.len() != nms_confidences.len()) || (nms_classes_ids.len() != nms_confidences.len()) {
        // Something wrong?
        println!("BBoxes len: {}, Classed IDs len: {}, Confidences len: {}", nms_bboxes.len(), nms_classes_ids.len(), nms_confidences.len());
        return Detections {
            blobs: vec![],
            class_names: vec![],
            confidences: vec![]
        };
    }
    let mut aggregated_data = vec![];
    let mut class_names: Vec<String> = Vec::with_capacity(nms_classes_ids.len());
    for (i, bbox) in nms_bboxes.iter().enumerate() {
        let class_id = nms_classes_ids[i];
        if class_id >= net_classes.len() {
            // Evade panic?
            continue
        };
        let classname = net_classes[class_id].clone();
        if target_classes.len() > 0 && !target_classes.contains(&classname) {
            continue;
        }
        class_names.push(classname);
        let center_x = (bbox.x as f32 + bbox.width as f32 / 2.0);
        let bottom_center_y = (bbox.y as f32 + bbox.height as f32);
        let kb: SimpleBlob = SimpleBlob::new_with_center_dt(Point::new(center_x, bottom_center_y), Rect::new(bbox.x as f32, bbox.y as f32, bbox.width as f32, bbox.height as f32), dt);
        // let mut kb = SimpleBlob::new_with_dt(Rect::new(bbox.x as f32, bbox.y as f32, bbox.width as f32, bbox.height as f32), dt);
        aggregated_data.push(kb);
    }
    return Detections {
        blobs: aggregated_data,
        class_names: class_names,
        confidences: nms_confidences,
    }
}
