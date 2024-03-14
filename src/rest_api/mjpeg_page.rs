use actix_web::{HttpResponse,  Responder};

pub async fn mjpeg_page() -> impl Responder {
    let content = include_str!("mjpeg.html");
    return HttpResponse::Ok().append_header(("Content-Type", "text/html")).body(content);
}