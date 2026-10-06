use actix_web::{HttpResponse, web::Bytes};
use futures::{SinkExt, StreamExt, channel::mpsc};
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize)]
struct DemoPassageEvent {
    demo: bool,
    event_id: &'static str,
    #[serde(rename = "type")]
    event_type: &'static str,
    equipment_id: &'static str,
    passage_id: &'static str,
    started_at: &'static str,
    ended_at: &'static str,
    vehicle_class: &'static str,
    plate: &'static str,
}

const DEMO_EVENT: DemoPassageEvent = DemoPassageEvent {
    demo: true,
    event_id: "demo-passage-001",
    event_type: "vehicle.passed",
    equipment_id: "demo-camera",
    passage_id: "demo-passage-001",
    started_at: "2026-10-06T09:00:00Z",
    ended_at: "2026-10-06T09:00:03Z",
    vehicle_class: "car",
    plate: "A123BC77",
};

#[utoipa::path(
    get,
    tag = "Events",
    path = "/api/events/stream",
    responses(
        (status = 200, description = "test sse", body = String, content_type = "text/event-stream")
    )
)]
pub async fn stream() -> Result<HttpResponse, actix_web::Error> {
    let data =
        serde_json::to_string(&DEMO_EVENT).map_err(actix_web::error::ErrorInternalServerError)?;
    let frame = Bytes::from(format!(
        "event: {}\ndata: {data}\n\n",
        DEMO_EVENT.event_type
    ));
    let (mut sender, receiver) = mpsc::channel(1);

    actix_web::rt::spawn(async move {
        loop {
            if sender.send(frame.clone()).await.is_err() {
                break;
            }
            actix_web::rt::time::sleep(Duration::from_secs(5)).await;
        }
    });

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache, no-transform"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(receiver.map(Ok::<_, actix_web::Error>)))
}
