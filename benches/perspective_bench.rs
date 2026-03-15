use criterion::{black_box, criterion_group, criterion_main, Criterion};

use rust_road_traffic::lib::spatial::{
    PerspectiveTransform,
    compute_perspective_matrix,
    lonlat_to_meters,
};

fn setup_test_data() -> ([(f32, f32); 4], [(f32, f32); 4]) {
    let src = [(554.0_f32, 592.0), (959.0, 664.0), (1098.0, 360.0), (998.0, 359.0)];
    let dst_wgs84 = [
        (37.353610_f32, 55.853085),
        (37.353559, 55.853081),
        (37.353564, 55.852918),
        (37.353618, 55.852930),
    ];
    let dst_meters: [(f32, f32); 4] = core::array::from_fn(|i| {
        lonlat_to_meters(dst_wgs84[i].0, dst_wgs84[i].1)
    });
    (src, dst_meters)
}

fn bench_create_transform(c: &mut Criterion) {
    let (src, dst) = setup_test_data();

    c.bench_function("create_transform", |b| {
        b.iter(|| {
            let mat = compute_perspective_matrix(black_box(&src), black_box(&dst)).unwrap();
            black_box(mat);
        })
    });
}

fn bench_transform_point(c: &mut Criterion) {
    let (src, dst) = setup_test_data();
    let transform = PerspectiveTransform::new(&src, &dst).unwrap();

    c.bench_function("transform_point", |b| {
        b.iter(|| {
            black_box(transform.transform(black_box(800.0), black_box(500.0)))
        })
    });
}

criterion_group!(benches, bench_create_transform, bench_transform_point);
criterion_main!(benches);
