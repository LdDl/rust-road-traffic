use std::collections::HashMap;

use crate::lib::tracking::BlobID;
use crate::lib::polygons::PolygonID;

pub struct BlobsPolygons {
    data: HashMap<BlobID, PolygonID>
}