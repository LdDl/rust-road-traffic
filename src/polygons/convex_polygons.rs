use crate::settings::RoadLanesSettings;

use opencv::{
    core::Point,
    core::Scalar,
};

impl RoadLanesSettings {
    pub fn convert_to_convex_polygon(&self) -> ConvexPolygon{
        return ConvexPolygon{
            coordinates: self.geometry
                .iter()
                .map(|pt| Point::new(pt[0], pt[1]))
                .collect(),
            // RGB to OpenCV = [B, G, R]. So use reverse order
            color: Scalar::from((self.color_rgb[2] as f64, self.color_rgb[1] as f64, self.color_rgb[0] as f64)),
        }
    }
}

use opencv::{
    core::Mat,
    imgproc::LINE_8,
    imgproc::line
};

#[derive(Debug)]
pub struct ConvexPolygon {
    coordinates: Vec<Point>,
    color: Scalar
}

impl ConvexPolygon {
    pub fn draw_on_mat(&self, img: &mut Mat) {
        // @todo: proper error handling
        for i in 1..self.coordinates.len() {
            let prev_pt = self.coordinates[i - 1];
            let current_pt = self.coordinates[i];
            match line(img, prev_pt, current_pt, self.color, 2, LINE_8, 0) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't draw line for polygon due the error: {:?}", err)
                }
            };
        }
        match line(img, self.coordinates[self.coordinates.len() - 1], self.coordinates[0], self.color, 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw line for polygon due the error: {:?}", err)
            }
        };
    }
}
