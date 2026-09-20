use crate::lib::logging;
use crate::lib::zones::{VirtualLine, VirtualLineDirection, Zone};
use crate::rest_api::APIStorage;
use crate::rest_api::change_state::ChangeState;
use crate::rest_api::errors::{ErrorResponse, FieldError};
use actix_web::{Error, HttpResponse, http::StatusCode, web};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;
use tracing::{error, info};
use utoipa::ToSchema;

/// The body of the request to update the zone
#[derive(Debug, Deserialize, ToSchema)]
pub struct ZoneUpdateRequest {
    /// Zone identifier
    #[schema(example = "dir_0_lane_1")]
    pub zone_id: String,
    /// 4 points represinting zone for the image coordinates
    #[schema(example = json!([[299, 222], [572, 265], [547, 66], [359, 69]]))]
    pub pixel_points: Option<[[u16; 2]; 4]>,
    /// 4 points represinting zone for the spatial coordinates (WGS84)
    /// Order of points should be the same as for the pixel_points
    #[schema(example = json!([[37.61896269287956, 54.205680987916566], [37.61892595368445, 54.205685474312446], [37.618908137083054, 54.20564619851147], [37.618944938776394, 54.20563975740504]]))]
    pub spatial_points: Option<[[f32; 2]; 4]>,
    /// Road lane number
    #[schema(example = 939)]
    pub lane_number: Option<u16>,
    /// Road lane direction
    #[schema(example = 1)]
    pub lane_direction: Option<u8>,
    /// Color of the zone
    #[schema(example = json!([130, 0, 100]))]
    pub color_rgb: Option<[i16; 3]>,
    /// Virtual line
    pub virtual_line: Option<VirtualLineRequestData>,
}

/// Respone on zone update request
#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneUpdateResponse<'a> {
    /// Message
    #[schema(example = "ok")]
    pub message: &'a str,
    #[serde(flatten)]
    pub state: ChangeState,
}

#[utoipa::path(
    post,
    tag = "Zones mutations",
    path = "/api/mutations/zones/update",
    request_body = ZoneUpdateRequest,
    responses(
        (status = 200, description = "Specific zone has been updated", body = ZoneUpdateResponse),
        (status = 424, description = "Failed dependency", body = ErrorResponse)
    )
)]
pub async fn update_zone(
    data: web::Data<APIStorage>,
    _update_zone: web::Json<ZoneUpdateRequest>,
) -> Result<HttpResponse, Error> {
    let ds_guard = data
        .data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    let mut zones = ds_guard
        .zones
        .write()
        .expect("Spatial data is poisoned [RWLock]");

    let zone_guarded = match zones.get_mut(&_update_zone.zone_id) {
        /* Check if polygon with such identifier exists */
        Some(val) => val,
        None => {
            return Ok(HttpResponse::build(StatusCode::FAILED_DEPENDENCY).json(
                ErrorResponse::text(format!(
                    "No such zone. Requested ID: {}",
                    _update_zone.zone_id
                )),
            ));
        }
    };

    // @todo need to deal with those (see main function):
    // polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);

    match _update_zone.pixel_points {
        Some(data) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.update_pixel_map(data);
            drop(zone)
        }
        _ => {}
    }

    match _update_zone.spatial_points {
        Some(data) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.update_spatial_map(data);
            drop(zone)
        }
        _ => {}
    }

    match _update_zone.lane_direction {
        Some(val) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.set_road_lane_direction(val);
            drop(zone)
        }
        _ => {}
    }

    match _update_zone.lane_number {
        Some(val) => {
            info!(
                scope = logging::SCOPE_REST_API,
                lane_number = val,
                "Zone lane number updated"
            );
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.set_road_lane_num(val);
            drop(zone)
        }
        _ => {}
    }

    match _update_zone.color_rgb {
        Some(val) => {
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            zone.set_color(val);
            zone.set_line_color(val);
            drop(zone)
        }
        _ => {}
    }

    match &_update_zone.virtual_line {
        Some(val) => {
            let dir = VirtualLineDirection::from_str(val.direction.as_str()).unwrap_or_default();
            let mut new_line = VirtualLine::new_from(val.geometry, dir);
            let mut zone = zone_guarded.lock().expect("Zone is poisoned [Mutex]");
            if let Some(rgb) = val.color_rgb {
                new_line.set_color_rgb(rgb[0], rgb[1], rgb[2]);
            } else {
                let zone_color = zone.get_color();
                new_line.set_color_rgb(zone_color[0], zone_color[1], zone_color[2]);
            };
            zone.set_virtual_line(new_line);
            drop(zone)
        }
        _ => {}
    }

    drop(zone_guarded);
    drop(zones);

    // Rebuild zone grid if pixel coordinates changed
    if _update_zone.pixel_points.is_some() {
        let _ = ds_guard.rebuild_zone_grid();
    }
    drop(ds_guard);

    return Ok(HttpResponse::Ok().json(ZoneUpdateResponse {
        message: "ok",
        state: ChangeState::of(&data),
    }));
}

