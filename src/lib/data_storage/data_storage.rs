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
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub id: String,
    pub verbose: bool
}

impl DataStorage {
    pub fn new_with_id(_id: String, _verbose: bool) -> Self {
        return DataStorage {
            zones: Arc::new(RwLock::new(HashMap::<String, Mutex<Zone>>::new())),
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
