use crate::lib::logging;
use crate::rest_api::APIStorage;
use crate::rest_api::change_state::{ChangeState, live_road_lanes};
use crate::rest_api::errors::ErrorResponse;
use actix_web::{Error, HttpResponse, web};
use serde::Serialize;
use tracing::{error, info};
use utoipa::ToSchema;

/// Response for the save configuration file request
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateTOMLResponse<'a> {
    /// Message
    #[schema(example = "ok")]
    pub message: &'a str,
    #[serde(flatten)]
    pub state: ChangeState,
}

#[utoipa::path(
    get,
    tag = "Configuration file mutations",
    path = "/api/mutations/save_toml",
    responses(
        (status = 200, description = "Settings and zones are in the file; the answer says whether a restart is due", body = UpdateTOMLResponse),
        (status = 500, description = "Internal error", body = ErrorResponse)
    )
)]
/// The one thing that writes the configuration file: the settings as they
/// are held in memory, with the zones the running process uses. Everything
/// else only changes memory, and a restart reads the file, so this is what
/// makes a change survive one
pub async fn save_toml(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    info!(scope = logging::SCOPE_REST_API, file = %data.settings_filename, "Saving TOML configuration");
    // Held for the whole write, so that a settings change arriving meanwhile
    // waits for the file instead of being made on a copy about to be replaced
    let mut settings = data
        .app_settings
        .write()
        .expect("Settings are poisoned [RwLock]");
    let zones = live_road_lanes(&data.data_storage);
    let mut to_save = settings.clone();
    to_save.road_lanes = Some(zones.clone());
    if let Err(err) = to_save.save(&data.settings_filename) {
        // The path and what the file system said about it stay in the log
        error!(
            scope = logging::SCOPE_REST_API,
            file = %data.settings_filename,
            error = %err,
            "Can't save the configuration"
        );
        return Ok(HttpResponse::InternalServerError()
            .json(ErrorResponse::text("could not save the configuration")));
    }
    settings.road_lanes = Some(zones);
    *data
        .saved_settings
        .write()
        .expect("Saved settings are poisoned [RwLock]") = to_save;
    drop(settings);
    Ok(HttpResponse::Ok().json(UpdateTOMLResponse {
        message: "ok",
        state: ChangeState::of(&data),
    }))
}
