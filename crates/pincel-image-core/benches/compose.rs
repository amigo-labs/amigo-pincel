//! Image-mode performance benchmarks (docs/specs/performance-benchmarks.md,
//! AC-01; budgets in docs/specs/pincel.md §4.5).
//!
//! Every `performance-*` change states before → after against these
//! medians. The scenarios mirror the live editor's hot paths:
//!
//! * `image_compose/full_<size>_<n>layers` — [`compose`] of the whole canvas,
//!   1024², 2048² and 4096² with 2, 5 and 10 layers.
//! * `image_region/zoom1_4k_10layers` — the viewport renderer at 1:1: a
//!   1920×1080 viewport out of a 3840×2160, 10-layer document.
//! * `image_region/fit_4k_10layers` — the viewport renderer at fit zoom: the
//!   whole 3840×2160 canvas composed and scaled bilinearly to 1920×1080,
//!   as `pincel-image-wasm`'s `composite_region` does.
//! * `image_stroke/brush_100_dabs_4k_10layers` — 100 single-dab Pencil strokes
//!   (one per pointer event), each rasterized and applied to the top
//!   layer of a 3840×2160, 10-layer document.
//! * `image_effect/gaussian_r10_4k` — a radius-10 Gaussian blur of one
//!   3840×2160 layer, including the RGBA8 hand-off to `pincel-effects`.
//! * `image_cache/brush_100_dabs_fit_4k_10layers` — the same 100 dabs through
//!   a `CommandBus`, each followed by updating the document's [`RenderCache`]
//!   and presenting the dab's changed region at fit zoom (½), as the editor
//!   does (docs/specs/performance-image-cache.md, AC-02).
//! * `image_cache/fit_viewport_4k_10layers` — presenting the whole 3840×2160
//!   canvas into the 1920×1080 viewport from a current cache (pan / zoom
//!   without edits).
//!
//! * `image_live/ellipse_move_1024_4k_5layers` — one edit that moves a live
//!   1024 × 1024 filled, anti-aliased ellipse with a 4 px outline by 1 px on
//!   a 4096 × 4096, 5-layer document, plus the region compose of what
//!   changed (docs/specs/image-live-objects.md AC-17).
//! * `image_shapes/curve_64pts_w20_dashdot_arrows_4k` — drawing a 64-point
//!   Line / Curve (stroke width 20, Dash-Dot, anti-aliased, Arrow caps)
//!   across a 3840×2160 layer (docs/specs/image-shapes-curves.md AC-21).
//! * `image_live/gradient_end_move_2k_5layers` — one edit that moves the end
//!   point of a live linear gradient by 1 px on a 2048 × 2048, 5-layer
//!   document, plus the region compose of what changed.
//!
//! * `image_export/over_background_4k` — [`compose_over`] of one 3840×2160
//!   layer with per-pixel varied color and alpha onto white (Flatten Image):
//!   a one-layer compose plus the linear-light over-background blend.
//! * `image_export/jpeg_4k` — JPEG export of the same 3840×2160 buffer, which
//!   flattens it onto white before encoding.
//! * `selection/feather_r20_4k`, `selection/expand_r20_4k` — Feather and
//!   Expand by 20 px of an anti-aliased elliptical selection on a
//!   3840×2160 canvas (spec §8.4).
//! * `image_open/png_4k` — decoding a 3840×2160 RGBA PNG (per-pixel varied
//!   color and alpha, written by the crate's own encoder) into an
//!   [`ImageBuffer`], as opening a file does.
//!
//! The 4K scenarios take hundreds of milliseconds to seconds per
//! iteration natively, so their groups run the minimum 10 samples.

use std::hint::black_box;
use std::time::Duration;

