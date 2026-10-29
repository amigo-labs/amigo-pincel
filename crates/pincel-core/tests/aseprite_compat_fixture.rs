//! Committed `.aseprite` files for the manual upstream-Aseprite check
//! (`docs/specs/aseprite-round-trip.md` AC-24; checklist in
//! `tests/fixtures/aseprite-compat-check.md`).
//!
//! Each test builds its sprite through the `pincel-core` API, writes it
//! with [`write_aseprite`] and compares the bytes with the committed
//! fixture, so the file someone opened in Aseprite is the file Pincel
//! writes today. When the writer changes on purpose, regenerate with
//! `PINCEL_REGENERATE_FIXTURES=1 cargo test -p pincel-core --test
//! aseprite_compat_fixture` and re-run the manual check.

use std::path::PathBuf;

use pincel_core::{
    AsepriteReadOutput, BlendMode, Cel, CelData, CelMap, ColorMode, Frame, FrameIndex, Layer,
    LayerId, LayerKind, Palette, PaletteEntry, PixelBuffer, Rect, Rgba, Slice, SliceId, SliceKey,
    Sprite, Tag, TagDirection, TileImage, TileRef, Tileset, TilesetId, read_aseprite,
    write_aseprite,
};

const REGENERATE: &str = "PINCEL_REGENERATE_FIXTURES";

/// Name, kind, visible, blend mode, opacity, parent.
type LayerShape<'a> = (&'a str, LayerKind, bool, BlendMode, u8, Option<LayerId>);

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

/// Write the sprite, then either regenerate the fixture or demand that
/// it matches byte for byte. Returns the committed bytes, read back.
fn check_fixture(name: &str, sprite: &Sprite, cels: &CelMap) -> AsepriteReadOutput {
    let mut bytes = Vec::new();
    write_aseprite(sprite, cels, &mut bytes).expect("fixture sprite writes");
    let path = fixture_path(name);
    if std::env::var_os(REGENERATE).is_some_and(|v| v == "1") {
        std::fs::write(&path, &bytes).expect("fixture written");
    }
    let committed = std::fs::read(&path).expect("committed fixture readable");
    assert!(
        committed == bytes,
        "{name} differs from what Pincel writes now; if the change is intended, \
         run with {REGENERATE}=1 to regenerate it and repeat the manual Aseprite \
         check in tests/fixtures/aseprite-compat-check.md"
    );
    read_aseprite(&committed).expect("committed fixture opens")
}

fn fill(w: u32, h: u32, c: [u8; 4]) -> PixelBuffer {
    PixelBuffer {
        width: w,
        height: h,
        color_mode: ColorMode::Rgba,
        data: c.repeat((w * h) as usize),
    }
}

fn cel(layer: u32, frame: u32, position: (i32, i32), data: CelData) -> Cel {
    Cel {
        layer: LayerId::new(layer),
        frame: FrameIndex::new(frame),
        position,
        opacity: 255,
        data,
        user_data: None,
        z_index: 0,
    }
}

/// "Terrain": 3×2 tiles — empty, green with a dark left column, brown
/// with a white top-left pixel (so flips are visible).
fn terrain() -> Tileset {
    let tile = |px: &dyn Fn(u32, u32) -> [u8; 4]| {
        let mut data = Vec::new();
        for y in 0..2 {
            for x in 0..3 {
                data.extend_from_slice(&px(x, y));
            }
        }
        TileImage {
            pixels: PixelBuffer {
                width: 3,
                height: 2,
                color_mode: ColorMode::Rgba,
                data,
            },
        }
    };
    let mut ts = Tileset::new(TilesetId::new(0), "Terrain", (3, 2));
    ts.tiles = vec![
        tile(&|_, _| [0, 0, 0, 0]),
        tile(&|x, _| {
            if x == 0 {
                [20, 80, 20, 255]
            } else {
                [60, 170, 70, 255]
            }
        }),
        tile(&|x, y| {
            if x == 0 && y == 0 {
                [255, 255, 255, 255]
            } else {
                [140, 90, 40, 255]
            }
        }),
    ];
    ts
}

