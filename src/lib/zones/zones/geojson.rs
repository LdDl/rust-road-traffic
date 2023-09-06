use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Detection zones as GeoJSON feature collection
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ZonesFeatureCollection {
    /// Constant type of the GeoJSON feature collection
    #[serde(rename(serialize = "type"))]
    #[schema(example = "FeatureCollection")]
    pub typ: String,
    /// Set of the GeoJSON features
    pub features: Vec<ZoneFeature>
}

impl ZonesFeatureCollection {
    pub fn new() -> Self {
        return ZonesFeatureCollection {
            typ: "FeatureCollection".to_string(),
            features: vec![]
        }
    }
}

/// Detection zone as GeoJSON feature
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ZoneFeature {
    /// Constant type of the GeoJSON feature
    #[serde(rename(serialize = "type"))]
    #[schema(example = "Feature")]
    pub typ: String,
    /// Unique identifier of the GeoJSON feature
    #[schema(example = "a83c4c5c-7af0-4283-83f4-43ad4956269f")]
    pub id: String,
    /// Zone's properties
    pub properties: ZonePropertiesGeoJSON,
    /// Geometry of the zone
    pub geometry: GeoPolygon,
}

/// Parameters for the detection zone
#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ZonePropertiesGeoJSON {
    /// Corresponding road lane number
    #[schema(example = 2)]
    pub road_lane_num: u16,
    /// Corresponding road lane direction
    #[schema(example = 1)]
    pub road_lane_direction: u8,
    /// Corresponding zone's coordinates for the video frames
    #[schema(example = json!([[51,266],[281,264],[334,80],[179,68]]))]
    pub coordinates: Vec<Vec<i32>>,
    /// Color to visually distinct zones
    #[schema(example = json!([255, 0, 0]))]
    pub color_rgb: [i16; 3],
    /// Information about virtual line (optional)
    pub virtual_line: Option<VirtualLineFeature>
}

/// Information about virtual line
#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct VirtualLineFeature {
    /// Geometry: two poins
    #[schema(example = json!([[100, 236], [270, 234]]))]
    pub geometry: [[i32; 2]; 2],
    /// Corresponding color
    #[schema(example = json!([255, 0, 0]))]
    pub color_rgb: [i16; 3],
    /// Direction. Possible values:
    /// 0 - left->right, top->bottom
    /// 1 - right->left, bottom->top
    #[schema(example = 1)]
    pub direction: u8,
}

/// Polygon in GeoJSON specification
#[derive(Serialize, Deserialize, Debug, Default, Clone, ToSchema)]
pub struct GeoPolygon {
    /// Constant value for specific geometry type
    #[serde(rename(serialize = "type", deserialize = "type"))]
    #[schema(example = "Polygon")]
    pub geometry_type: String,
    /// Coordinates for the given geometry (WGS84, EPSG 4326, [longitude, latitude])
    #[serde(rename(serialize = "coordinates", deserialize = "coordinates"))]
    #[schema(example = json!([[[37.61896,54.20568],[37.618927,54.205685],[37.618908,54.205647],[37.618946,54.20564],[37.61896,54.20568]]]))]
    pub coordinates: Vec<Vec<Vec<f32>>>,
}
