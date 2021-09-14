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
    imgproc::rectangle
};

use uuid::Uuid;
use crate::tracking::utils;

pub struct KalmanBlobie {
    id: Uuid,
    center: Point,
    predicted_next_position: Point,
    current_rect: Rect,
    diagonal: f32,
    exists: bool,
    no_match_times: usize,
    map_points_in_track: usize,
    is_still_tracked: bool,
    track: Vec<Point>,
    kf: KalmanWrapper,
}

impl KalmanBlobie {
    pub fn new(rect: &Rect, kalman_type: KalmanModelType, map_points_in_track: usize) -> Self {
        let center_x = rect.x as f32 + 0.5 * rect.width as f32;
        let center_y = rect.y as f32 + 0.5 * rect.height as f32;
        let center = Point::new(center_x.round() as i32, center_y.round() as i32);
        let diagonal = f32::sqrt((i32::pow(rect.width, 2) + i32::pow(rect.height, 2)) as f32);
        let kf = KalmanWrapper::new(kalman_type);
        let kb = KalmanBlobie {
            id : Uuid::new_v4(),
            center: center,
            predicted_next_position: Point::default(),
            current_rect: *rect,
            diagonal: diagonal,
            exists: true,
            no_match_times: 0,
            map_points_in_track: 100,
            is_still_tracked: true,
            track: vec![center],
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
    pub fn no_match_times(&self) -> usize {
        return self.no_match_times;
    }
    pub fn get_id(&self) -> Uuid {
        return self.id;
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
    pub fn predict_next_position(&mut self, max_no_match: usize) {
        let track_len = self.track.len();
        if track_len < 2 {
            return;
        }
        let account = usize::min(max_no_match, track_len);
        let mut current = track_len - 1;
        let mut prev = current - 1;
        let mut delta_x = 0;
        let mut delta_y = 0;
        let mut sum = 0;
        for i in 1..account {
            let weight = (account - i) as i32;
            delta_x += (self.track[current].x - self.track[prev].x) * weight;
		    delta_y += (self.track[current].y - self.track[prev].y) * weight;
            sum += i as i32;
            current = prev;
            prev = current - 1;
        }
        if sum > 0 {
            delta_x /= sum;
            delta_y /= sum;
        }
        self.predicted_next_position.x = self.track[track_len - 1].x + delta_x;
        self.predicted_next_position.y = self.track[track_len - 1].y + delta_y;
    }
    pub fn update(&mut self, newb: &KalmanBlobie) {
        // @todo: handle possible error instead of unwrap() call
        let new_center = newb.get_center();
        let predicted = self.kf.predict().unwrap();
        self.center = predicted;
        self.kf.correct(new_center.x as f32, new_center.y as f32);
        let diff_x = predicted.x-newb.center.x;
        let diff_y = predicted.y-newb.center.y;
        self.current_rect = Rect::new(
            newb.current_rect.x-diff_x,
            newb.current_rect.y-diff_y,
            newb.current_rect.width-diff_x,
            newb.current_rect.width-diff_y
        );
        self.diagonal = newb.diagonal;
        self.is_still_tracked = true;
        self.exists = true;
        self.track.push(self.center);
        // Restrict number of points in track (shift to the left)
        if self.track.len() > self.map_points_in_track {
            self.track = self.track[1..].to_vec();
        }
    }
    pub fn draw_center(&self, img: &mut Mat) {
        match circle(img, self.center, 5, Scalar::from((255.0, 0.0, 0.0)), 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw circle at blob's center due the error: {:?}", err)
            }
        };
    }
    pub fn draw_predicted(&self, img: &mut Mat) {
        match circle(img, self.predicted_next_position, 5, Scalar::from((0.0, 0.0, 255.0)), 2, LINE_8, 0) {
            Ok(_) => {},
            Err(err) => {
                panic!("Can't draw circle at blob's predicted position due the error: {:?}", err)
            }
        };
    }
    pub fn draw_rectangle(&self, img: &mut Mat) {
        match rectangle(img, self.current_rect, Scalar::from((0.0, 255.0, 0.0)), 2, 1, 0) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't draw bounding box of object due the error {:?}", err);
            }
        };
    }
}