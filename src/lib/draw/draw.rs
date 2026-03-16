use crate::lib::cv::{RawFrame, Scalar};
use crate::lib::draw::colors::ClassColors;
use crate::lib::draw::primitives::{
    draw_filled_circle, draw_rounded_rect, draw_text, scalar_to_bgr,
};
use crate::lib::tracker::TrackerTrait;

pub fn draw_track(img: &mut RawFrame, tracker: &dyn TrackerTrait, class_colors: &ClassColors) {
    draw_trajectories(img, tracker, class_colors);
    draw_bboxes(img, tracker, class_colors);
    draw_identifiers(img, tracker, class_colors);
    draw_speeds(img, tracker, class_colors);
    draw_projections(img, tracker, class_colors);
}

pub fn draw_trajectories(
    img: &mut RawFrame,
    tracker: &dyn TrackerTrait,
    class_colors: &ClassColors,
) {
    let w = img.cols() as usize;
    let h = img.rows() as usize;
    let step = img.step();
    let bytes = img.data_bytes_mut();

    let objects_extra = tracker.get_objects_extra();
    for (object_id, object) in tracker.iter_tracked_objects() {
        let class_name = objects_extra
            .get(&object_id)
            .map(|extra| extra.get_classname())
            .unwrap_or("unknown".to_string());
        let color = if object.get_no_match_times() > 1 {
            class_colors.get_lost_color(&class_name)
        } else {
            class_colors.get_color(&class_name)
        };
        let bgr = scalar_to_bgr(&color);
        for pt in object.get_track().iter() {
            draw_filled_circle(
                bytes,
                step,
                w,
                h,
                pt.x.floor() as i32,
                pt.y.floor() as i32,
                2,
                bgr,
            );
        }
    }
}

pub fn draw_bboxes(img: &mut RawFrame, tracker: &dyn TrackerTrait, class_colors: &ClassColors) {
    let w = img.cols() as usize;
    let h = img.rows() as usize;
    let step = img.step();
    let bytes = img.data_bytes_mut();

    let objects_extra = tracker.get_objects_extra();
    for (object_id, object) in tracker.iter_tracked_objects() {
        let class_name = objects_extra
            .get(&object_id)
            .map(|extra| extra.get_classname())
            .unwrap_or("unknown".to_string());
        let color = if object.get_no_match_times() > 1 {
            class_colors.get_lost_color(&class_name)
        } else {
            class_colors.get_color(&class_name)
        };
        let bbox = object.get_bbox();
        let x1 = (bbox.x.floor() as i32).max(0) as usize;
        let y1 = (bbox.y.floor() as i32).max(0) as usize;
        let x2 = ((bbox.x + bbox.width) as usize).min(w.saturating_sub(1));
        let y2 = ((bbox.y + bbox.height) as usize).min(h.saturating_sub(1));
        if x1 >= w || y1 >= h || x2 <= x1 || y2 <= y1 {
            continue;
        }
        let min_dim = (x2 - x1).min(y2 - y1);
        let radius = (min_dim / 8).clamp(2, 12);
        draw_rounded_rect(
            bytes,
            step,
            w,
            h,
            x1,
            y1,
            x2,
            y2,
            radius,
            scalar_to_bgr(&color),
            2,
        );
    }
}

pub fn draw_identifiers(
    img: &mut RawFrame,
    tracker: &dyn TrackerTrait,
    class_colors: &ClassColors,
) {
    let w = img.cols() as usize;
    let h = img.rows() as usize;
    let step = img.step();
    let bytes = img.data_bytes_mut();

    let objects_extra = tracker.get_objects_extra();
    for (object_id, object) in tracker.iter_tracked_objects() {
        let class_name = objects_extra
            .get(&object_id)
            .map(|extra| extra.get_classname())
            .unwrap_or("unknown".to_string());
        let color = if object.get_no_match_times() > 1 {
            class_colors.get_lost_color(&class_name)
        } else {
            class_colors.get_color(&class_name)
        };
        let bbox = object.get_bbox();
        let short_id = object
            .get_id()
            .to_string()
            .chars()
            .take(8)
            .collect::<String>();
        draw_text(
            bytes,
            step,
            w,
            h,
            bbox.x.floor() as i32 + 2,
            bbox.y.floor() as i32 + 2,
            &short_id,
            scalar_to_bgr(&color),
            1,
        );
    }
}

pub fn draw_speeds(img: &mut RawFrame, tracker: &dyn TrackerTrait, class_colors: &ClassColors) {
    let w = img.cols() as usize;
    let h = img.rows() as usize;
    let step = img.step();
    let bytes = img.data_bytes_mut();

    let objects_extra = tracker.get_objects_extra();
    for (object_id, object_extra) in objects_extra.iter() {
        let spatial_info = match object_extra.spatial_info {
            Some(ref spatial_info) => spatial_info,
            None => continue,
        };
        let object = match tracker.get_tracked_object_ref(object_id) {
            Some(obj) => obj,
            None => continue,
        };
        let class_name = object_extra.get_classname();
        let color = if object.get_no_match_times() > 1 {
            class_colors.get_lost_color(&class_name)
        } else {
            class_colors.get_color(&class_name)
        };
        let bbox = object.get_bbox();
        draw_text(
            bytes,
            step,
            w,
            h,
            bbox.x.floor() as i32 + 2,
            bbox.y.floor() as i32 + 12,
            &spatial_info.speed.to_string(),
            scalar_to_bgr(&color),
            1,
        );
    }
}

pub fn draw_projections(
    img: &mut RawFrame,
    tracker: &dyn TrackerTrait,
    class_colors: &ClassColors,
) {
    let w = img.cols() as usize;
    let h = img.rows() as usize;
    let step = img.step();
    let bytes = img.data_bytes_mut();

    let objects_extra = tracker.get_objects_extra();
    for (object_id, object_extra) in objects_extra.iter() {
        let spatial_info = match object_extra.spatial_info {
            Some(ref spatial_info) => spatial_info,
            None => continue,
        };
        let cyan: [u8; 3] = [255, 255, 0]; // BGR
        draw_filled_circle(
            bytes,
            step,
            w,
            h,
            spatial_info.last_x_projected.floor() as i32,
            spatial_info.last_y_projected.floor() as i32,
            2,
            cyan,
        );
    }
}

pub fn invert_color(color: &Scalar) -> Scalar {
    let b = color[0];
    let g = color[1];
    let r = color[2];
    let inv_b = 255.0 - b;
    let inv_g = 255.0 - g;
    let inv_r = 255.0 - r;
    Scalar::from((inv_b, inv_g, inv_r))
}
