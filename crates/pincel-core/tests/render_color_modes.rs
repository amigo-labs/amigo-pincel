//! `compose()` on Indexed and Grayscale sprites — feature spec
//! `docs/specs/indexed-grayscale-compose.md`, design `docs/specs/pincel.md`
//! §4.2. Expected values are derived by hand from the format rules (palette
//! lookup, transparent index, background flag, (V, V, V, A)), never from the
//! renderer itself.

use pincel_core::{
    AtlasOptions, BlendMode, Bus, Cel, CelData, CelMap, ColorMode, ComposeRequest, Frame,
    FrameIndex, Layer, LayerFilter, LayerId, OnionSkin, Palette, PaletteEntry, PixelBuffer, Rect,
    RenderError, Rgba, SetPaletteColor, Sprite, TileImage, TileRef, Tileset, TilesetId, compose,
    export_atlas_png, export_frame_png,
};

/// Aseprite layer flag `8`: the background layer.
const BACKGROUND_FLAG: u16 = 0x8;

const INDEXED_T0: ColorMode = ColorMode::Indexed {
    transparent_index: 0,
};

fn palette(colors: &[[u8; 4]]) -> Palette {
    Palette::from_entries(
        colors
            .iter()
            .map(|c| PaletteEntry::new(Rgba::new(c[0], c[1], c[2], c[3])))
            .collect(),
    )
}

fn buffer(w: u32, h: u32, mode: ColorMode, data: Vec<u8>) -> PixelBuffer {
    let mut buf = PixelBuffer::empty(w, h, mode);
    assert_eq!(buf.data.len(), data.len(), "test buffer size");
    buf.data = data;
    buf
}

fn sprite_with(
    w: u32,
    h: u32,
    mode: ColorMode,
    pal: Palette,
    layers: Vec<Layer>,
    frames: u32,
) -> Sprite {
    let mut b = Sprite::builder(w, h).color_mode(mode).palette(pal);
    for layer in layers {
        b = b.add_layer(layer);
    }
    for _ in 0..frames {
        b = b.add_frame(Frame::new(100));
    }
    b.build().expect("test sprite builds")
}

fn compose_full(sprite: &Sprite, cels: &CelMap, frame: u32) -> Result<Vec<u8>, RenderError> {
    let mut out = Vec::new();
    compose(
        sprite,
        cels,
        &ComposeRequest::full(FrameIndex::new(frame), sprite.width, sprite.height),
        &mut out,
    )?;
    Ok(out)
}

fn mul_u8(a: u8, b: u8) -> u8 {
    ((u16::from(a) * u16::from(b) + 127) / 255) as u8
}

#[test]
fn compose_indexed_renders_palette_entry_with_its_alpha_under_opacities() {
    // AC-01: index 1 → entry (200, 100, 50, 128); layer opacity 128 and
    // cel opacity 128 multiply on top of the entry's own alpha:
    // mul(128, 128) = 64, then mul(128, 64) = 32 and mul(255, 64) = 64.
    let pal = palette(&[[0, 0, 0, 0], [200, 100, 50, 128], [10, 20, 30, 255]]);
    let mut layer = Layer::image(LayerId::new(0), "art");
    layer.opacity = 128;
    let sprite = sprite_with(2, 1, INDEXED_T0, pal, vec![layer], 1);
    let mut cel = Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        buffer(2, 1, INDEXED_T0, vec![1, 2]),
    );
    cel.opacity = 128;
    let mut cels = CelMap::new();
    cels.insert(cel);
    let out = compose_full(&sprite, &cels, 0).expect("indexed composes");
    assert_eq!(&out[0..4], &[200, 100, 50, mul_u8(128, mul_u8(128, 128))]);
    assert_eq!(&out[0..4], &[200, 100, 50, 32]);
    assert_eq!(&out[4..8], &[10, 20, 30, 64]);

    // A non-Normal blend mode: Multiply of an opaque entry over an opaque
    // white layer gives the entry's colour itself.
    let pal = palette(&[[0, 0, 0, 0], [255, 255, 255, 255], [100, 150, 200, 255]]);
    let bg = Layer::image(LayerId::new(0), "bg");
    let mut top = Layer::image(LayerId::new(1), "top");
    top.blend_mode = BlendMode::Multiply;
    let sprite = sprite_with(1, 1, INDEXED_T0, pal, vec![bg, top], 1);
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        buffer(1, 1, INDEXED_T0, vec![1]),
    ));
    cels.insert(Cel::image(
        LayerId::new(1),
        FrameIndex::new(0),
        buffer(1, 1, INDEXED_T0, vec![2]),
    ));
    let out = compose_full(&sprite, &cels, 0).expect("indexed composes");
    assert_eq!(out, vec![100, 150, 200, 255]);
}

