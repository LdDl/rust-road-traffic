use actix_web::{HttpResponse, web, Error, http::StatusCode};
use serde::{
    Deserialize,
    Serialize
};
use crate::lib::zones::{
    Zone,
    VirtualLine
};
use crate::rest_api::APIStorage;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error_text: String,
}

#[derive(Debug, Deserialize)]
pub struct PolygonUpdateRequest {
    pub polygon_id: String,
    pub pixel_points: Option<[[u16; 2]; 4]>,
    pub spatial_points: Option<[[f32; 2]; 4]>,
    pub lane_number: Option<u16>,
    pub lane_direction: Option<u8>,
    pub color_rgb: Option<[i16; 3]>,
    pub virtual_line: Option<VirtualLineData>
}

#[derive(Debug, Serialize)]
pub struct PolygonUpdateResponse <'a>{
    pub message: &'a str,
}

//
// curl -XPOST 'http://localhost:42001/api/mutations/change_polygon' -d '{"polygon_id":"dir_0_lane_1", "lane_number": 939, "pixel_points": [[299, 222], [572, 265], [547, 66], [359, 69]], "color_rgb": [130, 0, 100]}' -H 'Content-Type: application/json'
//
pub async fn update_zone(data: web::Data<APIStorage>, _update_zone: web::Json<PolygonUpdateRequest>) -> Result<HttpResponse, Error> {

    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    let mut zones = ds_guard.zones.write().expect("Spatial data is poisoned [RWLock]");

    let zone_guarded = match zones.get_mut(&_update_zone.polygon_id) {
        /* Check if polygon with such identifier exists */
        Some(val) => val,
        None => {
            return Ok(HttpResponse::build(StatusCode::FAILED_DEPENDENCY).json(ErrorResponse {
                error_text: format!("No such zone. Requested ID: {}", _update_zone.polygon_id)
            }));
        }
    };

    // @todo need to deal with those (see main function):
    // polygon.scale_geom(scale_x, scale_y);    
    // polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);

    match _update_zone.pixel_points {
        Some(data) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.update_pixel_map(data);
            drop(zone)
        },
        _ => {}
    }

    match _update_zone.spatial_points {
        Some(data) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.update_spatial_map(data);
            drop(zone)
        },
        _ => {}
    }

    match _update_zone.lane_direction {
        Some(val) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.set_road_lane_direction(val);
            drop(zone)
        },
        _ => {}
    }

    match _update_zone.lane_number {
        Some(val) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.set_road_lane_num(val);
            drop(zone)
        },
        _ => {}
    }

    match _update_zone.color_rgb {
        Some(val) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.set_color(val);
            drop(zone)
        },
        _ => {}
    }

    match &_update_zone.virtual_line {
        Some(val) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            let mut new_line = VirtualLine::new_from(val.geometry, val.direction);
            new_line.set_color(val.color_rgb[2], val.color_rgb[1], val.color_rgb[0]);
            zone.set_virtual_line(new_line);
            drop(zone)
        },
        _ => {}
    }

    drop(zone_guarded);

    return Ok(HttpResponse::Ok().json(PolygonUpdateResponse{
        message: "ok"
    }));
}


#[derive(Debug, Deserialize)]
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
pub async fn delete_zone(data: web::Data<APIStorage>, _delete_zone: web::Json<PolygonDeleteRequest>) -> Result<HttpResponse, Error> {
    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    match ds_guard.delete_zone(&_delete_zone.polygon_id) {
        Ok(_) => {},
        Err(err) => {
            return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR).json(ErrorResponse {
                error_text: format!("Can't delete zone ID: {}. Error: {}", _delete_zone.polygon_id, err)
            }));
        }
    }
    drop(ds_guard);
    return Ok(HttpResponse::Ok().json(PolygonDeleteResponse{
        message: "ok"
    }));
}

#[derive(Debug, Deserialize)]
pub struct PolygonCreateRequest {
    pub pixel_points: Option<[[u16; 2]; 4]>,
    pub spatial_points: Option<[[f32; 2]; 4]>,
    pub lane_number: Option<u16>,
    pub lane_direction: Option<u8>,
    pub color_rgb: Option<[i16; 3]>,
    pub virtual_line: Option<VirtualLineData>
}

#[derive(Deserialize, Debug)]
pub struct VirtualLineData {
    pub geometry: [[i32; 2]; 2],
    pub color_rgb: [i16; 3],
    // 0 - left->right, top->bottom
    // 1 - right->left, bottom->top
    pub direction: u8,
}

#[derive(Debug, Serialize)]
pub struct PolygonCreateResponse {
    pub polygon_id: String
}

