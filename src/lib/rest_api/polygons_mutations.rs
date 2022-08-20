use actix_web::{HttpResponse, web, Error, http::StatusCode};
use serde::{
    Deserialize,
    Serialize
};
use std::sync::{Arc, RwLock};
use crate::lib::data_storage::DataStorage;

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonUpdateRequest {
    pub polygon_id: String,
    pub pixel_points: Option<[[u16; 2]; 4]>,
    pub spatial_points: Option<[[f32; 2]; 4]>,
    pub lane_number: Option<u16>,
    pub lane_direction: Option<u8>,
}

#[derive(Debug, Serialize)]
pub struct PolygonUpdateResponse <'a>{
    pub message: &'a str,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    pub error_text: String,
}

pub async fn change_polygon(data: web::Data<Arc<RwLock<DataStorage>>>, update_polygon: web::Json<PolygonUpdateRequest>) -> Result<HttpResponse, Error> {

    let data_storage = data.get_ref().clone();
    let data_expected = data_storage.read().expect("expect: polygons_list");
    let mut data_expected_polygons = data_expected.polygons.write().expect("expect: polygons_list");

    let polygon_mutex = match data_expected_polygons.get_mut(&update_polygon.polygon_id) {
        /* Check if polygon with such identifier exists */
        Some(val) => val,
        None => {
            return Ok(HttpResponse::build(StatusCode::FAILED_DEPENDENCY).json(ErrorResponse {
                error_text: format!("No such polygon. Requested ID: {}", update_polygon.polygon_id)
            }));
        }
    };
    match update_polygon.pixel_points {
        Some(data) => {
            let mut polygon = polygon_mutex.lock().expect("Mutex poisoned");
            polygon.update_pixel_map_arr(data);
            drop(polygon);
        },
        _ => {}
    }

    match update_polygon.spatial_points {
        Some(data) => {
            let mut polygon = polygon_mutex.lock().expect("Mutex poisoned");
            polygon.update_spatial_map_arr(data);
            drop(polygon);
        },
        _ => {}
    }

    match update_polygon.lane_direction {
        Some(val) => {
            let mut polygon = polygon_mutex.lock().expect("Mutex poisoned");
            polygon.set_road_lane_direction(val);
            drop(polygon);
        },
        _ => {}
    }

    match update_polygon.lane_number {
        Some(val) => {
            let mut polygon = polygon_mutex.lock().expect("Mutex poisoned");
            polygon.set_road_lane_num(val);
            drop(polygon);
        },
        _ => {}
    }

    return Ok(HttpResponse::Ok().json(PolygonUpdateResponse{
        message: "ok"
    }));
}