#[test]
fn compose_indexed_transparent_index_on_normal_layer_contributes_nothing() {
    // AC-02: index 3 is transparent; its palette entry is opaque green, yet
    // it shows nothing over the red layer below. Also with a transparent
    // index outside the palette (edge case).
    for transparent_index in [3u8, 255] {
        let mode = ColorMode::Indexed { transparent_index };
        let pal = palette(&[
            [0, 0, 0, 0],
            [255, 0, 0, 255],
            [0, 0, 0, 0],
            [0, 255, 0, 255],
        ]);
        let bottom = Layer::image(LayerId::new(0), "bottom");
        let top = Layer::image(LayerId::new(1), "top");
        let sprite = sprite_with(1, 1, mode, pal, vec![bottom, top], 1);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            buffer(1, 1, mode, vec![1]),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            buffer(1, 1, mode, vec![transparent_index]),
        ));
        let out = compose_full(&sprite, &cels, 0).expect("indexed composes");
        assert_eq!(
            out,
            vec![255, 0, 0, 255],
            "transparent index {transparent_index}"
        );
    }
}

#[test]
fn compose_indexed_transparent_index_on_background_layer_renders_entry() {
    // AC-03: on a background layer index 0 renders as entry 0.
    let pal = palette(&[[9, 8, 7, 255], [1, 2, 3, 255]]);
    let mut bg = Layer::image(LayerId::new(0), "Background");
    bg.extra_flags = BACKGROUND_FLAG;
    let sprite = sprite_with(2, 1, INDEXED_T0, pal, vec![bg], 1);
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        buffer(2, 1, INDEXED_T0, vec![0, 1]),
    ));
    let out = compose_full(&sprite, &cels, 0).expect("indexed composes");
    assert_eq!(out, vec![9, 8, 7, 255, 1, 2, 3, 255]);

    // Edge case: the background's transparent-index entry has alpha 0, so
    // it renders as that entry — transparent.
    let pal = palette(&[[9, 8, 7, 0]]);
    let mut bg = Layer::image(LayerId::new(0), "Background");
    bg.extra_flags = BACKGROUND_FLAG;
    let sprite = sprite_with(1, 1, INDEXED_T0, pal, vec![bg], 1);
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        buffer(1, 1, INDEXED_T0, vec![0]),
    ));
    let out = compose_full(&sprite, &cels, 0).expect("indexed composes");
    assert_eq!(out, vec![0, 0, 0, 0]);
}

#[test]
fn compose_grayscale_renders_value_value_value_alpha() {
    // AC-04: (V, A) → (V, V, V, A); A = 0 contributes nothing whatever V
    // is; layer opacity scales alpha; the background flag changes nothing.
    let mut layer = Layer::image(LayerId::new(0), "art");
    layer.extra_flags = BACKGROUND_FLAG;
    let sprite = sprite_with(
        3,
        1,
        ColorMode::Grayscale,
        Palette::default(),
        vec![layer],
        1,
    );
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        buffer(3, 1, ColorMode::Grayscale, vec![77, 255, 200, 100, 123, 0]),
    ));
    let out = compose_full(&sprite, &cels, 0).expect("grayscale composes");
    assert_eq!(out, vec![77, 77, 77, 255, 200, 200, 200, 100, 0, 0, 0, 0]);

    let mut layer = Layer::image(LayerId::new(0), "art");
    layer.opacity = 128;
    let sprite = sprite_with(
        1,
        1,
        ColorMode::Grayscale,
        Palette::default(),
        vec![layer],
        1,
    );
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        buffer(1, 1, ColorMode::Grayscale, vec![40, 255]),
    ));
    let out = compose_full(&sprite, &cels, 0).expect("grayscale composes");
    assert_eq!(out, vec![40, 40, 40, 128]);
}