//
// curl -XPOST 'http://localhost:42001/api/mutations/create_polygon' -d '{"lane_number": 939, "lane_direction": 33, "pixel_points": [[230, 200], [550, 235], [512, 40], [359, 69]], "spatial_points": [[37.618908137083054, 54.20564619851147], [37.61891517788172, 54.20564502193819], [37.618927247822285, 54.205668749493036], [37.61892020702362, 54.2056701221611]], "color_rgb": [130, 130, 0]}' -H 'Content-Type: application/json'
//
pub async fn create_zone(data: web::Data<APIStorage>, _new_zone: web::Json<PolygonCreateRequest>) -> Result<HttpResponse, Error> {

    // @todo need to deal with those (see main function):
    // polygon.scale_geom(scale_x, scale_y);    
    // polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);

    let mut zone = Zone::default();
    match _new_zone.pixel_points {
        Some(data) => {
            zone.update_pixel_map(data);
        },
        _ => {}
    }

    match _new_zone.spatial_points {
        Some(data) => {
            zone.update_spatial_map(data);
        },
        _ => {}
    }

    match _new_zone.lane_direction {
        Some(val) => {
            zone.set_road_lane_direction(val);
        },
        _ => {}
    }

    match _new_zone.lane_number {
        Some(val) => {
            zone.set_road_lane_num(val);
        },
        _ => {}
    }

    match _new_zone.color_rgb {
        Some(val) => {
            zone.set_color(val);
        },
        _ => {}
    }

    match &_new_zone.virtual_line {
        Some(val) => {
            let mut new_line = VirtualLine::new_from(val.geometry, val.direction);
            new_line.set_color(val.color_rgb[2], val.color_rgb[1], val.color_rgb[0]);
            zone.set_virtual_line(new_line);
        },
        _ => {}
    }

    let new_id = zone.get_id().clone();

    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    match ds_guard.insert_zone(zone) {
        Ok(_) => {},
        Err(err) => {
            return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR).json(ErrorResponse {
                error_text: format!("Can't insert zone ID: {}. Error: {}", new_id, err)
            }));
        }
    }

    drop(ds_guard);

    return Ok(HttpResponse::Ok().json(PolygonCreateResponse{
        polygon_id: new_id
    }));
}



#[derive(Debug, Deserialize)]
pub struct PolygonReplaceAllRequest {
    pub data: Vec<PolygonCreateRequest>
}

#[derive(Debug, Serialize)]
pub struct PolygonReplaceAllResponse {
    pub polygons_ids: Vec<String>
}

//
// curl -XPOST 'http://localhost:42001/api/mutations/replace_all' -d '{"data":[{"lane_number":0,"lane_direction":0,"pixel_points":[[1,1],[50,1],[50,50],[1,50]],"spatial_points":[[37.618908137083054,54.20564619851147],[37.61891517788172,54.20564502193819],[37.618927247822285,54.205668749493036],[37.61892020702362,54.2056701221611]],"color_rgb":[130,130,0]},{"lane_number":1,"lane_direction":0,"pixel_points":[[55,55],[105,55],[105,105],[55,105]],"spatial_points":[[37.618908137083054,54.20564619851147],[37.61891517788172,54.20564502193819],[37.618927247822285,54.205668749493036],[37.61892020702362,54.2056701221611]],"color_rgb":[130,0,130]}]}' -H 'Content-Type: application/json'
//
pub async fn replace_all(data: web::Data<APIStorage>, _new_zones: web::Json<PolygonReplaceAllRequest>) -> Result<HttpResponse, Error> {

    if _new_zones.data.len() == 0 {
        return Ok(HttpResponse::build(StatusCode::BAD_REQUEST).json(ErrorResponse {
            error_text: "No polygons".to_string()
        }));
    }

    // Mark data for clean
    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard.zones.read().expect("Spatial data is poisoned [RWLock]");
    let need_to_clean: Vec<String> = zones.iter().map(|poly| poly.0.clone()).collect();
    drop(zones);
    drop(ds_guard);

    // Add new data
    let mut response = vec![];
    for new_zone in _new_zones.data.iter() {
        let mut zone = Zone::default();
        match new_zone.pixel_points {
            Some(data) => {
                zone.update_pixel_map(data);
            },
            _ => {}
        }

        match new_zone.spatial_points {
            Some(data) => {
                zone.update_spatial_map(data);
            },
            _ => {}
        }

        match new_zone.lane_direction {
            Some(val) => {
                zone.set_road_lane_direction(val);
            },
            _ => {}
        }

        match new_zone.lane_number {
            Some(val) => {
                zone.set_road_lane_num(val);
            },
            _ => {}
        }

        match new_zone.color_rgb {
            Some(val) => {
                zone.set_color(val);
            },
            _ => {}
        }

        match &new_zone.virtual_line {
            Some(val) => {
                let mut new_line = VirtualLine::new_from(val.geometry, val.direction);
                new_line.set_color(val.color_rgb[2], val.color_rgb[1], val.color_rgb[0]);
                zone.set_virtual_line(new_line);
            },
            _ => {}
        }

        let new_id = zone.get_id().clone();


        let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
        match ds_guard.insert_zone(zone) {
            Ok(_) => {},
            Err(err) => {
                return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR).json(ErrorResponse {
                    error_text: format!("Can't insert zone ID: {}. Error: {}", new_id, err)
                }));
            }
        }
        drop(ds_guard);

        response.push(new_id);
    }

    // Clean data
    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    for zone_id in need_to_clean.iter() {
        match ds_guard.delete_zone(zone_id) {
            Ok(_) => {},
            Err(err) => {
                return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR).json(ErrorResponse {
                    error_text: format!("Can't delete obsolete zone ID: {}. Error: {}", zone_id, err)
                }));
            }
        }
    }

    return Ok(HttpResponse::Ok().json(PolygonReplaceAllResponse{
        polygons_ids: response
    }));
}
