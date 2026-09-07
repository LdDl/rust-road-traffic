use actix_web::{Error, HttpResponse, web};
use serde::Serialize;
use serde_json::{Map, Value};
use tracing::info;
use utoipa::ToSchema;

use crate::lib::logging;
use crate::rest_api::APIStorage;
use crate::settings::{AppSettings, ZONES_KEY, changed_paths, needs_restart};

/// Error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    /// What was wrong with the request
    #[schema(example = "Invalid tracker type: 'sort'. Supported: iou_naive, bytetrack")]
    pub error_text: String,
}

/// Result of a configuration change
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateConfigResponse {
    #[schema(example = "ok")]
    pub message: &'static str,
    /// Whether the application has to be restarted for the change to take
    /// effect: `POST /api/mutations/restart`
    pub restart_required: bool,
    /// Dotted paths of the settings that actually changed
    #[schema(example = json!(["input.video_src"]))]
    pub changed: Vec<String>,
}

#[utoipa::path(
    get,
    tag = "Configuration",
    path = "/api/config",
    responses(
        (status = 200, description = "Current configuration, without the zones and the Redis password", body = Object)
    )
)]
/// The configuration as the file has it, minus the zones (they have their own
/// endpoints) and minus the Redis password, which is reported as
/// `password_set` instead of being handed out
pub async fn get_config(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    let settings = data
        .app_settings
        .read()
        .expect("Settings are poisoned [RwLock]");
    let value = match public_view(&settings) {
        Ok(value) => value,
        Err(err) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_text: format!("Can't read the configuration: {}", err),
            }));
        }
    };
    Ok(HttpResponse::Ok().json(value))
}

#[utoipa::path(
    put,
    tag = "Configuration",
    path = "/api/config",
    request_body(content = Object, description = "The sections and keys to change, e.g. {\"input\": {\"video_src\": \"rtsp://...\"}}"),
    responses(
        (status = 200, description = "Saved to the configuration file", body = UpdateConfigResponse),
        (status = 400, description = "The request asks for something the app would not accept", body = ErrorResponse),
        (status = 500, description = "Can't write the configuration file", body = ErrorResponse)
    )
)]
/// Changes settings and writes them to the configuration file at once, so that
/// the change survives the restart it usually needs. Only the keys present in
/// the request are touched; the answer says which ones actually changed and
/// whether a restart is due
pub async fn update_config(
    data: web::Data<APIStorage>,
    patch: web::Json<Value>,
) -> Result<HttpResponse, Error> {
    let mut settings = data
        .app_settings
        .write()
        .expect("Settings are poisoned [RwLock]");

    let patch = patch.into_inner();
    if !patch.is_object() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse {
            error_text: "Expected an object with the sections to change".to_string(),
        }));
    }
    if patch.get(ZONES_KEY).is_some() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse {
            error_text: "Zones are changed through /api/mutations/zones/*, not here".to_string(),
        }));
    }

    let current = match serde_json::to_value(&*settings) {
        Ok(value) => value,
        Err(err) => {
            return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
                error_text: format!("Can't read the configuration: {}", err),
            }));
        }
    };
    let mut merged = current.clone();
    merge(&mut merged, &patch);

    let updated: AppSettings = match serde_json::from_value(merged.clone()) {
        Ok(settings) => settings,
        Err(err) => {
            return Ok(HttpResponse::BadRequest().json(ErrorResponse {
                error_text: format!("The configuration would not make sense: {}", err),
            }));
        }
    };
    if let Err(err) = updated.validate() {
        return Ok(HttpResponse::BadRequest().json(ErrorResponse {
            error_text: err.to_string(),
        }));
    }

    let mut changed = Vec::new();
    changed_paths(&current, &merged, "", &mut changed);
    changed.retain(|path| path != ZONES_KEY && !path.starts_with(&format!("{ZONES_KEY}.")));
    if changed.is_empty() {
        return Ok(HttpResponse::Ok().json(UpdateConfigResponse {
            message: "ok",
            restart_required: false,
            changed,
        }));
    }

    if let Err(err) = updated.save(&data.settings_filename) {
        return Ok(HttpResponse::InternalServerError().json(ErrorResponse {
            error_text: format!("Can't save the configuration file: {}", err),
        }));
    }
    *settings = updated;

    // Whatever can be applied right away, is: the rest waits for a restart
    if changed.iter().any(|path| path == "verbose.level") {
        if let Some(level) = settings.verbose.as_ref().and_then(|v| v.level.as_deref()) {
            logging::set_log_level(level);
        }
    }
    let restart_required = changed.iter().any(|path| needs_restart(path));
    info!(
        scope = logging::SCOPE_REST_API,
        changed = ?changed,
        restart_required,
        file = %data.settings_filename,
        "Configuration updated"
    );
    Ok(HttpResponse::Ok().json(UpdateConfigResponse {
        message: "ok",
        restart_required,
        changed,
    }))
}

/// The settings as JSON, with the zones dropped and the Redis password replaced
/// by whether there is one
fn public_view(settings: &AppSettings) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(settings)?;
    if let Some(object) = value.as_object_mut() {
        object.remove(ZONES_KEY);
        if let Some(Value::Object(redis)) = object.get_mut("redis_publisher") {
            let has_password = redis
                .get("password")
                .and_then(Value::as_str)
                .is_some_and(|password| !password.is_empty());
            redis.remove("password");
            redis.insert("password_set".to_string(), Value::Bool(has_password));
        }
    }
    Ok(value)
}

/// Copies the values of `patch` into `base`, going into objects rather than
/// replacing them, so that a request may name a single key of a section
fn merge(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base), Value::Object(patch)) => {
            for (key, value) in patch {
                match base.get_mut(key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn merge_goes_into_sections() {
        let mut base = json(r#"{"input":{"video_src":"a.mp4","nth":2},"worker":{"ms":30}}"#);
        merge(&mut base, &json(r#"{"input":{"video_src":"rtsp://x"}}"#));
        assert_eq!(
            base,
            json(r#"{"input":{"video_src":"rtsp://x","nth":2},"worker":{"ms":30}}"#)
        );
    }

    #[test]
    fn merge_adds_missing_keys() {
        let mut base = json(r#"{"tracking":{"type":"iou_naive"}}"#);
        merge(&mut base, &json(r#"{"tracking":{"max_lost_seconds":2.0}}"#));
        assert_eq!(
            base,
            json(r#"{"tracking":{"type":"iou_naive","max_lost_seconds":2.0}}"#)
        );
    }
}