/// Hand expansion of one indexed / grayscale buffer to RGBA by the rules of
/// AC-01 … AC-04 and AC-12.
fn expand(buf: &PixelBuffer, pal: &Palette, background: bool) -> PixelBuffer {
    let mut out = PixelBuffer::empty(buf.width, buf.height, ColorMode::Rgba);
    let px_count = (buf.width * buf.height) as usize;
    for i in 0..px_count {
        let rgba = match buf.color_mode {
            ColorMode::Rgba => [
                buf.data[i * 4],
                buf.data[i * 4 + 1],
                buf.data[i * 4 + 2],
                buf.data[i * 4 + 3],
            ],
            ColorMode::Grayscale => {
                let v = buf.data[i * 2];
                [v, v, v, buf.data[i * 2 + 1]]
            }
            ColorMode::Indexed { transparent_index } => {
                let idx = buf.data[i];
                if idx == transparent_index && !background {
                    [0; 4]
                } else {
                    match pal.colors.get(usize::from(idx)) {
                        Some(e) => [e.rgba.r, e.rgba.g, e.rgba.b, e.rgba.a],
                        None => [0; 4],
                    }
                }
            }
        };
        out.data[i * 4..i * 4 + 4].copy_from_slice(&rgba);
    }
    out
}

/// The RGBA twin of `sprite` / `cels`: same structure, every buffer
/// expanded by [`expand`].
fn rgba_twin(sprite: &Sprite, cels: &CelMap) -> (Sprite, CelMap) {
    let mut twin = sprite.clone();
    twin.color_mode = ColorMode::Rgba;
    for ts in &mut twin.tilesets {
        for tile in &mut ts.tiles {
            // The random sprites carry no tilemap layers; tiles expand as
            // on a non-background layer.
            tile.pixels = expand(&tile.pixels, &sprite.palette, false);
        }
    }
    let mut twin_cels = CelMap::new();
    for (_, cel) in cels.iter() {
        let mut c = cel.clone();
        if let CelData::Image(buf) = &cel.data {
            let background = sprite
                .layer(cel.layer)
                .is_some_and(|l| l.extra_flags & BACKGROUND_FLAG != 0);
            c.data = CelData::Image(expand(buf, &sprite.palette, background));
        }
        twin_cels.insert(c);
    }
    (twin, twin_cels)
}

/// Deterministic xorshift generator — the crate carries no `proptest`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn byte(&mut self) -> u8 {
        (self.next() & 0xff) as u8
    }
}

/// A random 3-layer, 3-frame sprite in `mode`, 6×5 pixels.
fn random_sprite(rng: &mut Rng, mode: ColorMode) -> (Sprite, CelMap) {
    let (w, h) = (6u32, 5u32);
    let pal_len = 1 + rng.below(12) as usize;
    let pal = Palette::from_entries(
        (0..pal_len)
            .map(|_| {
                let a = [0, 255, rng.byte()][rng.below(3) as usize];
                PaletteEntry::new(Rgba::new(rng.byte(), rng.byte(), rng.byte(), a))
            })
            .collect(),
    );
    let mut layers = Vec::new();
    for id in 0..3u32 {
        let mut layer = Layer::image(LayerId::new(id), format!("l{id}"));
        layer.blend_mode = BlendMode::ALL[rng.below(BlendMode::ALL.len() as u64) as usize];
        layer.opacity = [255, 128, rng.byte()][rng.below(3) as usize];
        if rng.below(3) == 0 {
            layer.extra_flags = BACKGROUND_FLAG;
        }
        layer.visible = rng.below(6) != 0;
        layers.push(layer);
    }
    let sprite = sprite_with(w, h, mode, pal, layers, 3);
    let mut cels = CelMap::new();
    for frame in 0..3u32 {
        for id in 0..3u32 {
            if rng.below(5) == 0 {
                continue;
            }
            let cw = 1 + rng.below(u64::from(w) + 1) as u32;
            let ch = 1 + rng.below(u64::from(h) + 1) as u32;
            let len = (cw * ch) as usize * mode.bytes_per_pixel();
            let data = (0..len)
                .map(|_| match mode {
                    // Mostly valid indexes, some transparent, some past the end.
                    ColorMode::Indexed { .. } => rng.below(pal_len as u64 + 3) as u8,
                    _ => [0, 255, rng.byte()][rng.below(3) as usize],
                })
                .collect();
            let mut cel = Cel::image(
                LayerId::new(id),
                FrameIndex::new(frame),
                buffer(cw, ch, mode, data),
            );
            cel.position = (rng.below(5) as i32 - 2, rng.below(5) as i32 - 2);
            cel.opacity = [255, 128, rng.byte()][rng.below(3) as usize];
            cels.insert(cel);
        }
    }
    (sprite, cels)
}

