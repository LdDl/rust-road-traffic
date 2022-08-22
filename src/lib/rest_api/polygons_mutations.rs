use actix_web::{HttpResponse, web, Error, http::StatusCode};
use serde::{
    Deserialize,
    Serialize
};
use crate::lib::polygons::ConvexPolygon;
use crate::lib::rest_api::Storage;

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    pub error_text: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonUpdateRequest {
    pub polygon_id: String,
    pub pixel_points: Option<[[u16; 2]; 4]>,
    pub spatial_points: Option<[[f32; 2]; 4]>,
    pub lane_number: Option<u16>,
    pub lane_direction: Option<u8>,
    pub color_rgb: Option<[i16; 3]>
}

#[derive(Debug, Serialize)]
pub struct PolygonUpdateResponse <'a>{
    pub message: &'a str,
}

//
// curl -XPOST 'http://localhost:42001/api/mutations/change_polygon' -d '{"polygon_id":"dir_0_lane_1", "lane_number": 939, "pixel_points": [[299, 222], [572, 265], [547, 66], [359, 69]], "color_rgb": [130, 0, 100]}' -H 'Content-Type: application/json'
//
pub async fn change_polygon(data: web::Data<Storage>, update_polygon: web::Json<PolygonUpdateRequest>) -> Result<HttpResponse, Error> {

    let data_storage = data.data_storage.as_ref().clone();
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

    // @todo need to deal with those (see main function):
    // polygon.scale_geom(scale_x, scale_y);    
    // polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);

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

    match update_polygon.color_rgb {
        Some(val) => {
            let mut polygon = polygon_mutex.lock().expect("Mutex poisoned");
            polygon.set_color(val);
            drop(polygon);
        },
        _ => {}
    }

    return Ok(HttpResponse::Ok().json(PolygonUpdateResponse{
        message: "ok"
    }));
}


#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonDeleteRequest {
    pub polygon_id: String,
}

#[derive(Debug, Serialize)]
pub struct PolygonDeleteResponse <'a>{
    pub message: &'a str,
}

//
// curl -XPOST 'http://localhost:42001/api/mutations/delete_polygon' -d '{"polygon_id":"dir_0_lane_1"}' -H 'Content-Type: application/json'
//
pub async fn delete_polygon(data: web::Data<Storage>, delete_polygon: web::Json<PolygonDeleteRequest>) -> Result<HttpResponse, Error> {

    let data_storage = data.data_storage.as_ref().clone();
    let data_expected = data_storage.read().expect("expect: polygons_list");
    let mut data_expected_polygons = data_expected.polygons.write().expect("expect: polygons_list");

    match data_expected_polygons.remove(&delete_polygon.polygon_id) {
        /* Check if polygon with such identifier exists */
        Some(_) => {},
        None => {
            return Ok(HttpResponse::build(StatusCode::FAILED_DEPENDENCY).json(ErrorResponse {
                error_text: format!("No such polygon. Requested ID: {}", delete_polygon.polygon_id)
            }));
        }
    };

    return Ok(HttpResponse::Ok().json(PolygonDeleteResponse{
        message: "ok"
    }));
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PolygonCreateRequest {
    pub pixel_points: Option<[[u16; 2]; 4]>,
    pub spatial_points: Option<[[f32; 2]; 4]>,
    pub lane_number: Option<u16>,
    pub lane_direction: Option<u8>,
    pub color_rgb: Option<[i16; 3]>
}

#[derive(Debug, Serialize)]
pub struct PolygonCreateResponse {
    pub polygon_id: String
}

//
// curl -XPOST 'http://localhost:42001/api/mutations/create_polygon' -d '{"lane_number": 939, "lane_direction": 33, "pixel_points": [[230, 200], [550, 235], [512, 40], [359, 69]], "spatial_points": [[37.618908137083054, 54.20564619851147], [37.61891517788172, 54.20564502193819], [37.618927247822285, 54.205668749493036], [37.61892020702362, 54.2056701221611]], "color_rgb": [130, 130, 0]}' -H 'Content-Type: application/json'
//
pub async fn create_polygon(data: web::Data<Storage>, new_polygon: web::Json<PolygonCreateRequest>) -> Result<HttpResponse, Error> {

    // @todo need to deal with those (see main function):
    // polygon.scale_geom(scale_x, scale_y);    
    // polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);

    let mut polygon = ConvexPolygon::empty();
    match new_polygon.pixel_points {
        Some(data) => {
            polygon.update_pixel_map_arr(data);
        },
        _ => {}
    }

    match new_polygon.spatial_points {
        Some(data) => {
            polygon.update_spatial_map_arr(data);
        },
        _ => {}
    }

    match new_polygon.lane_direction {
        Some(val) => {
            polygon.set_road_lane_direction(val);
        },
        _ => {}
    }

    match new_polygon.lane_number {
        Some(val) => {
            polygon.set_road_lane_num(val);
        },
        _ => {}
    }

    match new_polygon.color_rgb {
        Some(val) => {
            polygon.set_color(val);
        },
        _ => {}
    }

    let new_id = polygon.get_id().clone();

    let data_storage = data.data_storage.as_ref().clone();
    let data_expected = data_storage.read().expect("expect: polygons_list");
    data_expected.insert_polygon(polygon);

    return Ok(HttpResponse::Ok().json(PolygonCreateResponse{
        polygon_id: new_id
    }));
}