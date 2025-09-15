use actix_web::{web, Error, HttpResponse};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::rest_api::APIStorage;
use std::collections::HashMap;

/// Information about aggregated road traffic flow parameters for the equipment
#[derive(Debug, Serialize, ToSchema)]
pub struct AllZonesStats {
    /// Equipment identifier. Should match software configuration
    #[schema(example = "1e23985f-1fa3-45d0-a365-2d8525a23ddd")]
    pub equipment_id: String,
    /// Set of data with summary information about road traffic parameters for each detection zone
    pub data: Vec<ZoneStats>,
    /// Origin-Destination matrix.
    /// Key: "ld-{lane_direction}_ln-{lane_number}" (e.g. "ld-1_ln-2")
    /// Value: HashMap where key is the same as for the main HashMap and value is number of vehicles that moved from
    pub od_matrix: HashMap<String, HashMap<String, u32>>,
}

/// Summary information for each detection zone
#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneStats {
    /// Corresponding road lane number
    #[schema(example = 2)]
    pub lane_number: u16,
    /// Corresponding road lane direction
    #[schema(example = 1)]
    pub lane_direction: u8,
    /// Start time for the statistics aggeration
    #[schema(value_type = String, example = "2023-01-02T15:00:00Z")]
    pub period_start: DateTime<Utc>,
    /// End time for the statistics aggeration
    #[schema(value_type = String, example = "2023-01-02T15:05:00Z")]
    pub period_end: DateTime<Utc>,
    /// Statistic for every vehicle type. Key: vehicle type; Value - road traffic flow parameters
    #[schema(example = json!({"train":{"estimated_avg_speed":-1,"estimated_sum_intensity":0},"bus":{"estimated_avg_speed":15.2,"estimated_sum_intensity":2},"truck":{"estimated_avg_speed":20.965343,"estimated_sum_intensity":3},"car":{"estimated_avg_speed":23.004976,"estimated_sum_intensity":4},"motorbike":{"estimated_avg_speed":-1,"estimated_sum_intensity":0}  }))]
    pub statistics: HashMap<String, VehicleTypeParameters>,
    /// Aggregated traffic flow parameters across the all vehicle types
    pub traffic_flow_parameters: TrafficFlowInfo
}

/// Road traffic parameters for specific vehicle type
#[derive(Debug, Serialize, ToSchema)]
pub struct VehicleTypeParameters {
    /// Average speed of road traffic flow. Value "-1" indicates not vehicles detected at all.
    #[schema(example = 32.1)]
    pub estimated_avg_speed: f32,
    /// Summary road traffic flow (if it is needed could be extrapolated to the intensity: vehicles/hour)
    #[schema(example = 19)]
    pub estimated_sum_intensity: u32,
    // The main difference between defined_sum_intensity and sum_intensity is in that fact
    // that sum_intensity does not take into account whether vehicles have estimated speed, when
    // defined_sum_intensity does. Could be less or equal to sum_intensity.
    #[schema(example = 12)]
    pub estimated_defined_sum_intensity: u32
}

/// Road traffic parameters for specific vehicle type
#[derive(Debug, Serialize, ToSchema)]
pub struct TrafficFlowInfo {
    /// Average speed of road traffic flow. Value "-1" indicates not vehicles detected at all.
    #[schema(example = 32.1)]
    pub avg_speed: f32,
    /// Total number of vehicles that passed throught the zone
    #[schema(example = 15)]
    pub sum_intensity: u32,
    // The main difference between defined_sum_intensity and sum_intensity is in that fact
    // that sum_intensity does not take into account whether vehicles have estimated speed, when
    // defined_sum_intensity does. Could be less or equal to sum_intensity.
    #[schema(example = 13)]
    pub defined_sum_intensity: u32,
    /// Average headway. Headway - number of seconds between arrival of leading vehicle and following vehicle
    #[schema(example = 2.5)]
    pub avg_headway: f32,
}


