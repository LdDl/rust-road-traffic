use crate::lib::ocr_fusion::types::{Group, NormalizedBox};

pub fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0), |(sum, count), v| (sum + v, count + 1));
    sum / count as f64
}

pub fn median(values: impl Iterator<Item = f64>) -> f64 {
    let mut values: Vec<_> = values.collect();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

pub fn representative(group: &Group) -> NormalizedBox {
    NormalizedBox {
        x: median(group.iter().map(|s| s.bbox.x)),
        y: median(group.iter().map(|s| s.bbox.y)),
        width: median(group.iter().map(|s| s.bbox.width)),
        height: median(group.iter().map(|s| s.bbox.height)),
    }
}