use criterion::{
    BatchSize, BenchmarkGroup, Criterion, criterion_group, criterion_main, measurement::WallTime,
};
use pincel_effects::{Effect, EffectImage, blur::GaussianBlur};
use pincel_image_core::command::Damage;
use pincel_image_core::{
    BoxShape, Brush, Color, Command, CommandBus, Curve, CurveLine, DashPattern, Document, Gradient,
    ImageBuffer, Interpolation, Layer, LineCap, LiveEdit, LiveGradient, LiveParams, LiveShape,
    LiveShapeKind, Pencil, Point, Rect, RenderCache, SelectionMask, ShapeMode, ShapeStyle, codec,
    compose, compose_over, compose_region, scale,
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
    let mut group = c.benchmark_group("image_compose");
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
    let mut group = c.benchmark_group("image_region");
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
    let mut group = c.benchmark_group("image_stroke");
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

/// The bounding rect of `damage`, or the whole `doc` canvas.
fn damaged_rect(damage: &Damage, doc: &Document) -> Rect {
    let whole = Rect::new(0, 0, doc.canvas.width(), doc.canvas.height());
    match damage {
        Damage::Rects(rects) => rects.iter().copied().reduce(|a, b| {
            let (x0, y0) = (a.x.min(b.x), a.y.min(b.y));
            let (x1, y1) = (a.right().max(b.right()), a.bottom().max(b.bottom()));
            Rect::new(x0, y0, (x1 - x0) as u32, (y1 - y0) as u32)
        }),
        _ => None,
    }
    .unwrap_or(whole)
}

fn bench_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_cache");
    heavy(&mut group);
    let mut bus = CommandBus::new(document(UHD.0, UHD.1, 10));
    let top = bus.document.layer_count() - 1;
    let pencil = Pencil::new(Brush::new(20, Color::rgba(20, 40, 200, 255), 1.0));
    let dabs: Vec<Point> = (0..100)
        .map(|i| Point::new(200.0 + i as f32 * 30.0, 200.0 + i as f32 * 15.0))
        .collect();
    // The document is open and shown at fit zoom before the stroke starts.
    let mut cache = RenderCache::new(&bus.document);
    let whole = Rect::new(0, 0, UHD.0, UHD.1);
    let opened = bus.take_damage();
    cache.update(&bus.document, opened);
    black_box(cache.region(&bus.document, whole, VIEWPORT.0, VIEWPORT.1));

    group.bench_function("brush_100_dabs_fit_4k_10layers", |b| {
        b.iter(|| {
            for p in &dabs {
                let cmd = pencil
                    .stroke(top, std::slice::from_ref(p), &bus.document)
                    .expect("the dab lies on the canvas");
                bus.apply(Box::new(cmd)).expect("stroke applies");
                let damage = bus.take_damage();
                let rect = damaged_rect(&damage, &bus.document);
                cache.update(&bus.document, damage);
                let (ow, oh) = (rect.w.div_ceil(2), rect.h.div_ceil(2));
                black_box(cache.region(&bus.document, rect, ow, oh));
            }
        });
    });
    group.bench_function("fit_viewport_4k_10layers", |b| {
        b.iter(|| black_box(cache.region(&bus.document, whole, VIEWPORT.0, VIEWPORT.1)));
    });
    group.finish();
}

