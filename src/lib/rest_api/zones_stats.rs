use actix_web::{HttpResponse, web, Error};
use serde::Serialize;
use utoipa::ToSchema;
use chrono::{DateTime, Utc};

use std::collections::HashMap;
use crate::lib::rest_api::APIStorage;

#[derive(Debug, Serialize, ToSchema)]
pub struct AllZonesStats {
    pub equipment_id: String,
    pub data: Vec<ZoneStats>
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneStats {
    pub lane_number: u16,
    pub lane_direction: u8,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub statistics: HashMap<String, VehicleTypeParameters>
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VehicleTypeParameters {
    pub estimated_avg_speed: f32,
    pub estimated_sum_intensity: u32
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
    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard.zones.read().expect("Spatial data is poisoned [RWLock]");
    let mut ans: AllZonesStats = AllZonesStats{
        equipment_id: ds_guard.id.clone(),
        data: vec![]
    };
    for (_, zone_guarded) in zones.iter() {
        let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
        let mut stats = ZoneStats{
            lane_number: zone.road_lane_num,
            lane_direction: zone.road_lane_direction,
            period_start: zone.statistics.period_start,
            period_end: zone.statistics.period_end,
            statistics: HashMap::new()
        };
        for (vehicle_type, statistics) in zone.statistics.vehicles_data.iter() {
            stats.statistics.insert(vehicle_type.to_string(), VehicleTypeParameters{
                estimated_avg_speed: statistics.avg_speed,
                estimated_sum_intensity: statistics.sum_intensity
            });
        }
        ans.data.push(stats);
    }
    drop(zones);
    drop(ds_guard);
    return Ok(HttpResponse::Ok().json(ans));
}


/// Information about road traffic parameters in real-time for each detection zone
#[derive(Debug, Serialize, ToSchema)]
pub struct AllZonesRealtimeStatistics {
    /// Equipment identifier. Should match software configuration
    #[schema(example = "1e23985f-1fa3-45d0-a365-2d8525a23ddd")]
    pub equipment_id: String,
    /// Set of detection zones and its realtime statistics
    pub data: Vec<ZoneRealtime>
}

/// Information about realtime statistics for the specific detection zone
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
    /// Occupancy
    #[schema(example = 3)]
    pub occupancy: u16
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
    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard.zones.read().expect("Spatial data is poisoned [RWLock]");
    let mut ans: AllZonesRealtimeStatistics = AllZonesRealtimeStatistics{
        equipment_id: ds_guard.id.clone(),
        data: vec![]
    };
    for (_, zone_guarded) in zones.iter() {
        let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
        let stats = ZoneRealtime{
            lane_number: zone.road_lane_num,
            lane_direction: zone.road_lane_direction,
            last_time: zone.current_statistics.last_time,
            occupancy: zone.current_statistics.occupancy
        };
        ans.data.push(stats);
    }
    drop(zones);
    drop(ds_guard);
    return Ok(HttpResponse::Ok().json(ans));
}