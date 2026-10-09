use std::collections::HashMap;

use crate::lib::data_storage::ThreadedDataStorage;
use crate::lib::publisher::redis_transport::RedisPublisher;
use crate::rest_api::zones_stats::{
    AllZonesStats, TrafficFlowInfo, VehicleTypeParameters, ZoneStats,
};
use crate::settings::RedisPublisherSettings;

pub struct RedisPublishers {
    pub statistics: Option<StatisticsPublisher>,
    pub vehicle_events: Option<RedisPublisher>,
}

impl RedisPublishers {
    pub fn new(settings: &RedisPublisherSettings, data_storage: ThreadedDataStorage) -> Self {
        let events = &settings.vehicle_events;
        let common = if settings.enable || (events.enable && events.connection.is_none()) {
            RedisPublisher::new(&settings.connection())
        } else {
            None
        };
        let statistics = if settings.enable {
            common.as_ref().map(|publisher| StatisticsPublisher {
                publisher: publisher.with_channel(if settings.channel_name.is_empty() {
                    "DETECTORS_STATISTICS"
                } else {
                    &settings.channel_name
                }),
                data_storage,
            })
        } else {
            None
        };
        let vehicle_events = if events.enable {
            match &events.connection {
                Some(connection) => RedisPublisher::new(connection),
                None => common,
            }
            .map(|publisher| publisher.with_channel(&events.channel_name))
        } else {
            None
        };
        Self {
            statistics,
            vehicle_events,
        }
    }
}

pub struct StatisticsPublisher {
    publisher: RedisPublisher,
    data_storage: ThreadedDataStorage,
}

impl StatisticsPublisher {
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
            od_matrix: HashMap::new(),
        };
        for (_, v) in zones.iter() {
            let element = v.lock().expect("Mutex poisoned");
            let mut stats = ZoneStats {
                id: element.get_id().to_string(),
                lane_number: element.road_lane_num,
                lane_direction: element.road_lane_direction,
                period_start: element.statistics.period_start,
                period_end: element.statistics.period_end,
                statistics: HashMap::new(),
                traffic_flow_parameters: TrafficFlowInfo {
                    avg_speed: element.statistics.traffic_flow_parameters.avg_speed,
                    sum_intensity: element.statistics.traffic_flow_parameters.sum_intensity,
                    defined_sum_intensity: element
                        .statistics
                        .traffic_flow_parameters
                        .defined_sum_intensity,
                    avg_headway: element.statistics.traffic_flow_parameters.avg_headway,
                },
            };
            for (vehicle_type, statistics) in element.statistics.vehicles_data.iter() {
                stats.statistics.insert(
                    vehicle_type.to_string(),
                    VehicleTypeParameters {
                        estimated_avg_speed: statistics.avg_speed,
                        estimated_sum_intensity: statistics.sum_intensity,
                        estimated_defined_sum_intensity: statistics.defined_sum_intensity,
                    },
                );
            }
            drop(element);
            prepared_message.data.push(stats);
        }
        drop(zones);
        drop(ds_guard);
        self.publisher
            .publish(move || serde_json::to_string(&prepared_message));
    }
}
