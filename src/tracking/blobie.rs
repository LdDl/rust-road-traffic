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
use crate::tracking::utils;

pub struct KalmanBlobie {
    id: Uuid,
    center: Point,
    predicted_next_position: Point,
    diagonal: f32,
    exists: bool,
    no_match_times: i32,
    is_still_tracked: bool,
    // kf: Cell<KalmanWrapper>
    kf: KalmanWrapper,
}

impl KalmanBlobie {
    pub fn new(rect: &Rect, kalman_type: KalmanModelType) -> Self {
        let center_x = rect.x as f32 + 0.5 * rect.width as f32;
        let center_y = rect.y as f32 + 0.5 * rect.height as f32;
        let center = Point::new(center_x.round() as i32, center_y.round() as i32);
        let diagonal = f32::sqrt((i32::pow(rect.width, 2) + i32::pow(rect.height, 2)) as f32);
        let kf = KalmanWrapper::new(kalman_type);
        let kb = KalmanBlobie {
            id : Uuid::new_v4(),
            center: center,
            predicted_next_position: Point::default(),
            diagonal: diagonal,
            exists: true,
            no_match_times: 0,
            is_still_tracked: true,
            kf: kf
        };
        return kb 
    }
    pub fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }
    pub fn set_exists(&mut self, exists: bool) {
        self.exists = exists;
    }
    pub fn increment_no_match_times(&mut self) {
        self.no_match_times += 1;
    }
    pub fn set_tracking(&mut self, is_still_tracked: bool) {
        self.is_still_tracked = is_still_tracked;
    }
    pub fn exists(&self) -> bool {
        return self.exists;
    }
    pub fn no_match_times(&self) -> i32 {
        return self.no_match_times;
    }
    pub fn get_center(&self) -> Point {
        return self.center;
    }
    pub fn get_predicted_center(&self) -> Point {
        return self.predicted_next_position;
    }
    pub fn get_diagonal(&self) -> f32 {
        return self.diagonal;
    }
    pub fn distance_to(&self, b: &KalmanBlobie) -> f32 {
        return utils::euclidean_distance(self.center, b.get_center());
    }
    pub fn distance_to_predicted(&self, b: &KalmanBlobie) -> f32 {
        return utils::euclidean_distance(self.center, b.get_predicted_center());
    }
    pub fn predict_next_position(&mut self, max_no_match: i32) {
        // @todo
    }
    pub fn update(&mut self, newb: &KalmanBlobie) {
        // @todo
        // let new_center = newb.get_center();
        // self.kf.correct(new_center.x as f32, new_center.y as f32);
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