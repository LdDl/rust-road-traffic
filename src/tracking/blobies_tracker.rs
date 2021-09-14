use crate::tracking::{
    KalmanBlobie,
};

use uuid::Uuid;
use std::collections::HashMap;

pub struct KalmanBlobiesTracker {
    objects: HashMap<Uuid, KalmanBlobie>,
    max_no_match: usize,
    min_threshold_distance: f32,
    map_points_in_track: i32
}

impl KalmanBlobiesTracker {
    pub fn default() -> Self {
        return KalmanBlobiesTracker{
            objects: HashMap::new(),
            max_no_match: 30,
            min_threshold_distance: 15.0,
            map_points_in_track: 100,
        }
    }
    pub fn get_objects_num(&self) -> usize {
        return self.objects.len();
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
        // @todo: handle panic!() call
        self.prepare();
        let mut blobies_to_register = vec![];
        for (i, b) in blobies.iter_mut().enumerate() {
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
                    None => {
                        // continue
                        panic!("immposible self.objects.get_mut(&min_id)")
                    }
                };
            } else {
                let new_id = Uuid::new_v4();
                b.set_id(new_id);
                blobies_to_register.push(i)
            }
        }
        for (i, _) in blobies_to_register.iter().enumerate() {
            // @todo: arghhhh. Can't understand pointer's rust-ish stuff
            // let b = blobies[i];
            // self.objects.entry(b.get_id()).or_insert_with(|| b); // <----- here is an compile-time error
            // @todo: so create new blob.
            let b = &blobies[i];
            let copy_b = KalmanBlobie::new(&b.get_current_rect(), b.get_kalman_model_type(), b.get_max_points_in_track());
            self.objects.entry(b.get_id()).or_insert_with(|| copy_b);
        }
        let delete_blobs = self.refresh_no_match();
        for delete_id in delete_blobs {
            self.objects.remove(&delete_id);
        }
    }
}