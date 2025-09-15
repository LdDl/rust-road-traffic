use std::collections::{
    HashMap
};

use std::sync::{
    Arc,
    Mutex,
    RwLock,
    PoisonError
};

use std::{
    thread
};

use chrono::{
    DateTime,
    TimeZone,
    Utc,
};

use crate::lib::zones::{
    Zone
};

#[derive(Debug)]
pub enum DataStorageError {
    Poison
}

impl<T> From<PoisonError<T>> for DataStorageError {
    fn from(err: PoisonError<T>) -> Self {
        Self::Poison
    }
}
impl std::fmt::Display for DataStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            DataStorageError::Poison => write!(f, "PoisonError")
        }
    }
}

#[derive(Clone)]
pub struct DataStorage {
    pub zones: Arc<RwLock<HashMap<String, Mutex<Zone>>>>,
    pub vehicle_last_zone_cross:  Arc<RwLock<HashMap<uuid::Uuid, String>>>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub id: String,
    pub verbose: bool
}

impl DataStorage {
    pub fn new_with_id(_id: String, _verbose: bool) -> Self {
        return DataStorage {
            zones: Arc::new(RwLock::new(HashMap::<String, Mutex<Zone>>::new())),
            vehicle_last_zone_cross: Arc::new(RwLock::new(HashMap::<uuid::Uuid, String>::new())),
            period_start: TimeZone::with_ymd_and_hms(&Utc, 1970, 1, 1, 0, 0, 0).unwrap(),
            period_end: TimeZone::with_ymd_and_hms(&Utc, 1970, 1, 1, 0, 0, 0).unwrap(),
            id: _id,
            verbose: _verbose
        };
    }
    pub fn insert_zone(&self, zone: Zone) -> Result<(), DataStorageError> {
        let zones = Arc::clone(&self.zones);
        match zones.write() {
            Ok(mut mutex) => {
                mutex.insert(zone.get_id(), Mutex::new(zone));
            },
            Err(_) => {
                return Err(DataStorageError::Poison);
            }
        };
        Ok(())
    }
    pub fn delete_zone(&self, zone_id: &String) -> Result<(), DataStorageError> {
        let zones = Arc::clone(&self.zones);
        match zones.write() {
            Ok(mut mutex) => {
                mutex.remove(zone_id);
            },
            Err(_) => {
                return Err(DataStorageError::Poison);
            }
        };
        Ok(())
    }
    pub fn update_statistics(&mut self) -> Result<(), DataStorageError> {
        let zones = Arc::clone(&self.zones);
        match zones.read() {
            Ok(mutex) => {
                for (_zone_id, zone) in mutex.iter() {
                    let mut zone = zone.lock()?;
                    zone.update_statistics(self.period_start, self.period_end);
                }
            },
            Err(_) => {
                return Err(DataStorageError::Poison);
            }
        };
        Ok(())
    }
    pub fn print_od_matrix(&self) -> Result<(), DataStorageError> {
        let zones = Arc::clone(&self.zones);
        match zones.read() {
            Ok(zones_guard) => {
                // Collect all zone keys and create mapping
                let mut zone_id_to_key = HashMap::new();
                let mut zone_keys = Vec::new();
                for (_, zone_guarded) in zones_guard.iter() {
                    let zone = zone_guarded.lock()?;
                    let key = format!("ld-{}_ln-{}", zone.road_lane_direction, zone.road_lane_num);
                    zone_id_to_key.insert(zone.get_id(), key.clone());
                    zone_keys.push(key);
                    drop(zone);
                }
                // Sort zone keys for consistent output
                zone_keys.sort();
                // Initialize OD matrix with zeros
                let mut od_matrix: HashMap<String, HashMap<String, u32>> = HashMap::new();
                for from_key in &zone_keys {
                    let mut inner: HashMap<String, u32> = HashMap::new();
                    for to_key in &zone_keys {
                        inner.insert(to_key.clone(), 0);
                    }
                    od_matrix.insert(from_key.clone(), inner);
                }
                // Populate OD matrix with actual data
                for (_, zone_guarded) in zones_guard.iter() {
                    let zone = zone_guarded.lock()?;
                    let to_key = zone_id_to_key.get(&zone.get_id()).unwrap();
                    
                    for (from_zone_id, flow_count) in zone.current_statistics.income.iter() {
                        // Convert from_zone_id (internal UUID) to OD matrix key format
                        if let Some(from_key) = zone_id_to_key.get(from_zone_id) {
                            // Update the OD matrix: from from_key TO to_key
                            if let Some(from_matrix) = od_matrix.get_mut(from_key) {
                                from_matrix.insert(to_key.clone(), *flow_count);
                            }
                        }
                    }
                    drop(zone);
                }
                // Print the OD matrix in a nice table format
                println!("\n=== Origin-Destination Matrix ===");
                println!("Equipment ID: {}", self.id);
                println!("Period: {} to {}", self.period_start, self.period_end);
                if zone_keys.is_empty() {
                    println!("No zones configured.");
                    return Ok(());
                }
                // Print header
                print!("{:>12}", "FROM \\ TO");
                for to_key in &zone_keys {
                    print!("{:>12}", to_key);
                }
                println!();
                // Print separator
                print!("{:>12}", "----------");
                for _ in &zone_keys {
                    print!("{:>12}", "----------");
                }
                println!();
                // Print matrix rows
                for from_key in &zone_keys {
                    print!("{:>12}", from_key);
                    if let Some(from_matrix) = od_matrix.get(from_key) {
                        for to_key in &zone_keys {
                            let count = from_matrix.get(to_key).unwrap_or(&0);
                            print!("{:>12}", count);
                        }
                    }
                    println!();
                }
                // Print summary statistics
                let total_movements: u32 = od_matrix.values()
                    .flat_map(|inner| inner.values())
                    .sum();
                println!("\n=== Summary ===");
                println!("Total movements: {}", total_movements);
                // Print top flows
                let mut flows: Vec<(String, String, u32)> = Vec::new();
                for (from_key, from_matrix) in &od_matrix {
                    for (to_key, count) in from_matrix {
                        if *count > 0 {
                            flows.push((from_key.clone(), to_key.clone(), *count));
                        }
                    }
                }
                if !flows.is_empty() {
                    flows.sort_by(|a, b| b.2.cmp(&a.2)); // Sort by count descending
                    println!("\nTop flows:");
                    for (from, to, count) in flows.iter().take(5) {
                        if from == to {
                            println!("  {} → {} (U-turns): {} vehicles", from, to, count);
                        } else {
                            println!("  {} → {}: {} vehicles", from, to, count);
                        }
                    }
                }
                println!("=== End OD Matrix ===\n");
            },
            Err(_) => {
                return Err(DataStorageError::Poison);
            }
        }
        Ok(())
    }
}

