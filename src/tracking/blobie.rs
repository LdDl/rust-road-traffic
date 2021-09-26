use crate::tracking::{
    KalmanWrapper,
    KalmanModelType,
    KalmanFilterLinear,
    Matrix2x1f32,
    Matrix4x1f32
};

use opencv::{
    core::Mat,
    core::Rect,
    core::Point,
    core::Scalar,
    imgproc::LINE_8,
    imgproc::FONT_HERSHEY_SIMPLEX,
    imgproc::circle,
    imgproc::rectangle,
    imgproc::put_text
};

use uuid::Uuid;
use crate::tracking::utils;

pub struct KalmanBlobie {
    id: Uuid,
    class_name: String,
    center: Point,
    predicted_next_position: Point,
    current_rect: Rect,
    diagonal: f32,
    exists: bool,
    no_match_times: usize,
    max_points_in_track: usize,
    is_still_tracked: bool,
    track: Vec<Point>,
    custom_kf: KalmanFilterLinear
}

impl KalmanBlobie {
    pub fn new(rect: &Rect, max_points_in_track: usize) -> Self {
        let center_x = rect.x as f32 + 0.5 * rect.width as f32;
        let center_y = rect.y as f32 + 0.5 * rect.height as f32;
        // let center = Point::new(center_x.round() as i32, center_y.round() as i32);
        let center = Point::new(center_x as i32, center_y as i32);
        let diagonal = f32::sqrt((i32::pow(rect.width, 2) + i32::pow(rect.height, 2)) as f32);
        let mut custom_kf = KalmanFilterLinear::new();
        custom_kf.set_time(1.0);
        let kb = KalmanBlobie {
            id : Uuid::new_v4(),
            class_name: "Undefined".to_string(),
            center: center,
            predicted_next_position: Point::default(),
            current_rect: *rect,
            diagonal: diagonal,
            exists: true,
            no_match_times: 0,
            max_points_in_track: max_points_in_track,
            is_still_tracked: true,
            track: vec![center],
            custom_kf: custom_kf
        };
        return kb 
    }
    pub fn partial_copy(newb: &KalmanBlobie) -> Self {
        let mut copy_b = KalmanBlobie::new(&newb.get_current_rect(), newb.get_max_points_in_track());
        copy_b.set_class_name(newb.get_class_name());
        return copy_b;
    }
    pub fn set_id(&mut self, id: Uuid) {
        self.id = id;
    }
    pub fn get_id(&self) -> Uuid {
        return self.id;
    }
    pub fn set_class_name(&mut self, class_name: String) {
        self.class_name = class_name;
    }
    pub fn get_class_name(&self) -> String {
        return self.class_name.clone();
    }
    pub fn get_exists(&self) -> bool {
        return self.exists;
    }
    pub fn set_exists(&mut self, exists: bool) {
        self.exists = exists;
    }
    pub fn get_tracking(&self) -> bool{
        return self.is_still_tracked;
    }
    pub fn set_tracking(&mut self, is_still_tracked: bool) {
        self.is_still_tracked = is_still_tracked;
    }
    pub fn increment_no_match_times(&mut self) {
        self.no_match_times += 1;
    }
    pub fn get_no_match_times(&self) -> usize {
        return self.no_match_times;
    }
    pub fn get_center(&self) -> Point {
        return self.center;
    }
    pub fn get_predicted_center(&self) -> Point {
        return self.predicted_next_position;
    }
    pub fn get_current_rect(&self) -> Rect {
        return self.current_rect;
    }
    pub fn get_diagonal(&self) -> f32 {
        return self.diagonal;
    }
    pub fn get_max_points_in_track(&self) -> usize {
        return self.max_points_in_track;
    }
    pub fn distance_to(&self, b: &KalmanBlobie) -> f32 {
        return utils::euclidean_distance(self.center, b.get_center());
    }
    pub fn distance_to_predicted(&self, b: &KalmanBlobie) -> f32 {
        return utils::euclidean_distance(self.center, b.get_predicted_center());
    }
    pub fn predict_next_position(&mut self, max_no_match: usize) {
        let track_len = self.track.len();
        let account = usize::min(max_no_match, track_len);
        if account <= 1 {
            self.predicted_next_position.x = 0;
            self.predicted_next_position.y = 0;
            return
        }
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
            if current != 0 {
                prev = current - 1;
            }
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
        let y = Matrix2x1f32::new(
            new_center.x as f32,
            new_center.y as f32
        );
        // tracker.set_time(dt);
        let u = Matrix4x1f32::new(
            0.0,
            0.0,
            0.0,
            0.0,
        );
        let predicted_custom = self.custom_kf.step(u, y).unwrap();
        // let predicted = Point::new(predicted_custom[(0, 0)].round() as i32, predicted_custom[(1, 0)].round() as i32);
        let predicted = Point::new(predicted_custom[(0, 0)] as i32, predicted_custom[(1, 0)] as i32);
        self.center = predicted;
        let diff_x = predicted.x-newb.center.x;
        let diff_y = predicted.y-newb.center.y;
        self.current_rect = Rect::new(
            newb.current_rect.x-diff_x,
            newb.current_rect.y-diff_y,
            newb.current_rect.width-diff_x,
            newb.current_rect.height-diff_y
        );
        self.diagonal = newb.diagonal;
        self.is_still_tracked = true;
        self.exists = true;
        self.track.push(self.center);
        // Restrict number of points in track (shift to the left)
        if self.track.len() > self.max_points_in_track {
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
    pub fn draw_track(&self, img: &mut Mat) {
        for pt in self.track.iter() {
            match circle(img, *pt, 5, Scalar::from((0.0, 255.0, 0.0)), 2, LINE_8, 0) {
                Ok(_) => {},
                Err(err) => {
                    panic!("Can't draw circle at blob's center due the error: {:?}", err)
                }
            };
        }
    }
    pub fn draw_rectangle(&self, img: &mut Mat) {
        match rectangle(img, self.current_rect, Scalar::from((0.0, 255.0, 0.0)), 2, 1, 0) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't draw bounding box of object due the error {:?}", err);
            }
        };
    }
    pub fn draw_class_name(&self, img: &mut Mat) {
        let anchor = Point::new(self.current_rect.x + 2, self.current_rect.y + 3);
        match put_text(img, &self.class_name, anchor, FONT_HERSHEY_SIMPLEX, 1.5, Scalar::from((0.0, 255.0, 255.0)), 2, LINE_8, false) {
            Ok(_) => {},
            Err(err) => {
                println!("Can't display classname of object due the error {:?}", err);
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_kalman_prediction() {
        let points = vec![
            vec![0, 0],
            vec![1, 1],
            vec![2, 2],
            vec![4, 4],
            vec![6, 6],
            vec![9, 9],
            vec![11, 11],
            vec![16, 16],
            vec![20, 20]
        ];
        let kalman_points = vec![
            vec![0, 0],
			vec![0, 0],
			vec![1, 1],
			vec![3, 3],
			vec![5, 5],
			vec![7, 7],
			vec![10, 10],
			vec![13, 13],
			vec![17, 17],
        ];
        let correct_predictions = vec![
            vec![0, 0],
			vec![0, 0],
			vec![0, 0],
			vec![1, 1],
			vec![4, 4],
			vec![6, 6],
			vec![8, 8],
			vec![12, 12],
			vec![15, 15],
        ];

        assert_eq!(points.len(), kalman_points.len());
        assert_eq!(points.len(), correct_predictions.len());
        assert_eq!(kalman_points.len(), correct_predictions.len());

        let max_points_in_track = 150;
        let max_no_match = 5;
        let rect_half_height = 30;
	    let rect_half_width = 75;

        let center_one = &points[0];
        let rect_one = Rect::new(center_one[0]-rect_half_width, center_one[1]-rect_half_height, 2*rect_half_width, 2*rect_half_height);
        let mut b: KalmanBlobie = KalmanBlobie::new(&rect_one, max_points_in_track);
        let blob_one = KalmanBlobie::new(&rect_one, max_points_in_track);
        b.predict_next_position(max_no_match);
        b.update(&blob_one);

        for i in 1..points.len() {
            let center_one = &points[i];
            let rect_one = Rect::new(center_one[0]-rect_half_width, center_one[1]-rect_half_height, 2*rect_half_width, 2*rect_half_height);
            let blob_one = KalmanBlobie::new(&rect_one, max_points_in_track);
            b.predict_next_position(max_no_match);
            b.update(&blob_one);
            
            let check_x = b.center.x;
            let check_y = b.center.y;
            let smoothed_x = kalman_points[i][0];
            let smoothed_y = kalman_points[i][1];
            assert_eq!(check_x, smoothed_x);
            assert_eq!(check_y, smoothed_y);

            let predicted_x = b.predicted_next_position.x;
            let predicted_y = b.predicted_next_position.y;
            let correct_x = correct_predictions[i][0];
            let correct_y = correct_predictions[i][1];
            assert_eq!(predicted_x, correct_x);
            assert_eq!(predicted_y, correct_y);
        }
    }
}