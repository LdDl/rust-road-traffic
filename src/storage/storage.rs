use std::collections::HashMap;

use crate::lib::tracking::BlobID;
use crate::lib::polygons::PolygonID;

pub struct BlobsPolygons {
    data: HashMap<BlobID, PolygonID>
}


impl BlobsPolygons {
    pub fn check_pair(blob_id: BlobID, polygon_id: PolygonID, state: bool) {

    }
}