fn random_request(rng: &mut Rng, sprite: &Sprite) -> ComposeRequest {
    let (w, h) = (sprite.width, sprite.height);
    let vx = rng.below(3) as i32 - 1;
    let vy = rng.below(3) as i32 - 1;
    let viewport = Rect::new(vx, vy, 1 + rng.below(u64::from(w) + 1) as u32, h);
    let dirty_hint = (rng.below(2) == 0).then(|| {
        Rect::new(
            rng.below(u64::from(w)) as i32,
            rng.below(u64::from(h)) as i32,
            1 + rng.below(4) as u32,
            1 + rng.below(4) as u32,
        )
    });
    let onion_skin = (rng.below(2) == 0).then(|| OnionSkin {
        previous_alpha: rng.byte(),
        next_alpha: rng.byte(),
        frames_back: rng.below(3) as u32,
        frames_forward: rng.below(3) as u32,
    });
    let include_layers = match rng.below(3) {
        0 => LayerFilter::Visible,
        1 => LayerFilter::All,
        _ => LayerFilter::Only(vec![LayerId::new(rng.below(3) as u32)]),
    };
    ComposeRequest {
        viewport,
        zoom: 1 + rng.below(3) as u32,
        onion_skin,
        include_layers,
        dirty_hint,
        ..ComposeRequest::full(FrameIndex::new(rng.below(3) as u32), w, h)
    }
}

#[test]
fn compose_indexed_and_grayscale_equal_their_rgba_expansion() {
    // AC-05: for random structures, blend modes, opacities, zooms,
    // viewports, dirty hints, layer filters and onion-skin settings, the
    // indexed / grayscale compose equals the compose of the hand-expanded
    // RGBA twin.
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for case in 0..400 {
        let mode = match case % 3 {
            0 => INDEXED_T0,
            1 => ColorMode::Indexed {
                transparent_index: rng.byte() % 8,
            },
            _ => ColorMode::Grayscale,
        };
        let (sprite, cels) = random_sprite(&mut rng, mode);
        let (twin, twin_cels) = rgba_twin(&sprite, &cels);
        for _ in 0..4 {
            let req = random_request(&mut rng, &sprite);
            let mut got = Vec::new();
            let got_r = compose(&sprite, &cels, &req, &mut got).expect("source composes");
            let mut want = Vec::new();
            let want_r = compose(&twin, &twin_cels, &req, &mut want).expect("twin composes");
            assert_eq!(got_r, want_r, "case {case} {mode:?} {req:?}");
            assert_eq!(got, want, "case {case} {mode:?} {req:?}");
        }
    }
}

/// A tileset in `mode` whose tile 0 is *not* blank (so rendering it would
/// show) and whose tile 1 holds the given 2×1 pixels.
fn tileset_in(mode: ColorMode, tile0: Vec<u8>, tile1: Vec<u8>) -> Tileset {
    let mut ts = Tileset::new(TilesetId::new(0), "tiles", (2, 1));
    ts.tiles.push(TileImage {
        pixels: buffer(2, 1, mode, tile0),
    });
    ts.tiles.push(TileImage {
        pixels: buffer(2, 1, mode, tile1),
    });
    ts
}

fn tilemap_sprite(mode: ColorMode, pal: Palette, ts: Tileset, flags: u16) -> (Sprite, CelMap) {
    let mut layer = Layer::tilemap(LayerId::new(0), "map", TilesetId::new(0));
    layer.extra_flags = flags;
    let sprite = Sprite::builder(4, 1)
        .color_mode(mode)
        .palette(pal)
        .add_layer(layer)
        .add_frame(Frame::new(100))
        .add_tileset(ts)
        .build()
        .expect("tilemap sprite builds");
    let mut cels = CelMap::new();
    cels.insert(Cel {
        data: CelData::Tilemap {
            grid_w: 2,
            grid_h: 1,
            tiles: vec![
                TileRef::EMPTY,
                TileRef {
                    flip_x: true,
                    ..TileRef::new(1)
                },
            ],
        },
        ..Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            PixelBuffer::empty(0, 0, mode),
        )
    });
    (sprite, cels)
}

