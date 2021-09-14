use crate::tracking::{
    KalmanWrapper,
    KalmanModelType
};

use opencv::{
    core::Mat,
    core::Rect,
    core::Point,
    core::Scalar,
    imgproc::LINE_8,
    imgproc::circle,
};

use uuid::Uuid;

pub struct KalmanBlobie {
    id: Uuid,
    center: Point,
    // kf: Cell<KalmanWrapper>
    kf: KalmanWrapper
}


impl KalmanBlobie {
    pub fn new(rect: &Rect, kalman_type: KalmanModelType) -> Self {
        let center_x = rect.x as f32 + 0.5 * rect.width as f32;
        let center_y = rect.y as f32 + 0.5 * rect.height as f32;
        let center = Point::new(center_x.round() as i32, center_y.round() as i32);
        let kf = KalmanWrapper::new(kalman_type);
        let kb = KalmanBlobie {
            id : Uuid::new_v4(),
            center: center,
            kf: kf
        };
        return kb 
    }
    pub fn get_center(&self) -> Point {
        return self.center;
    }
    pub fn update(&mut self, newb: &KalmanBlobie) {
        let new_center = newb.get_center();
        // let new_center_x = new_center.x;
        // let new_center_y = new_center.y;
        self.kf.correct(new_center.x as f32, new_center.y as f32);
    }
    pub fn draw(&self, img: &mut Mat) {
        match circle(img, self.center, 5, Scalar::from((255.0, 0.0, 0.0)), 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw circle and blob's center due the error: {:?}", err)
            }
        };
    }
}