fn tile(id: u32, flip_x: bool, flip_y: bool) -> TileRef {
    TileRef {
        tile_id: id,
        flip_x,
        flip_y,
        rotate_90: false,
    }
}

/// The 4×3 tile grid, row-major.
fn terrain_grid() -> Vec<TileRef> {
    vec![
        tile(1, false, false),
        tile(1, true, false),
        tile(2, false, false),
        tile(2, false, true),
        tile(0, false, false),
        tile(1, false, false),
        tile(2, true, true),
        tile(1, false, false),
        tile(2, false, false),
        tile(0, false, false),
        tile(1, false, true),
        tile(2, false, false),
    ]
}

fn tilemap_cel(layer: u32) -> Cel {
    cel(
        layer,
        0,
        (2, 16),
        CelData::Tilemap {
            grid_w: 4,
            grid_h: 3,
            tiles: terrain_grid(),
        },
    )
}

fn assert_terrain(read: &AsepriteReadOutput, layer: u32) {
    assert_eq!(read.sprite.tilesets, vec![terrain()]);
    let cel = read
        .cels
        .get(LayerId::new(layer), FrameIndex::new(0))
        .expect("tilemap cel");
    assert_eq!(cel, &tilemap_cel(layer));
}

#[test]
fn aseprite_compat_check_fixture_matches_writer_output() {
    let mut hero = Layer::image(LayerId::new(2), "Hero");
    hero.parent = Some(LayerId::new(1));
    hero.blend_mode = BlendMode::Multiply;
    hero.opacity = 160;
    let mut notes = Layer::image(LayerId::new(3), "Hidden Notes");
    notes.visible = false;
    let tag = |name: &str, from, to, direction, color, repeats| Tag {
        name: name.into(),
        from: FrameIndex::new(from),
        to: FrameIndex::new(to),
        direction,
        color,
        repeats,
        user_data: None,
        raw_direction: None,
    };
    let slice = |id, name: &str, color, bounds, center, pivot| Slice {
        id: SliceId::new(id),
        name: name.into(),
        color,
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds,
            center,
            pivot,
        }],
        user_data: None,
    };
    let palette = Palette::from_entries(vec![
        PaletteEntry::with_name(Rgba::new(150, 200, 255, 255), "Sky"),
        PaletteEntry::with_name(Rgba::new(220, 40, 40, 255), "Hero Red"),
        PaletteEntry::with_name(Rgba::new(60, 170, 70, 255), "Grass"),
        PaletteEntry::with_name(Rgba::new(250, 220, 60, 128), "Note Yellow"),
    ]);
    let tags = vec![
        tag(
            "idle",
            0,
            1,
            TagDirection::Forward,
            Rgba::new(200, 40, 40, 255),
            0,
        ),
        tag(
            "bounce",
            1,
            3,
            TagDirection::Pingpong,
            Rgba::new(40, 90, 220, 255),
            2,
        ),
    ];
    let slices = vec![
        slice(
            0,
            "hitbox",
            Rgba::new(255, 0, 0, 255),
            Rect::new(4, 8, 8, 8),
            None,
            None,
        ),
        slice(
            1,
            "panel",
            Rgba::new(0, 128, 255, 255),
            Rect::new(16, 2, 12, 10),
            Some(Rect::new(3, 3, 6, 4)),
            Some((6, 5)),
        ),
    ];
    let mut builder = Sprite::builder(32, 24)
        .add_layer(Layer::image(LayerId::new(0), "Background"))
        .add_layer(Layer::group(LayerId::new(1), "Characters"))
        .add_layer(hero)
        .add_layer(notes)
        .add_layer(Layer::tilemap(LayerId::new(4), "Tiles", TilesetId::new(0)))
        .add_tileset(terrain())
        .palette(palette.clone());
    for ms in [100, 150, 200, 250] {
        builder = builder.add_frame(Frame::new(ms));
    }
    for t in &tags {
        builder = builder.add_tag(t.clone());
    }
    for s in &slices {
        builder = builder.add_slice(s.clone());
    }
    let sprite = builder.build().expect("sprite builds");

    let mut cels = CelMap::new();
    cels.insert(cel(
        0,
        0,
        (0, 0),
        CelData::Image(fill(32, 24, [150, 200, 255, 255])),
    ));
    for f in 1..4 {
        cels.insert(cel(0, f, (0, 0), CelData::Linked(FrameIndex::new(0))));
    }
    for f in 0..4 {
        cels.insert(cel(
            2,
            f,
            (4 + 4 * f as i32, 8),
            CelData::Image(fill(8, 8, [220, 40, 40, 255])),
        ));
    }
    cels.insert(cel(
        3,
        0,
        (0, 0),
        CelData::Image(fill(4, 4, [250, 220, 60, 255])),
    ));
    cels.insert(tilemap_cel(4));

    let read = check_fixture("aseprite-compat-check.aseprite", &sprite, &cels);

    let s = &read.sprite;
    assert_eq!((s.width, s.height), (32, 24));
    let layers: Vec<LayerShape<'_>> = s
        .layers
        .iter()
        .map(|l| {
            (
                l.name.as_str(),
                l.kind.clone(),
                l.visible,
                l.blend_mode,
                l.opacity,
                l.parent,
            )
        })
        .collect();
    let image = |name| (name, LayerKind::Image, true, BlendMode::Normal, 255, None);
    assert_eq!(
        layers,
        vec![
            image("Background"),
            (
                "Characters",
                LayerKind::Group,
                true,
                BlendMode::Normal,
                255,
                None
            ),
            (
                "Hero",
                LayerKind::Image,
                true,
                BlendMode::Multiply,
                160,
                Some(LayerId::new(1))
            ),
            (
                "Hidden Notes",
                LayerKind::Image,
                false,
                BlendMode::Normal,
                255,
                None
            ),
            (
                "Tiles",
                LayerKind::Tilemap {
                    tileset_id: TilesetId::new(0)
                },
                true,
                BlendMode::Normal,
                255,
                None
            ),
        ]
    );
    let durations: Vec<u16> = s.frames.iter().map(|f| f.duration_ms).collect();
    assert_eq!(durations, vec![100, 150, 200, 250]);
    for f in 1..4 {
        assert_eq!(
            read.cels
                .get(LayerId::new(0), FrameIndex::new(f))
                .map(|c| &c.data),
            Some(&CelData::Linked(FrameIndex::new(0)))
        );
    }
    assert_eq!(s.tags, tags);
    assert_eq!(s.palette, palette);
    assert_eq!(s.slices, slices);
    assert_terrain(&read, 4);
}

#[test]
fn aseprite_compat_tilemap_min_fixture_matches_writer_output() {
    // Only a tilemap layer and its tileset: the smallest file that shows
    // whether Aseprite resolves the tileset for the layer.
    let sprite = Sprite::builder(16, 24)
        .add_layer(Layer::tilemap(LayerId::new(0), "Tiles", TilesetId::new(0)))
        .add_tileset(terrain())
        .add_frame(Frame::new(100))
        .build()
        .expect("sprite builds");
    let mut cels = CelMap::new();
    cels.insert(tilemap_cel(0));

    let read = check_fixture("aseprite-compat-tilemap-min.aseprite", &sprite, &cels);

    assert_eq!(read.sprite.layers.len(), 1);
    assert_eq!(
        read.sprite.layers[0].kind,
        LayerKind::Tilemap {
            tileset_id: TilesetId::new(0)
        }
    );
    assert_terrain(&read, 0);
}