#[test]
fn compose_indexed_and_grayscale_tilemaps_follow_image_rules() {
    // AC-06: tile 0 stays empty even though its pixels are opaque; tile 1
    // (flipped) renders through the palette / grayscale expansion, with the
    // transparent index transparent.
    let pal = palette(&[[0, 0, 0, 255], [10, 20, 30, 255], [40, 50, 60, 255]]);
    let ts = tileset_in(INDEXED_T0, vec![1, 1], vec![2, 0]);
    let (sprite, cels) = tilemap_sprite(INDEXED_T0, pal.clone(), ts, 0);
    let out = compose_full(&sprite, &cels, 0).expect("indexed tilemap composes");
    assert_eq!(
        out,
        vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 40, 50, 60, 255]
    );

    let ts = tileset_in(
        ColorMode::Grayscale,
        vec![9, 255, 9, 255],
        vec![70, 255, 90, 128],
    );
    let (sprite, cels) = tilemap_sprite(ColorMode::Grayscale, Palette::default(), ts, 0);
    let out = compose_full(&sprite, &cels, 0).expect("grayscale tilemap composes");
    assert_eq!(
        out,
        vec![0, 0, 0, 0, 0, 0, 0, 0, 90, 90, 90, 128, 70, 70, 70, 255]
    );

    // Background flag on a tilemap layer: the transparent index renders as
    // its entry, as on an image layer.
    let ts = tileset_in(INDEXED_T0, vec![1, 1], vec![2, 0]);
    let (sprite, cels) = tilemap_sprite(INDEXED_T0, pal, ts, BACKGROUND_FLAG);
    let out = compose_full(&sprite, &cels, 0).expect("indexed tilemap composes");
    assert_eq!(&out[8..16], &[0, 0, 0, 255, 40, 50, 60, 255]);
}

#[test]
fn compose_indexed_follows_palette_recolour_and_its_undo() {
    // AC-07: a recolour drag (merged into one undo step) shows its final
    // colour; one undo restores the original.
    let pal = palette(&[[0, 0, 0, 0], [255, 0, 0, 255]]);
    let mut sprite = sprite_with(
        1,
        1,
        INDEXED_T0,
        pal,
        vec![Layer::image(LayerId::new(0), "a")],
        1,
    );
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        buffer(1, 1, INDEXED_T0, vec![1]),
    ));
    assert_eq!(
        compose_full(&sprite, &cels, 0).unwrap(),
        vec![255, 0, 0, 255]
    );

    let mut bus = Bus::new();
    for g in [50u8, 100, 200] {
        bus.execute(
            SetPaletteColor::new(1, Rgba::new(0, g, 0, 255)).into(),
            &mut sprite,
            &mut cels,
        )
        .expect("recolour applies");
    }
    assert_eq!(
        compose_full(&sprite, &cels, 0).unwrap(),
        vec![0, 200, 0, 255]
    );
    assert!(bus.undo(&mut sprite, &mut cels));
    assert_eq!(
        compose_full(&sprite, &cels, 0).unwrap(),
        vec![255, 0, 0, 255]
    );
}

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("png header parses");
    let size = reader.output_buffer_size().expect("known output size");
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).expect("frame decodes");
    assert_eq!(info.color_type, png::ColorType::Rgba);
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

#[test]
fn export_indexed_and_grayscale_frames_equal_compose() {
    // AC-08: PNG frame export and every atlas cell equal compose().
    let mut rng = Rng(0x0123_4567_89ab_cdef);
    for mode in [INDEXED_T0, ColorMode::Grayscale] {
        let (mut sprite, cels) = random_sprite(&mut rng, mode);
        for layer in &mut sprite.layers {
            layer.visible = true;
        }
        let (w, h) = (sprite.width, sprite.height);
        let mut frames = Vec::new();
        for f in 0..3u32 {
            let want = compose_full(&sprite, &cels, f).expect("frame composes");
            let png = export_frame_png(&sprite, &cels, FrameIndex::new(f)).expect("frame exports");
            assert_eq!(decode_png(&png), (w, h, want.clone()), "{mode:?} frame {f}");
            frames.push(want);
        }
        let atlas = export_atlas_png(&sprite, &cels, &AtlasOptions::new(3)).expect("atlas exports");
        let (aw, ah, pixels) = decode_png(&atlas.png);
        assert_eq!((aw, ah), (w * 3, h));
        for (f, want) in frames.iter().enumerate() {
            for y in 0..h as usize {
                let row = (y * aw as usize + f * w as usize) * 4;
                let cell = &pixels[row..row + w as usize * 4];
                let src = &want[y * w as usize * 4..(y + 1) * w as usize * 4];
                assert_eq!(cell, src, "{mode:?} atlas frame {f} row {y}");
            }
        }
    }
}

