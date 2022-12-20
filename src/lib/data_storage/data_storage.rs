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
use std::error::Error;

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
    pub verbose: bool
}

impl DataStorage {
    pub fn new_with_id(_id: String, _verbose: bool) -> Self {
        let now = Utc::now();
        return DataStorage {
            polygons: Arc::new(RwLock::new(HashMap::<String, Mutex<ConvexPolygon>>::new())),
            period_start: now,
            period_end: None,
            id: _id,
            verbose: _verbose
        };
    }
    pub fn new_arc_with_id(_id: String, _verbose: bool) -> Arc<RwLock<Self>> {
        let now = Utc::now();
        return Arc::new(RwLock::new(DataStorage {
            polygons: Arc::new(RwLock::new(HashMap::<String, Mutex<ConvexPolygon>>::new())),
            period_start: now,
            period_end: None,
            id: _id,
            verbose: _verbose
        }));
    }
    pub fn clone_polygons_arc(&self) -> Arc<RwLock<HashMap<String, Mutex<ConvexPolygon>>>> {
        return Arc::clone(&self.polygons);
    }
    pub fn insert_polygon(&self, polygon: ConvexPolygon) {
        let polygons = Arc::clone(&self.polygons);
        match polygons.write() {
            Ok(mut mutex) => {
                mutex.insert(polygon.get_id(), Mutex::new(polygon));
            },
            Err(err) => {
                println!("Can't insert polygon due PoisonErr: {}", err)
            }
        };
    }
    pub fn get_arc_copy(self) -> Arc<RwLock<Self>> {
        return Arc::new(RwLock::new(self));
    }
    pub fn start_data_worker_thread(self, millis: u64) {
        let verbose = self.verbose;
        let this = Arc::new(RwLock::new(self));
        thread::spawn(move || {
            DataStorage::start_data_worker(this, 2000, verbose);
        });
    }
    pub fn start_data_worker(this: Arc<RwLock<DataStorage>>, millis: u64, verbose: bool) {
        if verbose {
            println!("Polygons data would be refreshed every {} ms", millis);
        }
        let millis_asi64 = millis as i64;
        let mut write_mutex = this.write().expect("RwLock poisoned");
        write_mutex.period_start = Utc::now();
        drop(write_mutex);
        thread::sleep(STDDuration::from_millis(millis));

        // Next runs
        let read_mutex = this.read().expect("RwLock poisoned");
        let mut previous_tm = read_mutex.period_start;
        let cloned = Arc::clone(&read_mutex.polygons);
        drop(read_mutex);

        loop {
            let mut write_mutex = this.write().expect("RwLock poisoned");
            previous_tm= write_mutex.update_periods(previous_tm, millis_asi64);
            if verbose {
                println!("\nPeriod start: {} | Period end: {}", write_mutex.period_start, previous_tm);
            }
            write_mutex.update_polygons_stats();
            drop(write_mutex);
            thread::sleep(STDDuration::from_millis(millis));
        }
    }
    pub fn update_periods(&mut self, last_tm: DateTime<Utc>, add_millis: i64) -> DateTime<Utc>{
        self.period_start = last_tm;
        self.period_end = Some(self.period_start + Duration::milliseconds(add_millis));
        return self.period_end.unwrap();
    }
    pub fn update_polygons_stats(&mut self) {
        let mutext = Arc::clone(&self.polygons);
        match mutext.write() {
            Ok(mut polygons) => {
                for (_, v) in polygons.iter_mut() {
                    let mut element = v.lock().expect("Mutex poisoned");
                    // Summary
                    element.period_start = self.period_start;
                    element.period_end = self.period_end;
                    element.estimated_avg_speed = element.avg_speed;
                    element.estimated_sum_intensity = element.sum_intensity;
                    element.avg_speed = -1.0;
                    element.sum_intensity = 0;
                    if self.verbose {
                        println!("\tPolygon: {} | Intensity: {} | Speed: {}", element.get_id(), element.estimated_sum_intensity, element.estimated_avg_speed);
                    }
                    // Certain vehicle type
                    for (vehicle_type, statistics) in element.statistics.iter_mut() {
                        statistics.estimated_avg_speed = statistics.avg_speed;
                        statistics.estimated_sum_intensity = statistics.sum_intensity;
                        statistics.avg_speed = -1.0;
                        statistics.sum_intensity = 0;
                        if self.verbose {
                            println!("\t\tVehicle type: {} | Intensity: {} | Speed: {}", vehicle_type, statistics.estimated_sum_intensity, statistics.estimated_avg_speed);
                        }
                    }
                    drop(element);
                }
            },
            Err(err) => {
                println!("Can't update polygon statistics due PoisonErr: {}", err)
            }
        };
    }
}
