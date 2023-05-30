use std::error::Error;
use std::collections::HashMap;
use std::collections::hash_map::Entry::{
    Occupied,
    Vacant
};
use uuid::Uuid;
use mot_rs::mot::{
    SimpleTracker,
    SimpleBlob,
};

use crate::lib::detection::Detections;
use crate::lib::spatial::haversine;

pub struct Tracker {
    pub engine: SimpleTracker,
    pub objects_extra: HashMap<Uuid, ObjectExtra>,
}

pub struct ObjectExtra {
    class_name: String,
    confidence: f32,
    // Timestamps along the whole track
    pub times: Vec<f32>,
    pub estimated_velocity: f32,
    pub spatial_info: Option<SpatialInfo>,
}

impl ObjectExtra {
    pub fn get_classname(&self) -> String {
        self.class_name.clone()
    }
}

pub struct SpatialInfo {
    pub first_time: f32,
    pub first_x_projected: f32,
    pub first_y_projected: f32,
    pub last_time: f32,
    pub last_lon: f32,
    pub last_lat: f32,
    pub last_x: f32,
    pub last_y: f32,
    pub last_x_projected: f32,
    pub last_y_projected: f32,
    pub distance_traveled: f32,
    pub speed: f32,
}

impl SpatialInfo {
    pub fn new(_time: f32,  _x: f32, _y: f32, _x_projected: f32, _y_projected: f32) -> Self {
        Self {
            first_time: _time,
            first_x_projected: _x_projected,
            first_y_projected: _y_projected,
            last_time: _time,
            last_lon: -1.0,
            last_lat: -1.0,
            last_x: _x,
            last_y: _y,
            last_x_projected: _x_projected,
            last_y_projected: _y_projected,
            distance_traveled: -1.0,
            speed: -1.0,
        }
    }
    pub fn new_wgs84(_time: f32, _lon: f32, _lat: f32, _x: f32, _y: f32) -> Self {
        Self {
            first_time: _time,
            first_x_projected: -1.0,
            first_y_projected: -1.0,
            last_time: _time,
            last_lon: _lon,
            last_lat: _lat,
            last_x: _x,
            last_y: _y,
            last_x_projected: -1.0,
            last_y_projected: -1.0,
            distance_traveled: -1.0,
            speed: -1.0,
        }
    }
    // Same as update(), but calculations are done between first and last points
    // This approach helps to avoid situation when distance between two points is approx. 0
    pub fn update_avg(&mut self, _time: f32, _x: f32, _y: f32, _x_projected: f32, _y_projected: f32, pixels_per_meter: f32) {
        // It is possible to calculate speed between two points (old and new)
        let distance_pixels = ((_x_projected - self.first_x_projected).powi(2) + (_y_projected - self.first_y_projected).powi(2)).sqrt();
        let distance_meters = distance_pixels / pixels_per_meter;
        let time_diff = _time - self.first_time;
        let velocity = distance_meters / time_diff; // meters per second
        self.speed = velocity * 3.6; // convert m/s to km/h

        self.last_time = _time;
        self.last_x = _x;
        self.last_y = _y;
        self.last_x_projected = _x_projected;
        self.last_y_projected = _y_projected;
    }
    pub fn update(&mut self, _time: f32, _x: f32, _y: f32, _x_projected: f32, _y_projected: f32, pixels_per_meter: f32) {
        // It is possible to calculate speed between two points (old and new)
        let distance_pixels = ((_x_projected - self.last_x_projected).powi(2) + (_y_projected - self.last_y_projected).powi(2)).sqrt();
        let distance_meters = distance_pixels / pixels_per_meter;
        let time_diff = _time - self.last_time;
        let velocity = distance_meters / time_diff; // meters per second
        self.speed = velocity * 3.6; // convert m/s to km/h

        self.last_time = _time;
        self.last_x = _x;
        self.last_y = _y;
        self.last_x_projected = _x_projected;
        self.last_y_projected = _y_projected;
    }
    fn update_by_wgs84(&mut self, _time: f32, _lon: f32, _lat: f32, _x: f32, _y: f32) {
        // It is possible to calculate speed between two points (old and new)
        let distance = haversine(self.last_lon, self.last_lat, _lon, _lat) * 1000.0;
        let time_diff = _time - self.last_time;
        let velocity = distance / time_diff; // meters per second
        self.distance_traveled = distance;
        self.speed = velocity * 3.6; // convert m/s to km/h
        
        self.last_time = _time;
        self.last_lon = _lon;
        self.last_lat = _lat;
        self.last_x = _x;
        self.last_y = _y;
    }
}
impl Tracker {
    pub fn new(_max_no_match: usize, _min_dist_threshold: f32) -> Self {
        Self {
            engine: SimpleTracker::new(_max_no_match, _min_dist_threshold),
            objects_extra: HashMap::new(),
        }
    }
    pub fn match_objects(&mut self, detections: &mut Detections, current_second: f32) -> Result<(), Box<dyn Error>>{
        match self.engine.match_objects(&mut detections.blobs) {
            Ok(_) => {
            }
            Err(err) => {
                return Err(err)
            },
        }

        // println!("id;times");
        // Update extra information for each object
        for (idx, detection) in detections.blobs.iter().enumerate() {
            let object_id = detection.get_id();
            // self.objects_extra.entry(object_id)
            //     .and_modify(|entry| {
            //     })
            //     .or_insert({
            //         let object_extra = ObjectExtra {
            //             class_name: detections.class_names[idx].to_owned(),
            //             confidence: detections.confidences[idx],
            //         };
            //         object_extra
            //     });
            match self.objects_extra.entry(object_id) {
                Occupied(mut entry) => {
                    // Object exists in both hash maps, so update the extra information
                    entry.get_mut().times.push(current_second);
                    // Make sure that the times vector matches track
                    if entry.get().times.len() > detection.get_max_track_len() {
                        entry.get_mut().times = entry.get_mut().times[1..].to_vec();
                    }
                    // print!("{}_{}", object_id, detection.get_no_match_times());
                    // let times = entry.get().times.as_slice();
                    // for (idx, val) in times.iter().enumerate() {
                    //     if idx == times.len() - 1 {
                    //         print!("{}", val);
                    //     } else {
                    //         print!("{}|", val);
                    //     }
                    // }
                    // println!();
                }
                Vacant(entry) => {
                    // Object is a new one, so add it to the hash map (with extra information)
                    let mut object_extra = ObjectExtra {
                        class_name: detections.class_names[idx].to_owned(),
                        confidence: detections.confidences[idx],
                        times:  Vec::with_capacity(detection.get_max_track_len()),
                        estimated_velocity: -1.0,
                        spatial_info: None,
                    };
                    object_extra.times.push(current_second);
                    // print!("{}-initial_{}", object_id, detection.get_no_match_times());
                    // let times = object_extra.times.as_slice();
                    // for (idx, val) in times.iter().enumerate() {
                    //     if idx == times.len() - 1 {
                    //         print!("{}", val);
                    //     } else {
                    //         print!("{}|", val);
                    //     }
                    // }
                    // println!();
                    entry.insert(object_extra);
                }
            }
            
        }

        // Remove obsolete objects
        let ref_engine_objects = &self.engine.objects;
        self.objects_extra.retain(|object_id, _| {
            let save = ref_engine_objects.contains_key(object_id);
            save
        });
        Ok(())        
    }
}

use std::fmt;
impl fmt::Display for Tracker {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.engine)
    }
}
