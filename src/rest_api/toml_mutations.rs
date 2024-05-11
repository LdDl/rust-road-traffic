use actix_web::{HttpResponse, web, Error};
use serde::Serialize;
use utoipa::ToSchema;
use crate::rest_api::APIStorage;
use crate::settings::RoadLanesSettings;
use crate::settings::VirtualLineSettings;

/// Error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// Error message
    #[schema(example = "Can't save TOML due the error")]
    pub error_text: String,
}

/// Response for the save configuration file request
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateTOMLResponse<'a> {
    /// Message
    #[schema(example = "ok")]
    pub message: &'a str,
}

#[utoipa::path(
    get,
    tag = "Configuration file mutations",
    path = "/api/mutations/save_toml",
    responses(
        (status = 201, description = "All zones has been overwritten", body = UpdateTOMLResponse),
        (status = 500, description = "Internal error", body = ErrorResponse)
    )
)]
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
                        geometry: vl.line,
                        color_rgb: [vl.color[0] as i16, vl.color[1] as i16, vl.color[2] as i16], // BGR -> RGB
                        direction: vl.direction.to_string(),
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
    return Ok(HttpResponse::Ok().json(UpdateTOMLResponse{
        message: "ok"
    }));
}

