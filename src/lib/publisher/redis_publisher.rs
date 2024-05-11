extern crate redis;

use crate::lib::data_storage::ThreadedDataStorage;
use crate::lib::publisher::RedisMessage;
use crate::rest_api::zones_stats::{AllZonesStats, VehicleTypeParameters, ZoneStats};
use redis::{Client, Commands};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

pub struct RedisConnection {
    pub channel_name: String,
    pub client: Arc<Client>,
    pub data_storage: ThreadedDataStorage,
}

impl RedisConnection {
    pub fn new(
        host: String,
        port: i32,
        db_index: i32,
        data_storage: ThreadedDataStorage,
    ) -> RedisConnection {
        let client = Client::open(format!("redis://{}:{}/{}", host, port, db_index)).unwrap();
        return RedisConnection {
            channel_name: "DETECTORS_STATISTICS".to_string(),
            client: Arc::new(client),
            data_storage,
        };
    }
    pub fn new_with_password(
        host: String,
        port: i32,
        db_index: i32,
        password: String,
        data_storage: ThreadedDataStorage,
    ) -> RedisConnection {
        let client = Client::open(format!(
            "redis://:{}@{}:{}/{}",
            password, host, port, db_index
        ))
        .unwrap();
        return RedisConnection {
            channel_name: "DETECTORS_STATISTICS".to_string(),
            client: Arc::new(client),
            data_storage,
        };
    }
    pub fn set_channel(&mut self, _channel_name: String) {
        self.channel_name = _channel_name.clone();
    }
    pub fn publish(&self, msg: &dyn RedisMessage) -> Result<(), Box<dyn Error>> {
        println!("Trying to send data...");
        let mut redis_conn = match self.client.get_connection() {
            Ok(_conn) => _conn,
            Err(_err) => {
                return Err(_err.into());
            }
        };
        let msg_string = msg.prepare_string()?;
        redis_conn.publish(self.channel_name.to_owned(), msg_string)?;
        println!("...Success");
        Ok(())
    }
    pub fn push_statistics(&self) {
        let ds_guard = self
            .data_storage
            .read()
            .expect("DataStorage is poisoned [RWLock]");
        let zones = ds_guard
            .zones
            .read()
            .expect("Spatial data is poisoned [RWLock]");
        let mut prepared_message = AllZonesStats {
            equipment_id: ds_guard.id.clone(),
            data: vec![],
        };
        for (_, v) in zones.iter() {
            let element = v.lock().expect("Mutex poisoned");
            let mut stats = ZoneStats {
                lane_number: element.road_lane_num,
                lane_direction: element.road_lane_direction,
                period_start: element.statistics.period_start,
                period_end: element.statistics.period_end,
                statistics: HashMap::new(),
            };
            for (vehicle_type, statistics) in element.statistics.vehicles_data.iter() {
                stats.statistics.insert(
                    vehicle_type.to_string(),
                    VehicleTypeParameters {
                        estimated_avg_speed: statistics.avg_speed,
                        estimated_sum_intensity: statistics.sum_intensity,
                        estimated_avg_headway: statistics.avg_headway,
                    },
                );
            }
            drop(element);
            prepared_message.data.push(stats);
        }
        drop(zones);
        drop(ds_guard);
        match self.publish(&prepared_message) {
            Err(_err) => {
                println!("Errors while sending data to Redis: {}", _err);
            }
            Ok(_) => {}
        };
    }
}

impl RedisMessage for AllZonesStats {
    fn prepare_string(&self) -> Result<String, Box<dyn Error>> {
        let json = serde_json::to_string(self)?;
        Ok(json)
    }
}
