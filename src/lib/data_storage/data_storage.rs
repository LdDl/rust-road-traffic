use std::collections::{
    HashMap
};

use std::sync::{
    Arc,
    Mutex,
    RwLock
};

use std::{
    thread
};

use std::time::{
    Duration as STDDuration
};

use chrono::{
    DateTime,
    Utc,
    Duration
};

use crate::lib::polygons::{
    ConvexPolygon
};

#[derive(Clone)]
pub struct DataStorage {
    pub polygons: Arc<RwLock<HashMap<String, Mutex<ConvexPolygon>>>>,
    pub period_start: DateTime<Utc>,
    pub period_end: Option<DateTime<Utc>>,
    pub id: String,
}

impl DataStorage {
    pub fn new() -> Self {
        let now = Utc::now();
        return DataStorage {
            polygons: Arc::new(RwLock::new(HashMap::<String, Mutex<ConvexPolygon>>::new())),
            period_start: now,
            period_end: None,
            id: "Empty ID".to_string(),
        };
    }
    pub fn new_with_id(_id: String) -> Self {
        let now = Utc::now();
        return DataStorage {
            polygons: Arc::new(RwLock::new(HashMap::<String, Mutex<ConvexPolygon>>::new())),
            period_start: now,
            period_end: None,
            id: _id,
        };
    }
    pub fn clone_polygons_arc(&self) -> Arc<RwLock<HashMap<String, Mutex<ConvexPolygon>>>> {
        return Arc::clone(&self.polygons);
    }
    pub fn insert_polygon(&self, polygon: ConvexPolygon) {
        let cloned = Arc::clone(&self.polygons);
        let mut write_mutex = cloned.write().expect("RwLock poisoned");
        write_mutex.insert(polygon.get_id(), Mutex::new(polygon));
        drop(write_mutex);
    }
    pub fn start_data_worker_thread(st: Arc<RwLock<DataStorage>>, millis: u64) {
        println!("start with millis {}", millis);

        let millis_asi64 = millis as i64;
        let mut write_mutex = st.write().expect("RwLock poisoned");
        write_mutex.period_start = Utc::now();
        drop(write_mutex);
        thread::sleep(STDDuration::from_millis(millis));

        // Next runs
        let read_mutex = st.read().expect("RwLock poisoned");
        let mut previous_tm = read_mutex.period_start;
        let cloned = Arc::clone(&read_mutex.polygons);
        drop(read_mutex);

        loop {
            let mut write_mutex = st.write().expect("RwLock poisoned");
            write_mutex.period_start = previous_tm;
            write_mutex.period_end = Some(write_mutex.period_start + Duration::milliseconds(millis_asi64));
            println!("\nPeriod start: {} | Period end: {}", write_mutex.period_start, write_mutex.period_end.unwrap());
            previous_tm = write_mutex.period_end.unwrap();
            let write_mutex_polygons = cloned.write().expect("RwLock poisoned");
            for (_, v) in write_mutex_polygons.iter() {
                let mut element = v.lock().expect("Mutex poisoned");
                // Summary
                element.period_start = write_mutex.period_start;
                element.period_end = write_mutex.period_end;
                element.estimated_avg_speed = element.avg_speed;
                element.estimated_sum_intensity = element.sum_intensity;
                element.avg_speed = -1.0;
                element.sum_intensity = 0;
                println!("\tPolygon: {} | Intensity: {} | Speed: {}", element.get_id(), element.estimated_sum_intensity, element.estimated_avg_speed);
                // Certain vehicle type
                for (vehicle_type, statistics) in element.statistics.iter_mut() {
                    statistics.estimated_avg_speed = statistics.avg_speed;
                    statistics.estimated_sum_intensity = statistics.sum_intensity;
                    statistics.avg_speed = -1.0;
                    statistics.sum_intensity = 0;
                    println!("\t\tVehicle type: {} | Intensity: {} | Speed: {}", vehicle_type, statistics.estimated_sum_intensity, statistics.estimated_avg_speed);
                }
                drop(element);
            }
            drop(write_mutex_polygons);
            drop(write_mutex);
            thread::sleep(STDDuration::from_millis(millis));
        }
    }
}