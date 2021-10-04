use actix_web::{HttpResponse, web, Responder};
use std::sync::{Arc};

use crate::lib::polygons::ConvexPolygons;

async fn say_ping() -> impl Responder {
    HttpResponse::Ok().body("pong")
}

pub fn polygons_list(data: web::Data<Arc<ConvexPolygons>>) -> HttpResponse {
    let data_storage = data.get_ref().clone();
    let ans = data_storage.to_geojson();
    return HttpResponse::Ok().json(ans);
}

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .service(
            web::scope("/api")
            .route("/ping", web::get().to(say_ping))
            .service(
                web::scope("/polygons")
                .route("/geojson", web::get().to(polygons_list))
            )
            .service(
                web::scope("/stats")
                // .route("/all", web::get().to(/*todo*/))
                // .route("/by_polygon_id/{polygon_id}", web::get().to(/*todo*/))
            )
        );
}
