use opencv::{
    core::Mat,
    core::Rect,
    core::Point,
    core::Scalar,
    core::Size,
    imgproc::LINE_8,
    imgproc::FONT_HERSHEY_SIMPLEX,
    imgproc::circle,
    imgproc::ellipse,
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
        // Use rounded rectangle instead of regular rectangle
        match draw_rounded_rectangle(img, cv_rect, color_choose, 2, 8) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw rounded rectangle at blob's bbox due the error: {:?}", err)
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
        let short_id = object.get_id().to_string().chars().take(8).collect::<String>();
        match put_text(img, &short_id, anchor, FONT_HERSHEY_SIMPLEX, 0.5, color_choose, 2, LINE_8, false) {
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

// Add this new function for drawing rounded rectangles
pub fn draw_rounded_rectangle(img: &mut Mat, rect: Rect, color: Scalar, thickness: i32, corner_radius: i32) -> opencv::Result<()> {
    let x = rect.x;
    let y = rect.y;
    let width = rect.width;
    let height = rect.height;
    
    // Calculate adaptive corner radius based on bbox size
    let min_dimension = width.min(height);
    let max_corner_radius = min_dimension / 8; // Corner radius won't exceed 25% of the smallest dimension
    let adaptive_radius = corner_radius.min(max_corner_radius).max(2); // Minimum radius of 2 pixels
    
    // Draw the four corner arcs
    let arc_size = Size::new(adaptive_radius * 2, adaptive_radius * 2);
    
    // Top-left corner
    ellipse(img, 
        Point::new(x + adaptive_radius, y + adaptive_radius),
        arc_size,
        180.0, 0.0, 90.0,
        color, thickness, LINE_8, 0)?;
    
    // Top-right corner
    ellipse(img,
        Point::new(x + width - adaptive_radius, y + adaptive_radius),
        arc_size,
        270.0, 0.0, 90.0,
        color, thickness, LINE_8, 0)?;
    
    // Bottom-right corner
    ellipse(img,
        Point::new(x + width - adaptive_radius, y + height - adaptive_radius),
        arc_size,
        0.0, 0.0, 90.0,
        color, thickness, LINE_8, 0)?;
    
    // Bottom-left corner
    ellipse(img,
        Point::new(x + adaptive_radius, y + height - adaptive_radius),
        arc_size,
        90.0, 0.0, 90.0,
        color, thickness, LINE_8, 0)?;
    
    Ok(())
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