#[utoipa::path(
    get,
    tag = "Statistics",
    path = "/api/stats/all",
    responses(
        (status = 200, description = "List of detections zones", body = AllZonesStats)
    )
)]
pub async fn all_zones_stats(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    let ds_guard = data
        .data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard
        .zones
        .read()
        .expect("Spatial data is poisoned [RWLock]");
    let mut ans: AllZonesStats = AllZonesStats {
        equipment_id: ds_guard.id.clone(),
        data: vec![],
        od_matrix: HashMap::new(),
    };

    // Collect all zone keys
    let mut zone_id_to_key = HashMap::new();
    let mut zone_keys = Vec::new();
    for (_, zone_guarded) in zones.iter() {
        let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
        let key = format!("ld-{}_ln-{}", zone.road_lane_direction, zone.road_lane_num);
        zone_id_to_key.insert(zone.get_id(), key.clone()); // zone.id -> "ld-1_ln-2"
        zone_keys.push(key);
        drop(zone);
    }

    // Populate OD matrix with default empty HashMaps
    for from_key in &zone_keys {
        let mut inner: HashMap<String, u32> = HashMap::new();
        for to_key in &zone_keys {
            inner.insert(to_key.clone(), 0);
        }
        ans.od_matrix.insert(from_key.clone(), inner);
    }

    for (_, zone_guarded) in zones.iter() {
        let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
        let mut stats = ZoneStats {
            lane_number: zone.road_lane_num,
            lane_direction: zone.road_lane_direction,
            period_start: zone.statistics.period_start,
            period_end: zone.statistics.period_end,
            statistics: HashMap::new(),
            traffic_flow_parameters: TrafficFlowInfo{
                avg_speed: zone.statistics.traffic_flow_parameters.avg_speed,
                sum_intensity: zone.statistics.traffic_flow_parameters.sum_intensity,
                defined_sum_intensity: zone.statistics.traffic_flow_parameters.defined_sum_intensity,
                avg_headway: zone.statistics.traffic_flow_parameters.avg_headway,
            }
        };
        for (vehicle_type, statistics) in zone.statistics.vehicles_data.iter() {
            stats.statistics.insert(
                vehicle_type.to_string(),
                VehicleTypeParameters {
                    estimated_avg_speed: statistics.avg_speed,
                    estimated_sum_intensity: statistics.sum_intensity,
                    estimated_defined_sum_intensity: statistics.defined_sum_intensity
                },
            );
        }
        ans.data.push(stats);
        // Populate OD matrix based on real-time income statistics
        let to_key = zone_id_to_key.get(&zone.get_id()).unwrap();
        for (from_zone_id, flow_count) in zone.current_statistics.income.iter() {
            // Convert from_zone_id (internal UUID) to OD matrix key format
            if let Some(from_key) = zone_id_to_key.get(from_zone_id) {
                // Update the OD matrix: from from_key TO to_key
                if let Some(from_matrix) = ans.od_matrix.get_mut(from_key) {
                    from_matrix.insert(to_key.clone(), *flow_count);
                }
            }
        }
        drop(zone);
    }
    drop(zones);
    drop(ds_guard);
    return Ok(HttpResponse::Ok().json(ans));
}

/// Information about occupancy in real-time for each detection zone
#[derive(Debug, Serialize, ToSchema)]
pub struct AllZonesRealtimeStatistics {
    /// Equipment identifier. Should match software configuration
    #[schema(example = "1e23985f-1fa3-45d0-a365-2d8525a23ddd")]
    pub equipment_id: String,
    /// Set of detection zones and its realtime occupancy information
    pub data: Vec<ZoneRealtime>,
}

/// Information about realtime occupancy for the specific detection zone
#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneRealtime {
    /// Corresponding road lane number
    #[schema(example = 2)]
    pub lane_number: u16,
    /// Corresponding road lane direction
    #[schema(example = 1)]
    pub lane_direction: u8,
    /// Last time occupancy calculated. Unix Timestamp (seconds)
    #[schema(example = 1693386819)]
    pub last_time: u64,
    /// Time spent since video has been started. It is relative to FPS
    #[schema(example = 12.9)]
    pub last_time_relative: f32,
    /// Last time (spent since software has been started) when vehicle has been registered is zone (via crossing virtual line
    /// or appearing in zone) calculated. Measure in seconds.
    #[schema(example = 10.2)]
    pub last_time_registered: f32,
    /// Occupancy
    #[schema(example = 3)]
    pub occupancy: u16,
}

#[utoipa::path(
    get,
    tag = "Statistics",
    path = "/api/realtime/occupancy",
    responses(
        (status = 200, description = "List of detections zones", body = AllZonesRealtimeStatistics)
    )
)]
pub async fn all_zones_occupancy(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    let ds_guard = data
        .data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard
        .zones
        .read()
        .expect("Spatial data is poisoned [RWLock]");
    let mut ans: AllZonesRealtimeStatistics = AllZonesRealtimeStatistics {
        equipment_id: ds_guard.id.clone(),
        data: vec![],
    };
    for (_, zone_guarded) in zones.iter() {
        let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
        let stats = ZoneRealtime {
            lane_number: zone.road_lane_num,
            lane_direction: zone.road_lane_direction,
            last_time: zone.current_statistics.last_time,
            last_time_relative: zone.current_statistics.last_time_relative,
            last_time_registered: zone.current_statistics.last_time_registered,
            occupancy: zone.current_statistics.occupancy,
        };
        ans.data.push(stats);
    }
    drop(zones);
    drop(ds_guard);
    return Ok(HttpResponse::Ok().json(ans));
}