/// The body of the request to delete the zone
#[derive(Debug, Deserialize, ToSchema)]
pub struct ZoneDeleteRequest {
    /// Zone identifier
    #[schema(example = "dir_0_lane_1")]
    pub zone_id: String,
}

/// Respone on zone delete request
#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneDeleteResponse<'a> {
    /// Message
    #[schema(example = "ok")]
    pub message: &'a str,
    #[serde(flatten)]
    pub state: ChangeState,
}

#[utoipa::path(
    post,
    tag = "Zones mutations",
    path = "/api/mutations/zones/delete",
    request_body = ZoneDeleteRequest,
    responses(
        (status = 200, description = "Zone has been deleted", body = ZoneDeleteResponse),
        (status = 500, description = "Internal error", body = ErrorResponse)
    )
)]
pub async fn delete_zone(
    data: web::Data<APIStorage>,
    _delete_zone: web::Json<ZoneDeleteRequest>,
) -> Result<HttpResponse, Error> {
    let ds_guard = data
        .data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    match ds_guard.delete_zone(&_delete_zone.zone_id) {
        Ok(_) => {}
        Err(err) => {
            // Why it failed describes how this app is built, so it goes to the
            // log; the caller is told what did not happen
            error!(
                scope = logging::SCOPE_REST_API,
                zone_id = %_delete_zone.zone_id,
                error = %err,
                "Can't delete the zone"
            );
            return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                .json(ErrorResponse::text("could not delete the zone")));
        }
    }
    // Rebuild zone grid after deletion
    let _ = ds_guard.rebuild_zone_grid();
    drop(ds_guard);
    // 200 rather than 204: the answer carries what is left unsaved
    return Ok(HttpResponse::Ok().json(ZoneDeleteResponse {
        message: "ok",
        state: ChangeState::of(&data),
    }));
}

/// The body of the request to create new zone
#[derive(Debug, Deserialize, ToSchema)]
pub struct ZoneCreateRequest {
    /// Identifier to keep. Hand back the one this zone already has and it
    /// stays addressable by that name; leave it out for a zone that is new,
    /// and the answer says what it was called. Creating refuses an id that is
    /// already taken, while `replace_all` takes it as meaning that zone and
    /// replaces it, statistics and all
    #[schema(example = "dir_0_lane_1")]
    pub id: Option<String>,
    /// 4 points represinting zone for the image coordinates
    #[schema(example = json!([[230, 200], [550, 235], [512, 40], [359, 69]]))]
    pub pixel_points: Option<[[u16; 2]; 4]>,
    /// 4 points represinting zone for the spatial coordinates (WGS84)
    /// Order of points should be the same as for the pixel_points
    #[schema(example = json!([[37.618908137083054, 54.20564619851147], [37.61891517788172, 54.20564502193819], [37.618927247822285, 54.205668749493036], [37.61892020702362, 54.2056701221611]]))]
    pub spatial_points: Option<[[f32; 2]; 4]>,
    /// Road lane number
    #[schema(example = 939)]
    pub lane_number: Option<u16>,
    /// Road lane direction
    #[schema(example = 33)]
    pub lane_direction: Option<u8>,
    /// Color of the zone
    #[schema(example = json!([130, 130, 0]))]
    pub color_rgb: Option<[i16; 3]>,
    /// Virtual line
    pub virtual_line: Option<VirtualLineRequestData>,
}

