use crate::rest_api::APIStorage;
use actix_web::{HttpResponse, web};
use futures::{SinkExt, channel::mpsc};
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
struct BoundingBox {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Serialize)]
struct OcrSymbol {
    class: String,
    confidence: f32,
    bbox: BoundingBox,
}

#[derive(Serialize)]
struct OcrResult {
    number: String,
    symbols: Vec<OcrSymbol>,
}

#[derive(Serialize)]
struct PlateDetection {
    class: String,
    bbox: BoundingBox,
    ocr: Option<OcrResult>,
}

#[derive(Serialize)]
struct DemoPassageEvent {
    event_id: &'static str,
    #[serde(rename = "type")]
    event_type: EventType,
    equipment_id: String,
    passage_id: &'static str,
    started_at: &'static str,
    ended_at: &'static str,
    vehicle_class: String,
    plate: Option<PlateDetection>,
    frame_base64: Option<String>,
    frame_width: u32,
    frame_height: u32,
    vehicle_bbox: BoundingBox,
}

#[utoipa::path(
    get,
    tag = "Events",
    path = "/api/events/stream",
    responses(
        (status = 200, description = "test sse", body = String, content_type = "text/event-stream")
    )
)]
pub async fn stream(data: web::Data<APIStorage>) -> HttpResponse {
    let (mut sender, receiver) = mpsc::channel(1);

    actix_web::rt::spawn(async move {
        loop {
            let equipment_id = data
                .app_settings
                .read()
                .expect("Settings are poisoned [RwLock]")
                .equipment_info
                .id
                .clone();
            let plate = "A123BC77";
            let event = DemoPassageEvent {
                event_id: "demo-passage-001",
                event_type: EventType::VehiclePassed,
                equipment_id,
                passage_id: "demo-passage-001",
                started_at: "2026-10-06T09:00:00Z",
                ended_at: "2026-10-06T09:00:03Z",
                vehicle_class: "car".to_owned(),
                frame_base64: None,
                frame_width: 1920,
                frame_height: 1080,
                vehicle_bbox: BoundingBox {
                    x: 600,
                    y: 400,
                    width: 500,
                    height: 300,
                },
                plate: Some(PlateDetection {
                    class: "civilian".to_owned(),
                    bbox: BoundingBox {
                        x: 780,
                        y: 610,
                        width: 120,
                        height: 30,
                    },
                    ocr: Some(OcrResult {
                        number: plate.to_owned(),
                        symbols: plate
                            .chars()
                            .enumerate()
                            .map(|(index, symbol)| OcrSymbol {
                                class: symbol.to_string(),
                                confidence: 0.98,
                                bbox: BoundingBox {
                                    x: 784 + index as u32 * 14,
                                    y: 613,
                                    width: 12,
                                    height: 24,
                                },
                            })
                            .collect(),
                    }),
                }),
            };
            let frame = serde_json::to_string(&event)
                .map(|json| {
                    web::Bytes::from(format!(
                        "event: {}\ndata: {json}\n\n",
                        event.event_type.as_str()
                    ))
                })
                .map_err(actix_web::error::ErrorInternalServerError);
            if sender.send(frame).await.is_err() {
                break;
            }
            actix_web::rt::time::sleep(Duration::from_secs(5)).await;
        }
    });

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache, no-transform"))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(receiver)
}
