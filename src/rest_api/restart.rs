use actix_web::{Error, HttpResponse, web};
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;
use tracing::info;
use utoipa::{IntoParams, ToSchema};

use crate::lib::logging;
use crate::lib::restart::restart_process;
use crate::rest_api::APIStorage;
use crate::rest_api::change_state::ChangeState;

/// Time given to the reply and the buffered log lines to leave the process
/// before its image is replaced
const GRACE: Duration = Duration::from_millis(300);

/// Response for the restart request
#[derive(Debug, Serialize, ToSchema)]
pub struct RestartResponse<'a> {
    /// Message
    #[schema(example = "restarting")]
    pub message: &'a str,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct RestartQuery {
    /// Restart even though something is unsaved; whatever is unsaved is lost
    pub force: Option<bool>,
}

/// Why the restart did not happen, with the same block of what is unsaved
/// that every changing request answers with
#[derive(Debug, Serialize, ToSchema)]
pub struct RestartRefused {
    #[schema(
        example = "Unsaved changes would be lost: save them first, or restart with ?force=true"
    )]
    pub error_text: String,
    #[serde(flatten)]
    pub state: ChangeState,
}

#[utoipa::path(
    post,
    tag = "Application",
    path = "/api/mutations/restart",
    params(RestartQuery),
    responses(
        (status = 200, description = "The application is restarting", body = RestartResponse),
        (status = 409, description = "Something is unsaved and would be lost; nothing happened", body = RestartRefused)
    )
)]
/// Restarts the application with the configuration file as it is on disk.
///
/// A restart reads the file, so anything held only in memory - settings
/// changed through `PUT /api/config`, zones, even the settings that took
/// effect at once - would be rolled back. While anything is unsaved the
/// request is refused with 409 and the list of it; `?force=true` restarts
/// anyway. The check is made here rather than left to a client because only
/// the server sees every client: another tab, a script or curl included.
///
/// The reply is sent before the restart happens, so the caller should wait
/// for `/api/ping` to answer again
pub async fn restart(
    data: web::Data<APIStorage>,
    query: web::Query<RestartQuery>,
) -> Result<HttpResponse, Error> {
    let force = query.force.unwrap_or(false);
    if !force {
        let state = ChangeState::of(&data);
        if state.save_required {
            info!(
                scope = logging::SCOPE_REST_API,
                unsaved = ?state.unsaved_changes,
                "Restart refused: unsaved changes would be lost"
            );
            return Ok(HttpResponse::Conflict().json(RestartRefused {
                error_text:
                    "Unsaved changes would be lost: save them first, or restart with ?force=true"
                        .to_string(),
                state,
            }));
        }
    }
    info!(
        scope = logging::SCOPE_REST_API,
        grace_ms = GRACE.as_millis() as u64,
        force,
        "Restart requested"
    );
    thread::spawn(|| {
        thread::sleep(GRACE);
        restart_process();
    });
    Ok(HttpResponse::Ok().json(RestartResponse {
        message: "restarting",
    }))
}
