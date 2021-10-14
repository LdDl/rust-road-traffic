use actix_web::{HttpResponse, web, Responder};

use std::sync::{Arc, Mutex, RwLock};
use std::collections::HashMap;
use crate::lib::polygons::ConvexPolygon;
use crate::lib::polygons::PolygonID;
use crate::lib::polygons::PolygonsGeoJSON;

async fn say_ping() -> impl Responder {
    HttpResponse::Ok().body("pong")
}

pub fn polygons_list(data: web::Data<Arc<RwLock<HashMap<PolygonID, Mutex<ConvexPolygon>>>>>) -> HttpResponse {
    let data_storage = data.get_ref().clone();
    let data_expected = data_storage.read().expect("expect: polygons_list");
    let mut ans = PolygonsGeoJSON::new();
    for (_, v) in data_expected.iter() {
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
    pub data: Vec<PolygonStats>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonStats {
    pub period_start: DateTime<Utc>,
    pub period_end: Option<DateTime<Utc>>,
    pub statistics: HashMap<String, VehicleTypeParameters>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VehicleTypeParameters {
    pub estimated_avg_speed: f32,
    pub estimated_sum_intensity: u32
}

pub fn all_polygons_stats(data: web::Data<Arc<RwLock<HashMap<PolygonID, Mutex<ConvexPolygon>>>>>) -> HttpResponse {
    let data_storage = data.get_ref().clone();
    let data_expected = data_storage.read().expect("expect: all_polygons_stats");
    let mut ans = AllPolygonsStats{
        data: vec![]
    };
    for (_, v) in data_expected.iter() {
        let element = v.lock().expect("Mutex poisoned");
        let mut stats = PolygonStats{
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