#[test]
fn compose_cel_or_tile_in_another_color_mode_fails_with_mismatch() {
    // AC-09: an RGBA cel in an indexed sprite, and a grayscale tile in an
    // indexed sprite.
    let pal = palette(&[[0, 0, 0, 0], [1, 2, 3, 255]]);
    let sprite = sprite_with(
        1,
        1,
        INDEXED_T0,
        pal.clone(),
        vec![Layer::image(LayerId::new(0), "a")],
        1,
    );
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        PixelBuffer::empty(1, 1, ColorMode::Rgba),
    ));
    assert_eq!(
        compose_full(&sprite, &cels, 0),
        Err(RenderError::CelColorModeMismatch {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            mode: ColorMode::Rgba,
        })
    );

    let ts = tileset_in(ColorMode::Grayscale, vec![0; 4], vec![0; 4]);
    let (sprite, cels) = tilemap_sprite(INDEXED_T0, pal, ts, 0);
    assert_eq!(
        compose_full(&sprite, &cels, 0),
        Err(RenderError::CelColorModeMismatch {
            layer: LayerId::new(0),
            frame: FrameIndex::new(0),
            mode: ColorMode::Grayscale,
        })
    );
}

#[test]
fn compose_malformed_indexed_or_grayscale_buffer_fails() {
    // AC-10: a buffer one byte short, for an image cel and for a tile.
    for mode in [INDEXED_T0, ColorMode::Grayscale] {
        let layer = Layer::image(LayerId::new(0), "a");
        let sprite = sprite_with(2, 2, mode, Palette::default(), vec![layer], 1);
        let mut buf = PixelBuffer::empty(2, 2, mode);
        buf.data.pop();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));
        assert_eq!(
            compose_full(&sprite, &cels, 0),
            Err(RenderError::MalformedCelBuffer {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }),
            "{mode:?} image cel"
        );

        let bpp = mode.bytes_per_pixel();
        let mut ts = tileset_in(mode, vec![0; 2 * bpp], vec![0; 2 * bpp]);
        ts.tiles[1].pixels.data.pop();
        let (sprite, cels) = tilemap_sprite(mode, Palette::default(), ts, 0);
        assert_eq!(
            compose_full(&sprite, &cels, 0),
            Err(RenderError::MalformedCelBuffer {
                layer: LayerId::new(0),
                frame: FrameIndex::new(0),
            }),
            "{mode:?} tile"
        );
    }
}

#[test]
fn compose_accepts_every_color_mode() {
    // AC-11: no color mode the model can represent is refused — every one
    // composes (the `UnsupportedColorMode` render error no longer exists).
    let modes = [
        ColorMode::Rgba,
        ColorMode::Grayscale,
        INDEXED_T0,
        ColorMode::Indexed {
            transparent_index: 255,
        },
    ];
    for mode in modes {
        let sprite = sprite_with(
            2,
            2,
            mode,
            Palette::db32(),
            vec![Layer::image(LayerId::new(0), "a")],
            1,
        );
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            PixelBuffer::empty(2, 2, mode),
        ));
        let out = compose_full(&sprite, &cels, 0);
        assert!(out.is_ok(), "{mode:?}: {out:?}");
    }
}

#[test]
fn compose_indexed_index_past_palette_end_contributes_nothing() {
    // AC-12: indexes 2 and 200 lie past a 2-entry palette and are not the
    // transparent index (255); they leave the white layer below untouched,
    // also on a background layer.
    let mode = ColorMode::Indexed {
        transparent_index: 255,
    };
    for flags in [0, BACKGROUND_FLAG] {
        let pal = palette(&[[255, 255, 255, 255], [5, 5, 5, 255]]);
        let bottom = Layer::image(LayerId::new(0), "bottom");
        let mut top = Layer::image(LayerId::new(1), "top");
        top.extra_flags = flags;
        let sprite = sprite_with(3, 1, mode, pal, vec![bottom, top], 1);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            buffer(3, 1, mode, vec![0, 0, 0]),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            buffer(3, 1, mode, vec![2, 200, 1]),
        ));
        let out = compose_full(&sprite, &cels, 0).expect("indexed composes");
        assert_eq!(
            out,
            vec![255, 255, 255, 255, 255, 255, 255, 255, 5, 5, 5, 255],
            "flags {flags:#x}"
        );
    }
}
