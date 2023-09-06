use opencv::{
    core::Mat,
    core::Point2i,
    core::Point2f,
    core::Scalar,
    imgproc::line,
    imgproc::LINE_8,
};

#[derive(Debug)]
pub struct Skeleton {
    line_cvf: [Point2f; 2],
    line_cvi: [Point2i; 2],
    color: Scalar,
    pub length_pixels: f32,
    pub length_meters: f32,
    pub pixels_per_meter: f32,
}

impl Skeleton {
    pub fn new(a: Point2f, b: Point2f) -> Self {
        let length_pixels = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
        Skeleton {
            line_cvf: [a, b],
            line_cvi: [Point2i::new(a.x as i32, a.y as i32), Point2i::new(b.x as i32, b.y as i32)],
            color: Scalar::from((0.0, 0.0, 0.0)),
            length_pixels: length_pixels,
            length_meters: -1.0,
            pixels_per_meter: -1.0,
        }
    }
    pub fn default() -> Self {
        Skeleton {
            line_cvf: [Point2f::default(), Point2f::default()],
            line_cvi: [Point2i::default(), Point2i::default()],
            color: Scalar::from((0.0, 0.0, 0.0)),
            length_pixels: -1.0,
            length_meters: -1.0,
            pixels_per_meter: -1.0,
        }
    }
    pub fn project(&self, x: f32, y: f32) -> (f32, f32) {
        let a = self.line_cvf[0];
        let b = self.line_cvf[1];
        let (x1, y1) = (a.x, a.y);
        let (x2, y2) = (b.x, b.y);
        let (x_p, y_p) = (x, y);

        // Calculate vector components of AB
        let ab_x = x2 - x1;
        let ab_y = y2 - y1;

        // Calculate vector components of AP
        let ap_x = x_p - x1;
        let ap_y = y_p - y1;

        // Calculate the dot product of AB and AP
        let dot_product = ap_x * ab_x + ap_y * ab_y;

        // Calculate the magnitude of AB squared
        let ab_squared = ab_x.powi(2) + ab_y.powi(2);

        // Calculate the scalar projection of P onto AB
        let scalar_projection = dot_product / ab_squared;
        
        if scalar_projection < 0.0 {
            // P is closest to point A, so use A as the projection point
            (a.x, a.y)
        } else if scalar_projection > 1.0 {
            // P is closest to point B, so use B as the projection point
            (b.x, b.y)
        } else {
            // Calculate the coordinates of the projected point P' on AB
            let x_p_prime = x1 + scalar_projection * ab_x;
            let y_p_prime = y1 + scalar_projection * ab_y;
            (x_p_prime, y_p_prime)
        }
    }
    pub fn draw_on_mat(&self, img: &mut Mat) {
        match line(img, self.line_cvi[0], self.line_cvi[1], self.color, 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw skeleton line for polygon due the error: {:?}", err)
            }
        };

    }
}