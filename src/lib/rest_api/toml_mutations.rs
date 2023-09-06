use actix_web::{HttpResponse, web, Error};
use serde::{
    Serialize
};
use crate::lib::rest_api::APIStorage;
use crate::settings::RoadLanesSettings;
use crate::settings::VirtualLineSettings;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error_text: String,
}

#[derive(Debug, Serialize)]
pub struct SucccessResponse<'a> {
    pub message: &'a str,
}
pub async fn save_toml(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    println!("Saving TOML configuration");
    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard.zones.read().expect("Spatial data is poisoned [RWLock]");
    let mut setting_cloned = data.app_settings.get_copy_no_roads();
    for (_, zone_guarded) in zones.iter() {
        let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
        setting_cloned.road_lanes.push(RoadLanesSettings{
            color_rgb: [zone.color[2] as i16, zone.color[1] as i16, zone.color[0] as i16], // BGR -> RGB
            geometry: zone.get_pixel_coordinates().iter().map(|pt| [pt.x as i32, pt.y as i32]).collect(),
            geometry_wgs84: zone.get_spatial_coordinates_epsg4326().iter().map(|pt| [pt.x, pt.y]).collect(),
            lane_direction: zone.road_lane_direction,
            lane_number: zone.road_lane_num,
            virtual_line: match &zone.get_virtual_line() {
                Some(vl) => {
                    Some(VirtualLineSettings{
                        geometry: vec![
                            [vl.line[0].x as i32, vl.line[0].y as i32],
                            [vl.line[1].x as i32, vl.line[1].y as i32],
                        ],
                        color_rgb: [vl.color[2] as i16, vl.color[1] as i16, vl.color[0] as i16], // BGR -> RGB
                    })
                },
                None => {
                    None
                }
            },
        });
        drop(zone);
    }
    drop(zones);
    drop(ds_guard);
    match setting_cloned.save(&data.settings_filename) {
        Ok(_) => {},
        Err(_err) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_text: format!("Can't save TOML due the error: {}", _err),
            }));
        },
    };
    return Ok(HttpResponse::Ok().json(SucccessResponse{
        message: "ok"
    }));
}

