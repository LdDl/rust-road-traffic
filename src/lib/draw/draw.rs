use opencv::{
    core::Mat,
    core::Rect,
    core::Point,
    core::Scalar,
    imgproc::LINE_8,
    imgproc::LINE_4,
    imgproc::FONT_HERSHEY_SIMPLEX,
    imgproc::circle,
    imgproc::rectangle,
    imgproc::put_text,
};

use crate::lib::tracker::Tracker;

pub fn draw_trajectories(img: &mut Mat, tracker: &Tracker, color: Scalar, inv_color: Scalar) {
    for (_, object) in tracker.engine.objects.iter() {
        let mut color_choose = color;
        if object.get_no_match_times() > 1 {
            color_choose = inv_color;
        }
        for pt in object.get_track().iter() {
            let cv_pt = Point::new(pt.x.floor() as i32, pt.y.floor() as i32);
            match circle(img, cv_pt, 5, color_choose, 2, LINE_8, 0) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't draw circle at blob's center due the error: {:?}", err)
                }
            };
        }
    }
}

pub fn draw_bboxes(img: &mut Mat, tracker: &Tracker, color: Scalar, inv_color: Scalar) {
    for (_, object) in tracker.engine.objects.iter() {
        let mut color_choose = color;
        if object.get_no_match_times() > 1 {
            color_choose = inv_color;
        }
        let bbox = object.get_bbox();
        let cv_rect = Rect::new(bbox.x.floor() as i32, bbox.y.floor() as i32, bbox.width as i32, bbox.height as i32);
        match rectangle(img, cv_rect, color_choose, 2, LINE_4, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw rectangle at blob's bbox due the error: {:?}", err)
            }
        };
    }
}

pub fn draw_identifiers(img: &mut Mat, tracker: &Tracker, color: Scalar, inv_color: Scalar) {
    for (_, object) in tracker.engine.objects.iter() {
        let mut color_choose = color;
        if object.get_no_match_times() > 1 {
            color_choose = inv_color;
        }
        let bbox = object.get_bbox();
        let anchor = Point::new(bbox.x.floor() as i32 + 2, bbox.y.floor() as i32 + 10);
        match put_text(img, &object.get_id().to_string(), anchor, FONT_HERSHEY_SIMPLEX, 0.5, color_choose, 2, LINE_8, false) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't display ID of object due the error {:?}", err);
            }
        };
    }
}

pub fn draw_speeds(img: &mut Mat, tracker: &Tracker, color: Scalar, inv_color: Scalar) {
    for (object_id, object_extra) in tracker.objects_extra.iter() {
        let spatial_info = match object_extra.spatial_info {
            Some(ref spatial_info) => spatial_info,
            None => continue,
        };
        let object = tracker.engine.objects.get(&object_id).unwrap();
        let mut color_choose = color;
        if object.get_no_match_times() > 1 {
            color_choose = inv_color;
        }
        let bbox = object.get_bbox();
        let anchor = Point::new(bbox.x.floor() as i32 + 2, bbox.y.floor() as i32 + 20);
        match put_text(img, &spatial_info.speed.to_string(), anchor, FONT_HERSHEY_SIMPLEX, 0.5, color_choose, 2, LINE_8, false) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't display velocity of object due the error {:?}", err);
            }
        };
    }
}

pub fn draw_projections(img: &mut Mat, tracker: &Tracker, color: Scalar, inv_color: Scalar) {
    for (object_id, object_extra) in tracker.objects_extra.iter() {
        let spatial_info = match object_extra.spatial_info {
            Some(ref spatial_info) => spatial_info,
            None => continue,
        };
        let cv_pt = Point::new(spatial_info.last_x_projected.floor() as i32, spatial_info.last_y_projected.floor() as i32);
        match circle(img, cv_pt, 5, Scalar::from((255.0, 255.0, 0.0)), 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw circle at blob's projected center due the error: {:?}", err)
            }
        };
        // let object = tracker.engine.objects.get(&object_id).unwrap();
        // let mut color_choose = color;
        // if object.get_no_match_times() > 1 {
        //     color_choose = inv_color;
        // }
        // let bbox = object.get_bbox();
        // let anchor = Point::new(bbox.x.floor() as i32 + 2, bbox.y.floor() as i32 + 20);
        // match put_text(img, &spatial_info.speed.to_string(), anchor, FONT_HERSHEY_SIMPLEX, 0.5, color_choose, 2, LINE_8, false) {
        //     Ok(_) => {},
        //     Err(err) => {
        //         println!("Can't display velocity of object due the error {:?}", err);
        //     }
        // };
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