/// Information about virtual line
#[derive(Deserialize, Debug, ToSchema)]
pub struct VirtualLineRequestData {
    /// Line geometry. 2 points
    #[schema(example = json!([[365, 177], [540, 185]]))]
    pub geometry: [[i32; 2]; 2],
    /// Color of the line
    #[schema(example = json!([130, 70, 0]))]
    pub color_rgb: Option<[i16; 3]>,
    /// Direction. Possible values:
    /// 'inbound' - inbound traffic (towards target side)
    /// 'outbound' - outbound traffic (away from target side)
    #[schema(example = "inbound")]
    pub direction: String,
}

/// Respone on zone create request
#[derive(Debug, Serialize, ToSchema)]
pub struct ZoneCreateResponse {
    /// Zone identifier
    #[schema(example = "fad8a040-5979-47e9-9ebf-3a571f677f49")]
    pub zone_id: String,
    #[serde(flatten)]
    pub state: ChangeState,
}

#[utoipa::path(
    post,
    tag = "Zones mutations",
    path = "/api/mutations/zones/create",
    request_body = ZoneCreateRequest,
    responses(
        (status = 201, description = "Zone has been created", body = ZoneCreateResponse),
        (status = 400, description = "The requested id is empty or already taken", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse)
    )
)]
pub async fn create_zone(
    data: web::Data<APIStorage>,
    _new_zone: web::Json<ZoneCreateRequest>,
) -> Result<HttpResponse, Error> {
    // @todo need to deal with those (see main function):
    // polygon.set_target_classes(COCO_FILTERED_CLASSNAMES);

    let mut zone = Zone::default();
    // A zone keeps the name it was given; only a zone that has none gets the
    // generated one `Zone::default` came with
    let requested_id =
        match _new_zone.id.as_deref().map(str::trim) {
            Some(id) if id.is_empty() => {
                return Ok(HttpResponse::build(StatusCode::BAD_REQUEST).json(
                    ErrorResponse::fields(vec![FieldError::new("id", "must not be empty")]),
                ));
            }
            Some(id) => Some(id.to_string()),
            None => None,
        };
    match _new_zone.pixel_points {
        Some(data) => {
            zone.update_pixel_map(data);
        }
        _ => {}
    }

    match _new_zone.spatial_points {
        Some(data) => {
            zone.update_spatial_map(data);
        }
        _ => {}
    }

    match _new_zone.lane_direction {
        Some(val) => {
            zone.set_road_lane_direction(val);
        }
        _ => {}
    }

    match _new_zone.lane_number {
        Some(val) => {
            zone.set_road_lane_num(val);
        }
        _ => {}
    }

    match _new_zone.color_rgb {
        Some(val) => {
            zone.set_color(val);
        }
        _ => {}
    }

    match &_new_zone.virtual_line {
        Some(val) => {
            let dir = VirtualLineDirection::from_str(val.direction.as_str()).unwrap_or_default();
            let mut new_line = VirtualLine::new_from(val.geometry, dir);
            if let Some(rgb) = val.color_rgb {
                new_line.set_color_rgb(rgb[0], rgb[1], rgb[2]);
            } else {
                let zone_color = zone.get_color();
                new_line.set_color_rgb(zone_color[0], zone_color[1], zone_color[2]);
            };
            zone.set_virtual_line(new_line);
        }
        _ => {}
    }

    if let Some(id) = requested_id {
        zone.set_id(id);
    }
    let new_id = zone.get_id().clone();

    let ds_guard = data
        .data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    // Creating is not a way to replace: the zone that is already called this
    // keeps its statistics, and the caller is told rather than finding out by
    // the numbers
    let taken = ds_guard
        .zones
        .read()
        .expect("Spatial data is poisoned [RWLock]")
        .contains_key(&new_id);
    if taken {
        return Ok(
            HttpResponse::build(StatusCode::BAD_REQUEST).json(ErrorResponse::fields(vec![
                FieldError::new(
                    "id",
                    format!("'{new_id}' is the id of a zone that already exists"),
                ),
            ])),
        );
    }
    match ds_guard.insert_zone(zone) {
        Ok(_) => {}
        Err(err) => {
            error!(
                scope = logging::SCOPE_REST_API,
                zone_id = %new_id,
                error = %err,
                "Can't create the zone"
            );
            return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                .json(ErrorResponse::text("could not create the zone")));
        }
    }

    // Rebuild zone grid after creation
    let _ = ds_guard.rebuild_zone_grid();
    drop(ds_guard);

    return Ok(HttpResponse::Created().json(ZoneCreateResponse {
        zone_id: new_id,
        state: ChangeState::of(&data),
    }));
}

