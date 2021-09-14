use crate::tracking::{
    KalmanBlobie,
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
use std::collections::HashMap;

pub struct KalmanBlobiesTracker {
    objects: HashMap<Uuid, KalmanBlobie>,
    max_no_match: i32,
    min_threshold_distance: f32,
    map_points_in_track: i32
}

impl KalmanBlobiesTracker {
    pub fn default() -> Self {
        return KalmanBlobiesTracker{
            objects: HashMap::new(),
            max_no_match: 5,
            min_threshold_distance: 15.0,
            map_points_in_track: 100,
        }
    }
    fn prepare(&mut self) {
        for (_, b) in self.objects.iter_mut() {
            b.set_exists(false);
            b.predict_next_position(self.max_no_match);
        }
    }
    fn refresh_no_match(&mut self) -> Vec<Uuid> {
        let mut delete_blobs = vec![];
        for (blob_id, b) in self.objects.iter_mut() {
            if b.exists() == false {
                b.increment_no_match_times()
            }
            if b.no_match_times() > self.max_no_match {
                b.set_tracking(false);
                delete_blobs.push(*blob_id);
            }
        }
        return delete_blobs
    }
    pub fn match_to_existing(&mut self, blobies: &mut Vec<KalmanBlobie>) {
        self.prepare();
        for b in blobies.iter_mut() {
            let mut min_id = Uuid::default();
            let mut min_distance = f32::MAX;
            for (j, sb) in self.objects.iter() {
                let dist = b.distance_to(sb);
                let dist_predicted = b.distance_to_predicted(sb);
                let dist_verified = f32::min(dist, dist_predicted);
                if dist_verified < min_distance {
                    min_distance = dist_verified;
                    min_id = *j;
                }
            }
            if min_distance < b.get_diagonal() * 0.5 || min_distance < self.min_threshold_distance {
                match self.objects.get_mut(&min_id) {
                    Some(v) => v.update(b),
                    None => continue
                };
            } else {
                self.register(b);
            }
        }
        let delete_blobs = self.refresh_no_match();
        for delete_id in delete_blobs {
            self.objects.remove(&delete_id);
        }
    }
    fn deregister(&mut self, delete_id: &Uuid) {
        self.objects.remove(delete_id);
    }
    pub fn register(&mut self,  b: &mut KalmanBlobie) {
        // @todo
        // let new_id = Uuid::new_v4();
        // b.set_id(new_id);
        // // self.objects.insert(new_id, b);
    }
}