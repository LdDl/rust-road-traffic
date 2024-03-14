use actix_web::{HttpResponse, web, Responder};
use crate::rest_api::APIStorage;

pub async fn add_new_client(ds: web::Data<APIStorage>) -> impl Responder {
    let rx = ds.mjpeg_broadcaster.lock().unwrap().add_client();
    HttpResponse::Ok()
        .append_header(("Cache-Control", "no-store, must-revalidate"))
        .append_header(("Pragma", "no-cache"))
        .append_header(("Expires", "0"))
        .append_header(("Connection", "close"))
        .append_header(("Content-Type", "multipart/x-mixed-replace;boundary=boundarydonotcross"))
        .streaming(rx)
}