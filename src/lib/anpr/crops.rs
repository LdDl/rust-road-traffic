use crate::lib::anpr::types::BoundingBox;
use crate::lib::cv::{RawFrame, Rect};

pub fn ocr_crop_bbox(frame: &RawFrame, bbox: &BoundingBox) -> BoundingBox {
    // Expand each side by 10%, with at least two pixels for small plates.
    let pad_x = bbox.width.div_ceil(10).max(2);
    let pad_y = bbox.height.div_ceil(10).max(2);
    let x = bbox.x.saturating_sub(pad_x).min(frame.width);
    let y = bbox.y.saturating_sub(pad_y).min(frame.height);
    let right = bbox
        .x
        .saturating_add(bbox.width)
        .saturating_add(pad_x)
        .min(frame.width);
    let bottom = bbox
        .y
        .saturating_add(bbox.height)
        .saturating_add(pad_y)
        .min(frame.height);
    BoundingBox {
        x,
        y,
        width: right.saturating_sub(x),
        height: bottom.saturating_sub(y),
    }
}

pub fn crop_frame(frame: &RawFrame, bbox: &BoundingBox) -> Option<RawFrame> {
    if bbox.width == 0
        || bbox.height == 0
        || bbox.x.checked_add(bbox.width)? > frame.width
        || bbox.y.checked_add(bbox.height)? > frame.height
    {
        return None;
    }
    let mut crop = RawFrame::new(bbox.width, bbox.height);
    let row_bytes = crop.step();
    for row in 0..bbox.height as usize {
        let source = (bbox.y as usize + row) * frame.step() + bbox.x as usize * 3;
        let target = row * row_bytes;
        crop.data[target..target + row_bytes]
            .copy_from_slice(&frame.data[source..source + row_bytes]);
    }
    Some(crop)
}

pub fn bbox_in_frame(bbox: Rect, crop: &BoundingBox) -> Option<BoundingBox> {
    if bbox.width <= 0 || bbox.height <= 0 {
        return None;
    }
    let left = (bbox.x as i64).clamp(0, crop.width as i64) as u32;
    let top = (bbox.y as i64).clamp(0, crop.height as i64) as u32;
    let right = (bbox.x as i64 + bbox.width as i64).clamp(0, crop.width as i64) as u32;
    let bottom = (bbox.y as i64 + bbox.height as i64).clamp(0, crop.height as i64) as u32;
    if right <= left || bottom <= top {
        return None;
    }
    Some(BoundingBox {
        x: crop.x + left,
        y: crop.y + top,
        width: right - left,
        height: bottom - top,
    })
}
