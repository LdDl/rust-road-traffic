use actix_web::{HttpResponse, web, Error};
use crate::lib::zones::geojson::ZonesFeatureCollection;
use crate::rest_api::APIStorage;

#[utoipa::path(
    get,
    tag = "Zones",
    path = "/api/polygons/geojson",
    responses(
        (status = 200, description = "List of detections zones", body = ZonesFeatureCollection)
    )
)]
pub async fn all_zones_list(data: web::Data<APIStorage>) -> Result<HttpResponse, Error> {
    let ds_guard = data.data_storage.read().expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard.zones.read().expect("Spatial data is poisoned [RWLock]");
    let mut ans = ZonesFeatureCollection::new();

    for (_, zone_guarded) in zones.iter() {
        let zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
        let geo_feature = zone.to_geojson();
        ans.features.push(geo_feature);
    }

    return Ok(HttpResponse::Ok().json(ans));
}