/// The body of the request to overwrite all zones
/// It does delete all existing zones and create new ones
#[derive(Debug, Deserialize, ToSchema)]
pub struct ZonesOverwriteAllRequest {
    /// The zones that should exist after this request, all of them: anything
    /// not in this list is deleted
    #[schema(example = json!([{"lane_number":53,"lane_direction":153,"pixel_points":[[230,200],[550,235],[512,40],[359,69]],"spatial_points":[[37.618908137083054,54.20564619851147],[37.61891517788172,54.20564502193819],[37.618927247822285,54.205668749493036],[37.61892020702362,54.2056701221611]],"color_rgb":[130,130,0],"virtual_line":{"geometry":[[365,177],[540,185]],"color_rgb":[210,65,80],"direction":"lrtb"}},{"lane_number":42,"lane_direction":142,"pixel_points":[[591,265],[835,265],[726,48],[555,58]],"spatial_points":[[37.618923808130916,54.205684902663165],[37.618887068935805,54.205689389059046],[37.618869252334406,54.205650113258066],[37.61890605402775,54.20564367215164]],"color_rgb":[130,0,130]}]))]
    pub data: Vec<ZoneCreateRequest>,
}

/// Respone on overwrite all zones request
#[derive(Debug, Serialize, ToSchema)]
pub struct ZonesOverwriteAllResponse {
    /// List of new zones identifiers
    #[schema(example = json!(["fad8a040-5979-47e9-9ebf-3a571f677f49", "dcd66eeb-545c-4f81-99f6-e94229f8008a"]))]
    pub zones_ids: Vec<String>,
    #[serde(flatten)]
    pub state: ChangeState,
}

