use actix_web::{HttpResponse, web, Responder};

use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use crate::lib::data_storage::DataStorage;
use crate::lib::geojson::PolygonsGeoJSON;

async fn say_ping() -> impl Responder {
    HttpResponse::Ok().body("pong")
}

pub fn polygons_list(data: web::Data<Arc<RwLock<DataStorage>>>) -> HttpResponse {
    let data_storage = data.get_ref().clone();
    let data_expected = data_storage.read().expect("expect: polygons_list");
    let data_expected_polygons = data_expected.polygons.read().expect("expect: polygons_list");
    let mut ans = PolygonsGeoJSON::new();
    for (_, v) in data_expected_polygons.iter() {
        let element = v.lock().expect("Mutex poisoned");
        let geo_feature = element.to_geojson();
        drop(element);
        ans.features.push(geo_feature);
    }
    return HttpResponse::Ok().json(ans);
}

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Deserialize, Serialize)]
pub struct AllPolygonsStats {
    pub equipment_id: String,
    pub data: Vec<PolygonStats>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonStats {
    pub lane_number: u16,
    pub lane_direction: u8,
    pub period_start: DateTime<Utc>,
    pub period_end: Option<DateTime<Utc>>,
    pub statistics: HashMap<String, VehicleTypeParameters>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VehicleTypeParameters {
    pub estimated_avg_speed: f32,
    pub estimated_sum_intensity: u32
}

pub fn all_polygons_stats(data: web::Data<Arc<RwLock<DataStorage>>>) -> HttpResponse {
    let data_storage = data.get_ref().clone();
    let data_expected = data_storage.read().expect("expect: all_polygons_stats");
    let data_expected_polygons = data_expected.polygons.read().expect("expect: all_polygons_stats");
    let mut ans = AllPolygonsStats{
        equipment_id: data_expected.id.clone(),
        data: vec![]
    };
    for (_, v) in data_expected_polygons.iter() {
        let element = v.lock().expect("Mutex poisoned");
        let mut stats = PolygonStats{
            lane_number: element.road_lane_num,
            lane_direction: element.road_lane_direction,
            period_start: element.period_start,
            period_end: element.period_end,
            statistics: HashMap::new()
        };
        for (vehicle_type, statistics) in element.statistics.iter() {
            stats.statistics.insert(vehicle_type.to_string(), VehicleTypeParameters{
                estimated_avg_speed: statistics.estimated_avg_speed,
                estimated_sum_intensity: statistics.estimated_sum_intensity
            });
        }
        drop(element);
        ans.data.push(stats);
    }
    drop(data_expected_polygons);
    drop(data_expected);
    return HttpResponse::Ok().json(ans);
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::scope("/api")
            .route("/ping", web::get().to(say_ping))
            .service(
                web::scope("/polygons")
                .route("/geojson", web::get().to(polygons_list))
            )
            .service(
                web::scope("/stats")
                .route("/all", web::get().to(all_polygons_stats))
                // .route("/by_polygon_id/{polygon_id}", web::get().to(/*todo*/))
            )
        );
}