fn bench_gaussian_blur(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_effect");
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

/// Moves the live object's points by `dx` (to `point` 0, 1 or both) as one
/// gesture edit, then composes the damaged region.
fn live_edit(bus: &mut CommandBus, points: [Point; 2], dx: f32, which: Option<usize>) {
    let mut moved = points;
    for (i, p) in moved.iter_mut().enumerate() {
        if which.is_none_or(|w| w == i) {
            p.x += dx;
        }
    }
    let edit = LiveEdit {
        points: Some(moved),
        ..LiveEdit::default()
    };
    bus.edit_live(&edit, Some(1)).expect("the edit applies");
    let damage = bus.take_damage();
    let rect = damaged_rect(&damage, &bus.document);
    black_box(compose_region(bus.document.layers(), rect));
}

fn bench_live(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_live");
    heavy(&mut group);

    let mut bus = CommandBus::new(document(4096, 4096, 5));
    let top = bus.document.layer_count() - 1;
    let ellipse = [Point::new(1000.0, 1000.0), Point::new(2024.0, 2024.0)];
    let shape = LiveParams::Shape(LiveShape {
        kind: LiveShapeKind::Box(BoxShape::Ellipse),
        a: ellipse[0],
        b: ellipse[1],
        style: ShapeStyle {
            mode: ShapeMode::FillAndOutline,
            stroke_width: 4.0,
            anti_alias: true,
            ..ShapeStyle::default()
        },
        corner_radius: 0.0,
        sides: 5,
        star_points: 5,
        inner_ratio: 0.38,
        control: Vec::new(),
        start_cap: LineCap::Round,
        end_cap: LineCap::Round,
    });
    bus.begin_live(top, shape).expect("the ellipse begins");
    bus.take_damage();
    let mut step = 1.0;
    group.bench_function("ellipse_move_1024_4k_5layers", |b| {
        b.iter(|| {
            live_edit(&mut bus, ellipse, step, None);
            step = 1.0 - step;
        });
    });

    let mut bus = CommandBus::new(document(2048, 2048, 5));
    let top = bus.document.layer_count() - 1;
    let drag = [Point::new(100.0, 300.0), Point::new(1900.0, 1700.0)];
    let gradient = LiveParams::Gradient(LiveGradient {
        gradient: Gradient {
            from: Color::rgba(200, 30, 30, 255),
            to: Color::rgba(30, 30, 200, 255),
            ..Gradient::default()
        },
        a: drag[0],
        b: drag[1],
    });
    bus.begin_live(top, gradient).expect("the gradient begins");
    bus.take_damage();
    let mut step = 1.0;
    group.bench_function("gradient_end_move_2k_5layers", |b| {
        b.iter(|| {
            live_edit(&mut bus, drag, step, Some(1));
            step = 1.0 - step;
        });
    });
    group.finish();
}

/// A 64-point curve zig-zagging across a 4K layer, drawn as one command.
fn bench_curve(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_shapes");
    heavy(&mut group);
    let doc = document(UHD.0, UHD.1, 1);
    let points: Vec<Point> = (0..64)
        .map(|i| {
            let x = 40.0 + (UHD.0 as f32 - 80.0) * i as f32 / 63.0;
            let y = if i % 2 == 0 {
                300.0
            } else {
                UHD.1 as f32 - 300.0
            };
            Point::new(x, y)
        })
        .collect();
    let line = CurveLine::new(
        Curve {
            points,
            start_cap: LineCap::Arrow,
            end_cap: LineCap::Arrow,
        },
        ShapeStyle {
            stroke_width: 20.0,
            stroke_color: Color::rgba(200, 30, 30, 255),
            anti_alias: true,
            dash: DashPattern::DashDot,
            ..ShapeStyle::default()
        },
    );
    group.bench_function("curve_64pts_w20_dashdot_arrows_4k", |b| {
        b.iter(|| black_box(line.draw(0, black_box(&doc))));
    });
    group.finish();
}

/// A `w × h` buffer whose color and alpha vary per pixel, so every alpha
/// level (not only opaque or clear pixels) runs through the blend.
fn varied(w: u32, h: u32) -> ImageBuffer {
    let mut data = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&[
                (x % 256) as u8,
                (y % 256) as u8,
                ((x + y) % 256) as u8,
                ((x * 7 + y * 13) % 256) as u8,
            ]);
        }
    }
    ImageBuffer::from_raw(w, h, data).expect("varied buffer matches its size")
}

fn bench_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_export");
    heavy(&mut group);
    let pixels = varied(UHD.0, UHD.1);
    let layers = [Layer::from_pixels("Varied", pixels.clone())];
    group.bench_function("over_background_4k", |b| {
        b.iter(|| black_box(compose_over(black_box(&layers), Color::WHITE)));
    });
    group.bench_function("jpeg_4k", |b| {
        b.iter(|| {
            let mut out = Vec::new();
            codec::encode_jpeg(black_box(&pixels), &mut out, 90).expect("JPEG encodes");
            black_box(out)
        });
    });
    group.finish();
}

fn bench_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("selection");
    heavy(&mut group);
    let mask = SelectionMask::ellipse_aa(UHD.0, UHD.1, Rect::new(400, 300, 3000, 1500));
    group.bench_function("feather_r20_4k", |b| {
        b.iter_batched(
            || mask.clone(),
            |mut m| {
                m.feather(20);
                m
            },
            BatchSize::LargeInput,
        );
    });
    group.bench_function("expand_r20_4k", |b| {
        b.iter_batched(
            || mask.clone(),
            |mut m| {
                m.expand(20);
                m
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("image_open");
    heavy(&mut group);
    let mut png = Vec::new();
    codec::encode_png(&varied(UHD.0, UHD.1), &mut png, 6).expect("PNG encodes");
    group.bench_function("png_4k", |b| {
        b.iter(|| black_box(codec::decode(black_box(&png)).expect("PNG decodes")));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_full_compose,
    bench_region_compose,
    bench_brush_stroke,
    bench_cache,
    bench_gaussian_blur,
    bench_live,
    bench_curve,
    bench_export,
    bench_selection,
    bench_open
);
criterion_main!(benches);
