use crate::lib::logging;
use crate::rest_api::APIStorage;
use actix_web::{HttpResponse, web};
use futures::{StreamExt, stream};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;
use tracing::warn;

#[utoipa::path(
    get,
    tag = "Events",
    path = "/api/events/stream",
    responses(
        (status = 200, description = "test sse", body = String, content_type = "text/event-stream")
    )
)]
pub async fn stream(data: web::Data<APIStorage>) -> HttpResponse {
    let receiver = data.vehicle_events.subscribe();
    let events = stream::unfold(receiver, |mut receiver| async move {
        let frame = match tokio::time::timeout(Duration::from_secs(15), receiver.recv()).await {
            Ok(Ok(event)) => serde_json::to_string(event.as_ref())
                .map(|json| {
                    web::Bytes::from(format!(
                        "event: {}\ndata: {json}\n\n",
                        event.event_type.as_str()
                    ))
                })
                .map_err(actix_web::error::ErrorInternalServerError),
            Ok(Err(RecvError::Closed)) => return None,
            Ok(Err(RecvError::Lagged(skipped))) => {
                warn!(
                    scope = logging::SCOPE_REST_API,
                    skipped, "Closing SSE connection because the client missed vehicle events"
                );
                return None;
            }
            Err(_) => Ok(web::Bytes::from_static(b": keep-alive\n\n")),
        };
        Some((frame, receiver))
    });
    let connected = stream::once(async {
        Ok::<_, actix_web::Error>(web::Bytes::from_static(b": connected\n\n"))
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache, no-transform"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(connected.chain(events))
}
