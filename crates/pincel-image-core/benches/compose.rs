//! Image-mode performance benchmarks (docs/specs/performance-benchmarks.md,
//! AC-01; budgets in docs/specs/pincel.md §4.5).
//!
//! Every `performance-*` change states before → after against these
//! medians. The scenarios mirror the live editor's hot paths:
//!
//! * `compose/full_<size>_<n>layers` — [`compose`] of the whole canvas,
//!   1024², 2048² and 4096² with 2, 5 and 10 layers.
//! * `region/zoom1_4k_10layers` — the viewport renderer at 1:1: a
//!   1920×1080 viewport out of a 3840×2160, 10-layer document.
//! * `region/fit_4k_10layers` — the viewport renderer at fit zoom: the
//!   whole 3840×2160 canvas composed and scaled bilinearly to 1920×1080,
//!   as `pincel-image-wasm`'s `composite_region` does.
//! * `stroke/brush_100_dabs_4k_10layers` — 100 single-dab Pencil strokes
//!   (one per pointer event), each rasterized and applied to the top
//!   layer of a 3840×2160, 10-layer document.
//! * `effect/gaussian_r10_4k` — a radius-10 Gaussian blur of one
//!   3840×2160 layer, including the RGBA8 hand-off to `pincel-effects`.
//!
//! The 4K scenarios take hundreds of milliseconds to seconds per
//! iteration natively, so their groups run the minimum 10 samples.

use std::hint::black_box;
use std::time::Duration;

use criterion::{
    BenchmarkGroup, Criterion, criterion_group, criterion_main, measurement::WallTime,
};
use pincel_effects::{Effect, EffectImage, blur::GaussianBlur};
use pincel_image_core::{
    Brush, Color, Command, Document, ImageBuffer, Interpolation, Layer, Pencil, Point, Rect,
    compose, compose_region, scale,
};

/// 4K as the budget table defines it (docs/specs/pincel.md §4.5).
const UHD: (u32, u32) = (3840, 2160);
/// The viewport the region benches render into: a full-HD window.
const VIEWPORT: (u32, u32) = (1920, 1080);

/// A `w × h` buffer filled with one straight-alpha color.
fn solid(w: u32, h: u32, color: Color) -> ImageBuffer {
    let px = [color.r, color.g, color.b, color.a];
    let data = px.repeat((w * h) as usize);
    ImageBuffer::from_raw(w, h, data).expect("solid buffer matches its size")
}

/// `n` full-canvas layers: an opaque bottom and semi-transparent layers
/// above, so the blend path (not an opaque copy) runs for every pixel.
fn layer_stack(w: u32, h: u32, n: usize) -> Vec<Layer> {
    (0..n)
        .map(|i| {
            let alpha = if i == 0 { 255 } else { 128 };
            let shade = (i * 23 % 256) as u8;
            Layer::from_pixels(
                format!("Layer {}", i + 1),
                solid(w, h, Color::rgba(shade, 255 - shade, 128, alpha)),
            )
        })
        .collect()
}

/// A document holding `n` layers of `w × h`, the top one active.
fn document(w: u32, h: u32, n: usize) -> Document {
    let mut layers = layer_stack(w, h, n).into_iter();
    let bottom = layers.next().expect("at least one layer");
    let mut doc = Document::from_pixels(bottom.pixels).expect("valid canvas size");
    for (i, layer) in layers.enumerate() {
        doc.insert_layer(i + 1, layer)
            .expect("below the layer limit");
    }
    doc
}

/// Configures a group whose iterations take a large fraction of a second.
fn heavy(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(10));
}

fn bench_full_compose(c: &mut Criterion) {
    let mut group = c.benchmark_group("compose");
    heavy(&mut group);
    for size in [1024, 2048, 4096] {
        for n in [2, 5, 10] {
            let layers = layer_stack(size, size, n);
            group.bench_function(format!("full_{size}_{n}layers"), |b| {
                b.iter(|| black_box(compose(black_box(&layers))));
            });
        }
    }
    group.finish();
}

fn bench_region_compose(c: &mut Criterion) {
    let mut group = c.benchmark_group("region");
    heavy(&mut group);
    let layers = layer_stack(UHD.0, UHD.1, 10);
    let (vw, vh) = VIEWPORT;

    // Zoom 1: a viewport-sized rect in the middle of the canvas.
    let centered = Rect::new(((UHD.0 - vw) / 2) as i32, ((UHD.1 - vh) / 2) as i32, vw, vh);
    group.bench_function("zoom1_4k_10layers", |b| {
        b.iter(|| black_box(compose_region(&layers, black_box(centered))));
    });

    // Fit zoom: the whole canvas, downscaled into the viewport.
    let whole = Rect::new(0, 0, UHD.0, UHD.1);
    group.bench_function("fit_4k_10layers", |b| {
        b.iter(|| {
            let region = compose_region(&layers, black_box(whole));
            black_box(scale(&region, vw, vh, Interpolation::Bilinear))
        });
    });
    group.finish();
}

fn bench_brush_stroke(c: &mut Criterion) {
    let mut group = c.benchmark_group("stroke");
    heavy(&mut group);
    let mut doc = document(UHD.0, UHD.1, 10);
    let top = doc.layer_count() - 1;
    let pencil = Pencil::new(Brush::new(20, Color::rgba(20, 40, 200, 255), 1.0));
    // 100 dabs along a diagonal, 30 px apart — one per pointer event.
    let dabs: Vec<Point> = (0..100)
        .map(|i| Point::new(200.0 + i as f32 * 30.0, 200.0 + i as f32 * 15.0))
        .collect();
    group.bench_function("brush_100_dabs_4k_10layers", |b| {
        b.iter(|| {
            for p in &dabs {
                let mut cmd = pencil
                    .stroke(top, std::slice::from_ref(p), &doc)
                    .expect("the dab lies on the canvas");
                cmd.apply(&mut doc).expect("stroke applies");
            }
        });
    });
    group.finish();
}

fn bench_gaussian_blur(c: &mut Criterion) {
    let mut group = c.benchmark_group("effect");
    heavy(&mut group);
    let layer = &layer_stack(UHD.0, UHD.1, 1)[0];
    let blur = GaussianBlur::new(10.0);
    group.bench_function("gaussian_r10_4k", |b| {
        b.iter(|| {
            let src = EffectImage::from_rgba8(UHD.0, UHD.1, layer.pixels.data().to_vec())
                .expect("layer bytes match its size");
            black_box(blur.apply(&src))
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_full_compose,
    bench_region_compose,
    bench_brush_stroke,
    bench_gaussian_blur
);
criterion_main!(benches);
