use opencv::{
    prelude::*,
    core::Mat,
    core::Vector,
    core::Rect,
    dnn::nms_boxes
};

use chrono::{
    DateTime,
    Utc
};

use crate::lib::tracking::{
    KalmanBlobie
};

pub fn process_yolo_detections(detections: &Vector::<Mat>, conf_threshold: f32, nms_threshold: f32, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, classes: &Vec<String>, filtered_classes: &'static [&'static str], classes_num: usize, last_time: DateTime<Utc>, sec_diff: f64) -> Vec<KalmanBlobie> {
    let mut tmp_blobs = vec![];
    let outs = detections.len();
    let mut class_names = vec![];
    let mut confidences = Vector::<f32>::new();
    let mut bboxes = Vector::<Rect>::new();
    for o in 0..outs {
        let output = detections.get(o).unwrap();
        let data_ptr = output.data_typed::<f32>().unwrap();
        for (i, _) in data_ptr.iter().enumerate().step_by(classes_num + 5) {
            let mut class_id = 0 as usize;
            let mut max_probability = 0.0;
            for j in 5..(classes_num + 5) {
                if data_ptr[i+j] > max_probability {
                    max_probability = data_ptr[i+j];
                    class_id = (j-5) % classes_num;
                }
            }
            let class_name = classes[class_id].clone();
            if filtered_classes.contains(&&*class_name) {
                let confidence = max_probability * data_ptr[i+4];
                if confidence > conf_threshold {
                    let center_x = data_ptr[i] * frame_cols;
                    let center_y = data_ptr[i + 1] * frame_rows;
                    let width = data_ptr[i + 2] * frame_cols;
                    let height = data_ptr[i + 3] * frame_rows;
                    let left = center_x - width / 2.0;
                    let top = center_y - height / 2.0;
                    let bbox = Rect::new(left as i32, top as i32, width as i32, height as i32);
                    class_names.push(class_name);
                    confidences.push(confidence);
                    bboxes.push(bbox);
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
    for (i, _) in indices.iter().enumerate() {
        match bboxes.get(i) {
            Ok(bbox) => {
                let class_name = &class_names[i];
                let mut kb = KalmanBlobie::new_with_time(&bbox, max_points_in_track, last_time, sec_diff);
                kb.set_class_name(class_name.to_string());
                tmp_blobs.push(kb);
            },
            Err(err) => {
                panic!("Can't extract bbox from filtered bboxes due the error {:?}", err);
            }
        }
    }
    return tmp_blobs;
}

pub fn process_mobilenet_detections(detections: &Vector::<Mat>, conf_threshold: f32, frame_cols: f32, frame_rows: f32, max_points_in_track: usize, classes: &Vec<String>, filtered_classes: &'static [&'static str], last_time: DateTime<Utc>, sec_diff: f64) -> Vec<KalmanBlobie> {
    let mut tmp_blobs = vec![];
    let outs = detections.len();
    for o in 0..outs {
        let output = detections.get(o).unwrap();
        let data_ptr = output.data_typed::<f32>().unwrap();
        for (i, _) in data_ptr.iter().enumerate().step_by(7) {
            let confidence = data_ptr[i+2];
            let class_id = data_ptr[i+1] as usize;
            let class_name = classes[class_id].clone();
            if filtered_classes.contains(&&*class_name) {
                if confidence > conf_threshold {
                    let left = (data_ptr[i+3] * frame_cols) as i32;
                    let top = (data_ptr[i+4] * frame_rows) as i32;
                    let right = (data_ptr[i+5] * frame_cols) as i32;
                    let bottom = (data_ptr[i+6] * frame_rows) as i32;
                    let width = right - left + 1; 
                    let height = bottom - top + 1;
                    if (frame_cols as i32 - width) < 100 {
                        continue
                    }
                    let bbox = Rect::new(left, top, width, height);
                    let mut kb = KalmanBlobie::new_with_time(&bbox, max_points_in_track, last_time, sec_diff);
                    kb.set_class_name(class_name.to_string());
                    tmp_blobs.push(kb);
                }
            }
        }
    }
    return tmp_blobs;
}