#[utoipa::path(
    post,
    tag = "Zones mutations",
    path = "/api/mutations/replace_all",
    request_body = ZonesOverwriteAllRequest,
    responses(
        (status = 201, description = "All zones has been overwritten", body = ZonesOverwriteAllResponse),
        (status = 400, description = "Two zones sent under one id, or an empty one", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse)
    )
)]
/// The whole set of zones at once: whatever this request does not list stops
/// existing.
///
/// A zone sent with the `id` of one that is already there **replaces** it
/// rather than being merged into it. It keeps the name, so a client that
/// stored the id goes on using it and the configuration file goes on calling
/// the zone the same thing, but everything the running process had counted for
/// it - the statistics of the current period, the vehicles it has registered,
/// the crossings of its virtual line - starts over. A zone sent without an
/// `id` is a new zone and is given one; the answer lists the ids in the order
/// the zones were sent
pub async fn replace_all(
    data: web::Data<APIStorage>,
    _new_zones: web::Json<ZonesOverwriteAllRequest>,
) -> Result<HttpResponse, Error> {
    if _new_zones.data.len() == 0 {
        return Ok(HttpResponse::build(StatusCode::BAD_REQUEST)
            .json(ErrorResponse::text("No polygons".to_string())));
    }

    // Nothing is touched until the whole request is known to be good: two zones
    // sent under one name cannot both be kept, and finding that out halfway
    // through would leave the running zones in a state nobody asked for
    let mut refused = Vec::new();
    let mut named: HashSet<&str> = HashSet::new();
    for (index, new_zone) in _new_zones.data.iter().enumerate() {
        let Some(id) = new_zone.id.as_deref().map(str::trim) else {
            continue;
        };
        let field = format!("data.{index}.id");
        if id.is_empty() {
            refused.push(FieldError::new(field, "must not be empty"));
        } else if !named.insert(id) {
            refused.push(FieldError::new(
                field,
                format!("'{id}' names more than one zone in this request"),
            ));
        }
    }
    if !refused.is_empty() {
        return Ok(
            HttpResponse::build(StatusCode::BAD_REQUEST).json(ErrorResponse::fields(refused))
        );
    }

    // Mark data for clean
    let ds_guard = data
        .data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    let zones = ds_guard
        .zones
        .read()
        .expect("Spatial data is poisoned [RWLock]");
    let need_to_clean: Vec<String> = zones.iter().map(|poly| poly.0.clone()).collect();
    drop(zones);
    drop(ds_guard);

    // Add new data
    let mut response = vec![];
    for new_zone in _new_zones.data.iter() {
        let mut zone = Zone::default();
        match new_zone.pixel_points {
            Some(data) => {
                zone.update_pixel_map(data);
            }
            _ => {}
        }

        match new_zone.spatial_points {
            Some(data) => {
                zone.update_spatial_map(data);
            }
            _ => {}
        }

        match new_zone.lane_direction {
            Some(val) => {
                zone.set_road_lane_direction(val);
            }
            _ => {}
        }

        match new_zone.lane_number {
            Some(val) => {
                zone.set_road_lane_num(val);
            }
            _ => {}
        }

        match new_zone.color_rgb {
            Some(val) => {
                zone.set_color(val);
            }
            _ => {}
        }

        match &new_zone.virtual_line {
            Some(val) => {
                let dir =
                    VirtualLineDirection::from_str(val.direction.as_str()).unwrap_or_default();
                let mut new_line = VirtualLine::new_from(val.geometry, dir);
                if let Some(rgb) = val.color_rgb {
                    new_line.set_color_rgb(rgb[0], rgb[1], rgb[2]);
                } else {
                    let zone_color = zone.get_color();
                    new_line.set_color_rgb(zone_color[0], zone_color[1], zone_color[2]);
                };
                zone.set_virtual_line(new_line);
            }
            _ => {}
        }

        if let Some(id) = new_zone.id.as_deref().map(str::trim) {
            zone.set_id(id.to_string());
        }
        let new_id = zone.get_id().clone();

        let ds_guard = data
            .data_storage
            .read()
            .expect("DataStorage is poisoned [RWLock]");
        match ds_guard.insert_zone(zone) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    scope = logging::SCOPE_REST_API,
                    zone_id = %new_id,
                    error = %err,
                    "Can't create the zone while replacing them all"
                );
                return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                    .json(ErrorResponse::text("could not replace the zones")));
            }
        }
        drop(ds_guard);

        response.push(new_id);
    }

    // Clean data. A zone the caller sent back under the name it already had was
    // replaced in place just now, so cleaning by the old list would delete the
    // very zones this request came to keep
    let kept: HashSet<&String> = response.iter().collect();
    let ds_guard = data
        .data_storage
        .read()
        .expect("DataStorage is poisoned [RWLock]");
    for zone_id in need_to_clean.iter().filter(|id| !kept.contains(id)) {
        match ds_guard.delete_zone(zone_id) {
            Ok(_) => {}
            Err(err) => {
                error!(
                    scope = logging::SCOPE_REST_API,
                    zone_id = %zone_id,
                    error = %err,
                    "Can't delete a zone that is no longer there while replacing them all"
                );
                return Ok(HttpResponse::build(StatusCode::INTERNAL_SERVER_ERROR)
                    .json(ErrorResponse::text("could not replace the zones")));
            }
        }
    }

    // Rebuild zone grid once after all changes
    let _ = ds_guard.rebuild_zone_grid();
    drop(ds_guard);

    return Ok(HttpResponse::Created().json(ZonesOverwriteAllResponse {
        zones_ids: response,
        state: ChangeState::of(&data),
    }));
}
