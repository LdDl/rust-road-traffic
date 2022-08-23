use actix_web::{HttpResponse, web, Error};
use serde::{
    Deserialize,
    Serialize
};
use crate::lib::rest_api::Storage;
use crate::settings::RoadLanesSettings;

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    pub error_text: String,
}
pub async fn save_toml(data: web::Data<Storage>) -> Result<HttpResponse, Error> {
    println!("Saving TOML configuration. TBD");
    let data_storage = data.data_storage.as_ref().clone();
    let data_expected = data_storage.read().expect("expect: polygons_list");
    let data_expected_polygons = data_expected.polygons.read().expect("expect: polygons_list");
    let mut setting_cloned = data.app_settings.get_copy_no_roads();
    for (_, v) in data_expected_polygons.iter() {
        let polygon = v.lock().expect("Mutex poisoned");
        setting_cloned.road_lanes.push(RoadLanesSettings{
            color_rgb: [polygon.color[0] as i16, polygon.color[1] as i16, polygon.color[2] as i16],
            geometry: vec![],
            geometry_wgs84: vec![],
            lane_direction: polygon.road_lane_direction,
            lane_number: polygon.road_lane_num
        });
        drop(polygon);
    }
    match setting_cloned.save(&data.settings_filename) {
        Ok(_) => {},
        Err(_err) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_text: format!("Can't save TOML due the error: {}", _err),
            }));
        },
    };
    return Ok(HttpResponse::Ok().json("{}"));
}