pub type ThreadedDataStorage = Arc<RwLock<DataStorage>>;

pub fn new_datastorage(_id: String, _verbose: bool) -> ThreadedDataStorage {
    let data_storage = DataStorage::new_with_id(_id, _verbose);
    Arc::new(RwLock::new(data_storage))
}

pub fn start_analytics_thread(ds: ThreadedDataStorage, millis: u64, verbose: bool) {
    if verbose {
        println!("Analytics data would be refreshed every {} ms", millis);
    }

    thread::spawn(move || {
        let millis_i64 = millis as i64;
        let mut last_tm = Utc::now();
        // Sleep to accumulate data for the first time
        thread::sleep(std::time::Duration::from_millis(millis));
        loop {
            match ds.write() {
                Ok(mut mutex) => {
                    mutex.period_start = last_tm;
                    mutex.period_end = last_tm + chrono::Duration::milliseconds(millis_i64);
                    match mutex.update_statistics() {
                        Ok(_) => {
                            println!("Statistics updated: {}", last_tm);
                        },
                        Err(_) => {
                            println!("Can't update statistics due PoisonErr [1]");
                        }
                    }
                    last_tm = Utc::now();
                },
                Err(_) => {
                    println!("Can't update statistics due PoisonErr [2]");
                }
            }
            thread::sleep(std::time::Duration::from_millis(millis));
        }
    });
}
// trait DataStorageTrait {
//     fn insert_zone(&self, polygon: Zone);
// }

// impl DataStorageTrait for ThreadedDataStorage {
//     fn insert_zone(&self, polygon: Zone) {
//         let mut write_mutex = self.write().expect("RwLock poisoned");
//         write_mutex.insert_zone(polygon);
//         drop(write_mutex);
//     }
// }

// pub struct ThreadedDataStorage(Arc<RwLock<DataStorage>>);

// trait DataStorageTrait {
//     fn insert_zone(&self, polygon: Zone);
// }

// impl DataStorageTrait for DataStorage {
//     fn insert_zone(&self, polygon: Zone) {
//         let polygons = Arc::clone(&self.polygons);
//         match polygons.write() {
//             Ok(mut mutex) => {
//                 mutex.insert(polygon.get_id(), Mutex::new(polygon));
//             },
//             Err(err) => {
//                 println!("Can't insert polygon due PoisonErr: {}", err)
//             }
//         };
//     }
// }

// #[derive(Clone)]
// pub struct ThreadedDataStorage<T: ?Sized>(Arc<RwLock<T>>);

// impl<T> ThreadedDataStorage<T> {
//     /// Create new `Data` instance.
//     pub fn new(_id: String, _verbose: bool) -> ThreadedDataStorage<DataStorage> {
//         let data_storage = DataStorage::new_with_id(_id, _verbose);
//         let r = RwLock::new(data_storage);
//         let ar = Arc::new(r);
//         ThreadedDataStorage(ar)
//     }
// }

// impl<T: DataStorageTrait + ?Sized> ThreadedDataStorage<T> {
//     /// Insert a polygon into the data storage.
//     pub fn insert_zone(&self, polygon: Zone) {
//         let mut write_mutex = self.0.write().expect("RwLock poisoned");
//         write_mutex.insert_zone(polygon);
//         drop(write_mutex);
//     }
// }
