use actix_web::{Error, HttpResponse};
use serde::Serialize;
use std::thread;
use std::time::Duration;
use tracing::info;
use utoipa::ToSchema;

use crate::lib::logging;
use crate::lib::restart::restart_process;

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

#[utoipa::path(
    post,
    tag = "Application",
    path = "/api/mutations/restart",
    responses(
        (status = 200, description = "The application is restarting", body = RestartResponse)
    )
)]
/// Restarts the application with the configuration file as it is on disk.
///
/// Nothing is saved first: unsaved zones stay unsaved and a config edited over
/// SSH is picked up as written. The reply is sent before the restart happens,
/// so the caller should wait for `/api/ping` to answer again
pub async fn restart() -> Result<HttpResponse, Error> {
    info!(
        scope = logging::SCOPE_REST_API,
        grace_ms = GRACE.as_millis() as u64,
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
