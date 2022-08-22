use actix_web::{HttpResponse, web, Error, http::StatusCode};
use serde::{
    Deserialize,
    Serialize
};
use crate::lib::rest_api::Storage;

#[derive(Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
    pub error_text: String,
}

pub async fn save_toml(data: web::Data<Storage>) -> Result<HttpResponse, Error> {
    println!("Saving TOML configuration. TBD");
    // @todo: cast polygons fields to application settings
    match data.app_settings.save(&data.settings_filename) {
        Ok(_) => {},
        Err(_err) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_text: format!("Can't save TOML due the error: {}", _err),
            }));
        },
    };
    return Ok(HttpResponse::Ok().json("{}"));
}

