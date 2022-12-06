
extern crate redis;

use redis::{ Client, Commands };
use std::sync::{ Arc, RwLock };
use std::error::Error;
use std::collections::HashMap;
use crate::publisher::RedisMessage;
use crate::lib::data_storage::DataStorage;
use crate::lib::rest_api::{ AllPolygonsStats, PolygonStats, VehicleTypeParameters };
use std::thread;
use std::time::Duration as STDDuration;

pub struct RedisConnection {
    pub channel_name: String,
    pub client: Arc<Client>
}

impl RedisConnection {
    pub fn new(host: String, port: i32, db_index: i32) -> RedisConnection {
        let client = Client::open(format!("redis://{}:{}/{}", host, port, db_index)).unwrap();
        return RedisConnection {
            channel_name: "DETECTORS_STATISTICS".to_string(),
            client: Arc::new(client),
        }
    }
    pub fn new_with_password(host: String, port: i32, db_index: i32, password: String) -> RedisConnection {
        let client = Client::open(format!("redis://:{}@{}:{}/{}", password, host, port, db_index)).unwrap();
        return RedisConnection {
            channel_name: "DETECTORS_STATISTICS".to_string(),
            client: Arc::new(client),
        }
    }
    pub fn set_channel(&mut self, _channel_name: String) {
        self.channel_name = _channel_name.clone();
    }
    pub fn publish(&self, msg: &dyn RedisMessage) -> Result<(), Box<dyn Error>> {
        println!("Trying to send data...");
        let mut redis_conn = match self.client.get_connection() {
            Ok(_conn) => {
                _conn
            }
            Err(_err) => {
                return Err(_err.into());
            }
        };
        let msg_string = msg.prepare_string()?;
        redis_conn.publish(self.channel_name.to_owned(), msg_string)?;
        println!("\t...Success");
        Ok(())
    }
    pub fn start_worker(&self, data: Arc<RwLock<DataStorage>>, millis: u64) {
        thread::sleep(STDDuration::from_millis(millis));
        loop {
            let data_expected = data.read().expect("expect: all_polygons_stats");
            let data_expected_polygons = data_expected.polygons.read().expect("expect: all_polygons_stats");
            let mut prepared_message = AllPolygonsStats {
                equipment_id: data_expected.id.clone(),
                data: vec![]
            };
            for (_, v) in data_expected_polygons.iter() {
                let element = v.lock().expect("Mutex poisoned");
                let mut stats = PolygonStats {
                    lane_number: element.road_lane_num,
                    lane_direction: element.road_lane_direction,
                    period_start: element.period_start,
                    period_end: element.period_end,
                    statistics: HashMap::new()
                };
                for (vehicle_type, statistics) in element.statistics.iter() {
                    stats.statistics.insert(vehicle_type.to_string(), VehicleTypeParameters {
                        estimated_avg_speed: statistics.estimated_avg_speed,
                        estimated_sum_intensity: statistics.estimated_sum_intensity
                    });
                }
                drop(element);
                prepared_message.data.push(stats);
            }
            drop(data_expected_polygons);
            drop(data_expected);

            match self.publish(&prepared_message) {
                Err(_err) => {
                    println!("Errors while sending data to Redis: {}",_err);
                }
                Ok(_) => {}
            };
            thread::sleep(STDDuration::from_millis(millis));
        }
    }
}

impl RedisMessage for AllPolygonsStats {
    fn prepare_string(&self) -> Result<String, Box<dyn Error>> {
        let json = serde_json::to_string(self)?;
        Ok(json)
    }
}