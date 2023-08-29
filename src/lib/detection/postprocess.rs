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

#[derive(Debug)]
pub struct Detections {
    pub blobs: Vec<SimpleBlob>,
    pub class_names: Vec<String>,
    pub confidences: Vec<f32>,
}

pub fn process_yolo_detections(detections: &Vector::<Mat>, conf_threshold: f32, nms_threshold: f32, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, classes: &Vec<String>, filtered_classes: &'static [&'static str], dt: f32) -> Detections {
    let mut class_names = vec![];
    let mut confidences = Vector::<f32>::new();
    let mut bboxes = Vector::<RectCV>::new();

    for layer in detections {
        let num_boxes = layer.rows();
        for index in 0..num_boxes {
            let pred = layer.row(index).unwrap();
            let detection = pred.data_typed::<f32>().unwrap();
            let (center_x, center_y, width, height, confidence) = match &detection[0..5] {
                &[a,b,c,d,e] => (a * frame_cols, b * frame_rows, c * frame_cols, d * frame_rows, e),
                _ => {
                    panic!("unreachable???")
                }
            };
            let detected_classes = &detection[5..];
            if confidence > conf_threshold {
                let mut class_index = -1;
                let mut score = 0.0;
                for (idx, &val) in detected_classes.iter().enumerate() {
                    if val > score {
                        class_index = idx as i32;
                        score = val;
                    }
                }
                if class_index > -1 && score > 0. {
                    let class_name = classes[class_index as usize].clone();
                    if filtered_classes.contains(&&*class_name) {
                        let left = center_x - width / 2.0;
                        let top = center_y - height / 2.0;
                        let bbox = RectCV::new(left.floor() as i32, top.floor() as i32, width as i32, height as i32);
                        class_names.push(class_name);
                        confidences.push(confidence);
                        bboxes.push(bbox);
                    }
                }
            }
        }
    }

    let mut indices = Vector::<i32>::new();
    match nms_boxes(&bboxes, &confidences, conf_threshold, nms_threshold, &mut indices, 1.0, 0) {
        Ok(_) => {},
        Err(err) => {
            println!("Can't run NMSBoxes on detections due the error {:?}", err);
        }
    };
    let mut nms_bboxes = vec![];
    let mut nms_classes = vec![];
    let mut nms_confidences = vec![];
    let indices = indices.to_vec();
    let mut bboxes = bboxes.to_vec();
    nms_bboxes.extend(bboxes.drain(..)
        .enumerate()
        .filter_map(|(idx, item)| if indices.contains(&(idx as i32)) {Some(item)} else {None}));

    nms_classes.extend(class_names.drain(..)
        .enumerate()
        .filter_map(|(idx, item)| if indices.contains(&(idx as i32)) {Some(item)} else {None}));

    nms_confidences.extend(confidences.to_vec().drain(..)
        .enumerate()
        .filter_map(|(idx, item)| if indices.contains(&(idx as i32)) {Some(item)} else {None}));

    let mut aggregated_data = vec![];
    for (i, bbox) in nms_bboxes.iter().enumerate() {
        let class_name = &nms_classes[i];
        let confidence = nms_confidences[i];
        let center_x = (bbox.x as f32 + bbox.width as f32 / 2.0);
        let center_y = (bbox.y as f32 + bbox.height as f32);
        let mut kb = SimpleBlob::new_with_center_dt(Point::new(center_x, center_y), Rect::new(bbox.x as f32, bbox.y as f32, bbox.width as f32, bbox.height as f32), dt);
        // let mut kb = SimpleBlob::new_with_dt(Rect::new(bbox.x as f32, bbox.y as f32, bbox.width as f32, bbox.height as f32), dt);

        aggregated_data.push(kb);
    }
    return Detections {
        blobs: aggregated_data,
        class_names: nms_classes,
        confidences: nms_confidences,
    }
}
