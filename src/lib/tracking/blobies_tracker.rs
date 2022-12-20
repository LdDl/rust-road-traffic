use crate::lib::tracking::{
    KalmanBlobie,
};

use uuid::Uuid;
use std::collections::HashMap;

pub struct KalmanBlobiesTracker {
    pub objects: HashMap<Uuid, KalmanBlobie>,
    max_no_match: usize,
    min_threshold_distance: f32
}

impl KalmanBlobiesTracker {
    pub fn default() -> Self {
        return KalmanBlobiesTracker{
            objects: HashMap::new(),
            max_no_match: 5,
            min_threshold_distance: 30.0
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
            if b.get_exists() == false {
                b.increment_no_match_times()
            }
            if b.get_no_match_times() > self.max_no_match {
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
                    Some(v) => {
                        v.update(b);
                    },
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
        for (_, i) in blobies_to_register.iter().enumerate() {
            // @todo: make it more Rust-ish
            let b = &blobies[*i];
            let copy_b = KalmanBlobie::partial_copy(b);
            self.objects.entry(b.get_id()).or_insert_with(|| copy_b);
        }
        let delete_blobs = self.refresh_no_match();
        for delete_id in delete_blobs {
            self.objects.remove(&delete_id);
        }
    }
}

use std::fmt;
impl fmt::Display for KalmanBlobiesTracker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Maximum no match: {}\n\tMinimum threshold distance: {}",
            self.max_no_match,
            self.min_threshold_distance
        )
    }
}

#[cfg(test)]
mod tests {
    use opencv::{
        core::Rect,
    };
    use chrono::{
        Utc
    };
    use super::*;
    #[test]
    fn test_match_to_existing() {
        let mut tracker = KalmanBlobiesTracker::default();
        let mut blobies = vec![];
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(318, 242,  46,  44), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(375, 376,  92, 102), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(375, 238,  45,  42), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(313, 312,  78,  82), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(233, 425, 146,  75), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(561, 283,  77,  71), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(181, 173, 112, 241), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(287, 196,  42,  34), 0, 0.0));
        blobies.push(KalmanBlobie::new_with_time(&Rect::new(418, 432, 166,  6), 0, 0.0));

        for b in blobies.iter() {
            println!("{:?}", b.get_center());
        }
        tracker.match_to_existing(&mut blobies);
        tracker.match_to_existing(&mut blobies);
        // @todo: complete test
    }
}