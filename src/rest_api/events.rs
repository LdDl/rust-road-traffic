use actix_web::{HttpResponse, web::Bytes};
use futures::{SinkExt, StreamExt, channel::mpsc};
use serde::Serialize;
use std::time::Duration;

#[derive(Serialize)]
enum EventType {
    #[serde(rename = "vehicle.passed")]
    VehiclePassed,
}

impl EventType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::VehiclePassed => "vehicle.passed",
        }
    }
}

#[derive(Serialize)]
struct DemoPassageEvent {
    event_id: &'static str,
    #[serde(rename = "type")]
    event_type: EventType,
    equipment_id: &'static str,
    passage_id: &'static str,
    started_at: &'static str,
    ended_at: &'static str,
    vehicle_class: String,
    plate: &'static str,
}

#[utoipa::path(
    get,
    tag = "Events",
    path = "/api/events/stream",
    responses(
        (status = 200, description = "test sse", body = String, content_type = "text/event-stream")
    )
)]
pub async fn stream() -> Result<HttpResponse, actix_web::Error> {
    let event = DemoPassageEvent {
        event_id: "demo-passage-001",
        event_type: EventType::VehiclePassed,
        equipment_id: "demo-camera",
        passage_id: "demo-passage-001",
        started_at: "2026-10-06T09:00:00Z",
        ended_at: "2026-10-06T09:00:03Z",
        vehicle_class: "car".to_owned(),
        plate: "A123BC77",
    };
    let data = serde_json::to_string(&event).map_err(actix_web::error::ErrorInternalServerError)?;
    let frame = Bytes::from(format!(
        "event: {}\ndata: {data}\n\n",
        event.event_type.as_str()
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
