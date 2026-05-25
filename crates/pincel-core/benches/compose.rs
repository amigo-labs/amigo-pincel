//! `compose()` performance baselines.
//!
//! Runs the same compose scenarios that the M12 work targets — the spec exit
//! criterion calls for a 256×256 sprite at zoom 32 holding 60 fps, and these
//! benches pin the numbers we're moving against. The scenarios cover the
//! shape of the live editor's hot paths:
//!
//! * `compose_256_single_layer_full` — the bread-and-butter case: one image
//!   layer covering the canvas at 1:1 zoom.
//! * `compose_256_four_layers_full` — multi-layer source-over blend (M11.x
//!   sprites grow layers quickly).
//! * `compose_256_dirty_hint_4x4` — pre-M12.2 this matches the full path
//!   because `dirty_hint` is ignored; the bench's existence pins the
//!   baseline so the M12.2 numbers can quote a precise speedup.
//! * `compose_64_tilemap_full` — tilemap composite path (4×4 grid of 16×16
//!   tiles).
//! * `compose_zoom_32_upscale_8x8_to_256x256` — exercises the nearest-
//!   neighbor upscale path. Composes an 8×8 viewport at zoom 32 (256×256
//!   output) so the bench name matches what the function actually does.
//!   The UI today applies the spec-cited "zoom 32" via CSS and always
//!   calls `compose()` with `zoom = 1`, so this is a probe on the
//!   upscale cost in case we ever route zoom through the wasm boundary.

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use pincel_core::{
    Cel, CelData, CelMap, ColorMode, ComposeRequest, Frame, FrameIndex, Layer, LayerId,
    PixelBuffer, Rect, Sprite, TileImage, TileRef, Tileset, TilesetId, compose,
};

fn solid_rgba(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
    let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
    for px in buf.data.chunks_exact_mut(4) {
        px.copy_from_slice(&rgba);
    }
    buf
}

/// Build a 256×256 single-layer sprite. Cel buffer is filled with an opaque
/// solid color so the blend loop touches every output pixel.
fn sprite_256_single_layer() -> (Sprite, CelMap) {
    let sprite = Sprite::builder(256, 256)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_frame(Frame::default())
        .build()
        .expect("256 single-layer builder");
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        solid_rgba(256, 256, [80, 120, 200, 255]),
    ));
    (sprite, cels)
}

/// Build a 256×256 sprite with four overlapping image layers. Mixed
/// alphas force the blend path (not just the fast opaque copy).
fn sprite_256_four_layers() -> (Sprite, CelMap) {
    let mut builder = Sprite::builder(256, 256);
    for i in 0..4 {
        builder = builder.add_layer(Layer::image(LayerId::new(i), "layer"));
    }
    let sprite = builder
        .add_frame(Frame::default())
        .build()
        .expect("256 four-layer builder");
    let mut cels = CelMap::new();
    let colors = [
        [255, 0, 0, 255],
        [0, 255, 0, 128],
        [0, 0, 255, 128],
        [255, 255, 0, 64],
    ];
    for (i, c) in colors.iter().enumerate() {
        cels.insert(Cel::image(
            LayerId::new(i as u32),
            FrameIndex::new(0),
            solid_rgba(256, 256, *c),
        ));
    }
    (sprite, cels)
}

/// Build a 64×64 sprite with a 4×4 grid of 16×16 tiles. Tile 0 is empty,
/// tile 1 is solid — half the grid cells reference tile 1 in a
/// checkerboard pattern so the tilemap composite path is exercised.
fn sprite_64_tilemap() -> (Sprite, CelMap) {
    let mut tileset = Tileset::new(TilesetId::new(0), "tiles", (16, 16));
    tileset.tiles.push(TileImage {
        pixels: PixelBuffer::empty(16, 16, ColorMode::Rgba),
    });
    tileset.tiles.push(TileImage {
        pixels: solid_rgba(16, 16, [200, 100, 50, 255]),
    });

    let sprite = Sprite::builder(64, 64)
        .add_layer(Layer::tilemap(LayerId::new(0), "tm", TilesetId::new(0)))
        .add_frame(Frame::default())
        .add_tileset(tileset)
        .build()
        .expect("64 tilemap builder");
    let mut tiles = Vec::with_capacity(16);
    for j in 0..4 {
        for i in 0..4 {
            tiles.push(if (i + j) % 2 == 0 {
                TileRef::new(1)
            } else {
                TileRef::EMPTY
            });
        }
    }
    let mut cels = CelMap::new();
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Tilemap {
            grid_w: 4,
            grid_h: 4,
            tiles,
        },
    });
    (sprite, cels)
}

fn bench_compose(c: &mut Criterion) {
    let mut group = c.benchmark_group("compose");

    let (sprite, cels) = sprite_256_single_layer();
    group.bench_function("compose_256_single_layer_full", |b| {
        let req = ComposeRequest::full(FrameIndex::new(0), 256, 256);
        b.iter_batched(
            || (sprite.clone(), cels.clone()),
            |(s, c)| {
                let _ = black_box(compose(&s, &c, &req).expect("compose"));
            },
            BatchSize::SmallInput,
        );
    });

    let (sprite, cels) = sprite_256_four_layers();
    group.bench_function("compose_256_four_layers_full", |b| {
        let req = ComposeRequest::full(FrameIndex::new(0), 256, 256);
        b.iter_batched(
            || (sprite.clone(), cels.clone()),
            |(s, c)| {
                let _ = black_box(compose(&s, &c, &req).expect("compose"));
            },
            BatchSize::SmallInput,
        );
    });

    let (sprite, cels) = sprite_256_single_layer();
    group.bench_function("compose_256_dirty_hint_4x4", |b| {
        let mut req = ComposeRequest::full(FrameIndex::new(0), 256, 256);
        req.dirty_hint = Some(Rect::new(120, 120, 4, 4));
        b.iter_batched(
            || (sprite.clone(), cels.clone()),
            |(s, c)| {
                let _ = black_box(compose(&s, &c, &req).expect("compose"));
            },
            BatchSize::SmallInput,
        );
    });

    let (sprite, cels) = sprite_64_tilemap();
    group.bench_function("compose_64_tilemap_full", |b| {
        let req = ComposeRequest::full(FrameIndex::new(0), 64, 64);
        b.iter_batched(
            || (sprite.clone(), cels.clone()),
            |(s, c)| {
                let _ = black_box(compose(&s, &c, &req).expect("compose"));
            },
            BatchSize::SmallInput,
        );
    });

    // 8×8 viewport at zoom 32 → 256×256 output. The viewport is small on
    // purpose: 256×256 at zoom 32 would be an 8192×8192 (256 MB) buffer,
    // which dwarfs any plausible UI workload. This bench probes the
    // `upscale_nearest()` path with the spec-cited zoom factor at a
    // realistic output size.
    let (sprite, cels) = sprite_256_single_layer();
    group.bench_function("compose_zoom_32_upscale_8x8_to_256x256", |b| {
        let mut req = ComposeRequest::full(FrameIndex::new(0), 256, 256);
        req.viewport = Rect::new(0, 0, 8, 8);
        req.zoom = 32;
        b.iter_batched(
            || (sprite.clone(), cels.clone()),
            |(s, c)| {
                let _ = black_box(compose(&s, &c, &req).expect("compose"));
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_compose);
criterion_main!(benches);
