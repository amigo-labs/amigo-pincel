//! Integration tests for the Aseprite codec pair (M4 read + M5 write).
//!
//! Each test builds a Pincel [`Sprite`] / [`CelMap`], writes it to bytes
//! via [`write_aseprite`], reads the bytes back via [`read_aseprite`],
//! and asserts the result preserves the original document. The
//! round-trip is the canonical Phase 1 integration test for the codec
//! pair; see `docs/specs/pincel.md` §7.1.

use pincel_core::{
    AsepriteReadOutput, BlendMode, Cel, CelData, CelMap, ColorMode, Command, ExternalFile, Frame,
    FrameIndex, Layer, LayerId, LayerKind, MoveDirection, MoveLayer, Palette, PaletteEntry,
    PixelBuffer, Rect, Rgba, Slice, SliceId, SliceKey, Sprite, Tag, TagDirection, TileImage,
    TileRef, Tileset, TilesetId, UserData, read_aseprite, write_aseprite,
};

fn round_trip(sprite: &Sprite, cels: &CelMap) -> AsepriteReadOutput {
    let mut bytes = Vec::new();
    write_aseprite(sprite, cels, &mut bytes).expect("writer succeeds");
    read_aseprite(&bytes).expect("reader accepts our own output")
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    [r, g, b, a]
}

fn flat_pixels(colors: &[[u8; 4]]) -> Vec<u8> {
    colors.iter().flatten().copied().collect()
}

fn rgba_buffer(width: u32, height: u32, pixels: Vec<u8>) -> PixelBuffer {
    assert_eq!(
        pixels.len(),
        (width as usize) * (height as usize) * 4,
        "test bug: rgba_buffer pixel byte count must match width*height*4",
    );
    PixelBuffer {
        width,
        height,
        color_mode: ColorMode::Rgba,
        data: pixels,
    }
}

#[test]
fn single_layer_single_frame_round_trips_pixels() {
    let pixels = flat_pixels(&[
        rgba(255, 0, 0, 255),
        rgba(0, 255, 0, 255),
        rgba(0, 0, 255, 255),
        rgba(255, 255, 255, 128),
    ]);
    let mut bg = Layer::image(LayerId::new(0), "Background");
    bg.opacity = 200;
    let sprite = Sprite::builder(2, 2)
        .add_layer(bg)
        .add_frame(Frame::new(120))
        .build()
        .unwrap();
    let mut cels = CelMap::new();
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(rgba_buffer(2, 2, pixels.clone())),
        user_data: None,
        z_index: 0,
    });

    let AsepriteReadOutput { sprite, cels } = round_trip(&sprite, &cels);

    assert_eq!(sprite.width, 2);
    assert_eq!(sprite.height, 2);
    assert_eq!(sprite.color_mode, ColorMode::Rgba);
    assert_eq!(sprite.layers.len(), 1);
    assert_eq!(sprite.frames.len(), 1);
    assert_eq!(sprite.frames[0].duration_ms, 120);

    let layer = &sprite.layers[0];
    assert_eq!(layer.id, LayerId::new(0));
    assert_eq!(layer.name, "Background");
    assert_eq!(layer.kind, LayerKind::Image);
    assert!(layer.visible);
    assert!(layer.editable);
    assert_eq!(layer.blend_mode, BlendMode::Normal);
    assert_eq!(layer.opacity, 200);

    let cel = cels
        .get(LayerId::new(0), FrameIndex::new(0))
        .expect("single cel should be present");
    assert_eq!(cel.position, (0, 0));
    assert_eq!(cel.opacity, 255);
    let CelData::Image(buf) = &cel.data else {
        panic!("expected an image cel, got {:?}", cel.data);
    };
    assert_eq!(buf.width, 2);
    assert_eq!(buf.height, 2);
    assert_eq!(buf.color_mode, ColorMode::Rgba);
    assert_eq!(buf.data, pixels, "decoded pixels should match source bytes");
}

#[test]
fn multi_frame_document_round_trips_canvas_layers_durations_and_cels() {
    // Non-square canvas and cels, every layer kind, distinct durations,
    // and image cels with non-default positions and opacities on several
    // frames, so a swapped or dropped field cannot pass by coincidence.
    let mut child = Layer::image(LayerId::new(2), "child");
    child.parent = Some(LayerId::new(1));
    let sprite = Sprite::builder(5, 3)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_layer(Layer::group(LayerId::new(1), "grp"))
        .add_layer(child)
        .add_layer(Layer::tilemap(LayerId::new(3), "tiles", TilesetId::new(0)))
        .add_tileset(Tileset::new(TilesetId::new(0), "ts", (2, 2)))
        .add_frame(Frame::new(40))
        .add_frame(Frame::new(90))
        .add_frame(Frame::new(250))
        .build()
        .expect("sprite builds");
    let image_cel = |layer: u32, frame: u32, position: (i32, i32), opacity: u8, w: u32, h: u32| {
        let pixels = (0..w * h * 4)
            .map(|i| (i * 13 + layer * 7 + frame * 31) as u8)
            .collect();
        Cel {
            layer: LayerId::new(layer),
            frame: FrameIndex::new(frame),
            position,
            opacity,
            data: CelData::Image(rgba_buffer(w, h, pixels)),
            user_data: None,
            z_index: 0,
        }
    };
    let originals = [
        image_cel(0, 0, (1, 0), 77, 3, 2),
        image_cel(2, 1, (-2, 1), 128, 2, 3),
        image_cel(0, 2, (0, 2), 255, 4, 1),
        image_cel(2, 2, (3, -1), 9, 1, 2),
    ];
    let mut cels = CelMap::new();
    for cel in &originals {
        cels.insert(cel.clone());
    }

    let AsepriteReadOutput { sprite: read, cels } = round_trip(&sprite, &cels);

    assert_eq!((read.width, read.height), (5, 3));
    assert_eq!(read.color_mode, ColorMode::Rgba);
    let shape: Vec<(&str, LayerKind)> = read
        .layers
        .iter()
        .map(|l| (l.name.as_str(), l.kind.clone()))
        .collect();
    assert_eq!(
        shape,
        vec![
            ("bg", LayerKind::Image),
            ("grp", LayerKind::Group),
            ("child", LayerKind::Image),
            (
                "tiles",
                LayerKind::Tilemap {
                    tileset_id: TilesetId::new(0)
                }
            ),
        ]
    );
    assert_eq!(read.frames.len(), 3);
    let durations: Vec<u16> = read.frames.iter().map(|f| f.duration_ms).collect();
    assert_eq!(durations, vec![40, 90, 250]);
    assert_eq!(cels.len(), originals.len());
    for original in &originals {
        let back = cels
            .get(original.layer, original.frame)
            .expect("image cel survives");
        assert_eq!(back, original, "cel position, opacity and pixels");
    }
}

#[test]
fn multi_layer_blend_modes_and_offsets_are_preserved() {
    let bg_pixels = flat_pixels(&[rgba(10, 20, 30, 255); 4]);
    let fx_pixels = flat_pixels(&[rgba(200, 100, 50, 128), rgba(1, 2, 3, 4)]);

    let bg = Layer::image(LayerId::new(0), "bg");
    let mut fx = Layer::image(LayerId::new(1), "fx");
    fx.visible = false;
    fx.opacity = 180;
    fx.blend_mode = BlendMode::Multiply;

    let sprite = Sprite::builder(4, 4)
        .add_layer(bg)
        .add_layer(fx)
        .add_frame(Frame::new(100))
        .build()
        .unwrap();

    let mut cels = CelMap::new();
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(rgba_buffer(2, 2, bg_pixels)),
        user_data: None,
        z_index: 0,
    });
    cels.insert(Cel {
        layer: LayerId::new(1),
        frame: FrameIndex::new(0),
        position: (1, 2),
        opacity: 200,
        data: CelData::Image(rgba_buffer(2, 1, fx_pixels)),
        user_data: None,
        z_index: 0,
    });

    let AsepriteReadOutput { sprite, cels } = round_trip(&sprite, &cels);

    assert_eq!(sprite.layers.len(), 2);
    assert!(sprite.layers[0].visible);
    assert_eq!(sprite.layers[0].opacity, 255);
    assert_eq!(sprite.layers[0].blend_mode, BlendMode::Normal);
    assert_eq!(sprite.layers[1].blend_mode, BlendMode::Multiply);
    assert_eq!(sprite.layers[1].opacity, 180);
    assert!(!sprite.layers[1].visible);

    let fx_cel = cels
        .get(LayerId::new(1), FrameIndex::new(0))
        .expect("fx cel present");
    assert_eq!(fx_cel.position, (1, 2));
    assert_eq!(fx_cel.opacity, 200);
    let CelData::Image(buf) = &fx_cel.data else {
        panic!("fx cel should be an image");
    };
    assert_eq!((buf.width, buf.height), (2, 1));
}

#[test]
fn every_blend_mode_visibility_and_opacity_round_trip_per_layer() {
    let mut builder = Sprite::builder(2, 2).add_frame(Frame::new(100));
    let mut originals = Vec::new();
    for (i, mode) in BlendMode::ALL.iter().enumerate() {
        let mut layer = Layer::image(LayerId::new(i as u32), format!("L{i}"));
        layer.blend_mode = *mode;
        layer.visible = i % 2 == 0;
        layer.opacity = 255 - (i as u8) * 11;
        originals.push(layer.clone());
        builder = builder.add_layer(layer);
    }
    let sprite = builder.build().expect("sprite builds");

    let AsepriteReadOutput { sprite: read, .. } = round_trip(&sprite, &CelMap::new());

    let got: Vec<(BlendMode, bool, u8)> = read
        .layers
        .iter()
        .map(|l| (l.blend_mode, l.visible, l.opacity))
        .collect();
    let want: Vec<(BlendMode, bool, u8)> = originals
        .iter()
        .map(|l| (l.blend_mode, l.visible, l.opacity))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn group_hierarchy_round_trips_via_child_level() {
    // Aseprite's child_level encodes group depth in a flat layer list,
    // and Pincel's parent links must survive a write→read pass.
    //
    //   group "outer"        depth 0
    //     image "child_a"    depth 1   parent = outer
    //     group "inner"      depth 1   parent = outer
    //       image "child_b"  depth 2   parent = inner
    //   image "sibling"      depth 0   parent = None
    let outer = Layer::group(LayerId::new(0), "outer");
    let mut child_a = Layer::image(LayerId::new(1), "child_a");
    child_a.parent = Some(LayerId::new(0));
    let mut inner = Layer::group(LayerId::new(2), "inner");
    inner.parent = Some(LayerId::new(0));
    let mut child_b = Layer::image(LayerId::new(3), "child_b");
    child_b.parent = Some(LayerId::new(2));
    let sibling = Layer::image(LayerId::new(4), "sibling");

    let sprite = Sprite::builder(2, 2)
        .add_layer(outer)
        .add_layer(child_a)
        .add_layer(inner)
        .add_layer(child_b)
        .add_layer(sibling)
        .add_frame(Frame::new(100))
        .build()
        .unwrap();

    let mut cels = CelMap::new();
    let opaque_2x2 = rgba_buffer(2, 2, flat_pixels(&[rgba(0, 0, 0, 255); 4]));
    cels.insert(Cel {
        layer: LayerId::new(1),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(opaque_2x2.clone()),
        user_data: None,
        z_index: 0,
    });
    cels.insert(Cel {
        layer: LayerId::new(3),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(opaque_2x2.clone()),
        user_data: None,
        z_index: 0,
    });
    cels.insert(Cel {
        layer: LayerId::new(4),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(opaque_2x2),
        user_data: None,
        z_index: 0,
    });

    let AsepriteReadOutput { sprite, .. } = round_trip(&sprite, &cels);

    assert_eq!(sprite.layers.len(), 5);
    assert_eq!(sprite.layers[0].kind, LayerKind::Group);
    assert_eq!(sprite.layers[0].parent, None);
    assert_eq!(sprite.layers[1].parent, Some(LayerId::new(0)));
    assert_eq!(sprite.layers[2].kind, LayerKind::Group);
    assert_eq!(sprite.layers[2].parent, Some(LayerId::new(0)));
    assert_eq!(sprite.layers[3].parent, Some(LayerId::new(2)));
    assert_eq!(
        sprite.layers[4].parent, None,
        "depth-0 sibling after a nested group should have no parent",
    );
}

#[test]
fn linked_cel_within_range_round_trips() {
    // Two source frames and two links that point at different ones, so
    // neither "always frame 0" nor "the previous frame" passes.
    let sprite = Sprite::builder(2, 2)
        .add_layer(Layer::image(LayerId::new(0), "only"))
        .add_frame(Frame::new(100))
        .add_frame(Frame::new(100))
        .add_frame(Frame::new(100))
        .add_frame(Frame::new(100))
        .build()
        .unwrap();
    let mut cels = CelMap::new();
    for (frame, shade) in [(0, 0), (1, 90)] {
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(frame),
            position: (0, 0),
            opacity: 255,
            data: CelData::Image(rgba_buffer(2, 2, flat_pixels(&[rgba(shade, 0, 0, 255); 4]))),
            user_data: None,
            z_index: 0,
        });
    }
    for (frame, target) in [(2, 0), (3, 1)] {
        cels.insert(Cel {
            layer: LayerId::new(0),
            frame: FrameIndex::new(frame),
            position: (0, 0),
            opacity: 255,
            data: CelData::Linked(FrameIndex::new(target)),
            user_data: None,
            z_index: 0,
        });
    }

    let AsepriteReadOutput { cels, .. } = round_trip(&sprite, &cels);
    for (frame, target) in [(2, 0), (3, 1)] {
        let linked = cels
            .get(LayerId::new(0), FrameIndex::new(frame))
            .expect("linked cel present");
        assert_eq!(
            linked.data,
            CelData::Linked(FrameIndex::new(target)),
            "frame {frame} links to frame {target}"
        );
    }
}

#[test]
fn tags_round_trip_with_directions() {
    let sprite = Sprite::builder(2, 2)
        .add_layer(Layer::image(LayerId::new(0), "L0"))
        .add_frame(Frame::new(50))
        .add_frame(Frame::new(60))
        .add_frame(Frame::new(70))
        .add_tag(Tag {
            name: "idle".into(),
            from: FrameIndex::new(0),
            to: FrameIndex::new(1),
            direction: TagDirection::Forward,
            color: Rgba {
                r: 200,
                g: 30,
                b: 30,
                a: 255,
            },
            repeats: 0,
            user_data: None,
            raw_direction: None,
        })
        .add_tag(Tag {
            name: "wave".into(),
            from: FrameIndex::new(1),
            to: FrameIndex::new(2),
            direction: TagDirection::Pingpong,
            color: Rgba {
                r: 30,
                g: 60,
                b: 220,
                a: 255,
            },
            repeats: 3,
            user_data: None,
            raw_direction: None,
        })
        .build()
        .unwrap();
    let cels = CelMap::new();

    let AsepriteReadOutput { sprite, .. } = round_trip(&sprite, &cels);
    assert_eq!(sprite.tags.len(), 2);
    assert_eq!(sprite.tags[0].name, "idle");
    assert_eq!(sprite.tags[0].direction, TagDirection::Forward);
    assert_eq!(sprite.tags[0].from, FrameIndex::new(0));
    assert_eq!(sprite.tags[0].to, FrameIndex::new(1));
    assert_eq!(
        sprite.tags[0].color,
        Rgba {
            r: 200,
            g: 30,
            b: 30,
            a: 255
        }
    );
    assert_eq!(sprite.tags[1].name, "wave");
    assert_eq!(sprite.tags[1].direction, TagDirection::Pingpong);
    assert_eq!(sprite.tags[1].repeats, 3);
    assert_eq!(
        sprite.tags[1].color,
        Rgba {
            r: 30,
            g: 60,
            b: 220,
            a: 255
        }
    );
}

#[test]
fn tags_round_trip_every_direction_range_color_and_repeat_count() {
    let tag = |name: &str, from: u32, to: u32, direction, color, repeats| Tag {
        name: name.into(),
        from: FrameIndex::new(from),
        to: FrameIndex::new(to),
        direction,
        color,
        repeats,
        user_data: None,
        raw_direction: None,
    };
    let tags = vec![
        tag(
            "fwd",
            0,
            1,
            TagDirection::Forward,
            Rgba::new(1, 2, 3, 255),
            4,
        ),
        tag(
            "rev",
            1,
            3,
            TagDirection::Reverse,
            Rgba::new(40, 50, 60, 255),
            0,
        ),
        tag(
            "pp",
            2,
            2,
            TagDirection::Pingpong,
            Rgba::new(70, 80, 90, 255),
            2,
        ),
        tag(
            "pp-rev",
            0,
            3,
            TagDirection::PingpongReverse,
            Rgba::new(250, 128, 7, 255),
            9,
        ),
    ];
    let mut builder = Sprite::builder(2, 2).add_layer(Layer::image(LayerId::new(0), "L0"));
    for _ in 0..4 {
        builder = builder.add_frame(Frame::new(100));
    }
    for t in &tags {
        builder = builder.add_tag(t.clone());
    }
    let sprite = builder.build().expect("sprite builds");

    let AsepriteReadOutput { sprite: read, .. } = round_trip(&sprite, &CelMap::new());

    assert_eq!(read.tags, tags);
}

#[test]
fn palette_round_trips_colors_and_names() {
    let palette = Palette::from_entries(vec![
        PaletteEntry::new(Rgba::new(10, 20, 30, 255)),
        PaletteEntry::with_name(Rgba::new(200, 100, 50, 128), "ink"),
        PaletteEntry::new(Rgba::new(0, 0, 0, 0)),
    ]);
    let sprite = Sprite::builder(1, 1)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_frame(Frame::default())
        .palette(palette.clone())
        .build()
        .unwrap();
    let cels = CelMap::new();

    let AsepriteReadOutput { sprite: read, .. } = round_trip(&sprite, &cels);
    assert_eq!(read.palette, palette);
}

#[test]
fn tilemap_round_trips_layer_tileset_and_cel() {
    // Two-tile tileset: tile 0 transparent (Aseprite convention), tile 1
    // four red pixels.
    let tile0 = TileImage {
        pixels: PixelBuffer::empty(2, 2, ColorMode::Rgba),
    };
    let tile1 = TileImage {
        pixels: PixelBuffer {
            width: 2,
            height: 2,
            color_mode: ColorMode::Rgba,
            data: [255, 0, 0, 255].repeat(4),
        },
    };
    let tileset = Tileset {
        id: TilesetId::new(0),
        name: "ts".into(),
        tile_size: (2, 2),
        tiles: vec![tile0, tile1],
        base_index: 1,
        external_file: None,
        user_data: None,
        tile_user_data: Default::default(),
    };

    let mut tile_2 = TileRef::new(1);
    tile_2.flip_x = true;
    let mut tile_3 = TileRef::new(0);
    tile_3.flip_y = true;
    let tilemap_cel_tiles = vec![TileRef::new(0), TileRef::new(1), tile_2, tile_3];

    let sprite = Sprite::builder(8, 8)
        .add_layer(Layer::image(LayerId::new(0), "img"))
        .add_layer(Layer::tilemap(LayerId::new(1), "tiles", TilesetId::new(0)))
        .add_tileset(tileset)
        .add_frame(Frame::new(100))
        .build()
        .unwrap();

    let mut cels = CelMap::new();
    cels.insert(Cel {
        layer: LayerId::new(1),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 200,
        data: CelData::Tilemap {
            grid_w: 2,
            grid_h: 2,
            tiles: tilemap_cel_tiles,
        },
        user_data: None,
        z_index: 0,
    });

    let AsepriteReadOutput { sprite, cels } = round_trip(&sprite, &cels);

    // Tilemap layer survived with its tileset id.
    assert_eq!(sprite.layers.len(), 2);
    assert_eq!(sprite.layers[0].kind, LayerKind::Image);
    assert_eq!(
        sprite.layers[1].kind,
        LayerKind::Tilemap {
            tileset_id: TilesetId::new(0)
        },
    );

    // Tileset round-tripped with both tiles intact.
    assert_eq!(sprite.tilesets.len(), 1);
    let ts = &sprite.tilesets[0];
    assert_eq!(ts.id, TilesetId::new(0));
    assert_eq!(ts.name, "ts");
    assert_eq!(ts.tile_size, (2, 2));
    assert_eq!(ts.base_index, 1);
    assert_eq!(ts.tile_count(), 2);
    assert!(ts.tile(0).unwrap().pixels.data.iter().all(|&b| b == 0));
    assert_eq!(ts.tile(1).unwrap().pixels.data, [255, 0, 0, 255].repeat(4));

    // Tilemap cel round-tripped with per-cell flip flags.
    let cel = cels
        .get(LayerId::new(1), FrameIndex::new(0))
        .expect("tilemap cel present after round-trip");
    assert_eq!(cel.opacity, 200);
    let CelData::Tilemap {
        grid_w,
        grid_h,
        tiles,
    } = &cel.data
    else {
        panic!("expected tilemap cel, got {:?}", cel.data);
    };
    assert_eq!(*grid_w, 2);
    assert_eq!(*grid_h, 2);
    assert_eq!(tiles.len(), 4);
    assert_eq!(tiles[0].tile_id, 0);
    assert_eq!(tiles[1].tile_id, 1);
    assert!(!tiles[1].flip_x && !tiles[1].flip_y);
    assert_eq!(tiles[2].tile_id, 1);
    assert!(tiles[2].flip_x);
    assert!(!tiles[2].flip_y);
    assert_eq!(tiles[3].tile_id, 0);
    assert!(!tiles[3].flip_x);
    assert!(tiles[3].flip_y);
}

#[test]
fn tilemap_round_trips_rotate_90_flag() {
    let tileset = Tileset {
        id: TilesetId::new(2),
        name: "ts".into(),
        tile_size: (4, 4),
        tiles: vec![TileImage {
            pixels: PixelBuffer::empty(4, 4, ColorMode::Rgba),
        }],
        base_index: 1,
        external_file: None,
        user_data: None,
        tile_user_data: Default::default(),
    };
    let mut rotated = TileRef::new(0);
    rotated.rotate_90 = true;
    let sprite = Sprite::builder(4, 4)
        .add_layer(Layer::tilemap(LayerId::new(0), "tiles", TilesetId::new(2)))
        .add_tileset(tileset)
        .add_frame(Frame::new(100))
        .build()
        .unwrap();
    let mut cels = CelMap::new();
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Tilemap {
            grid_w: 1,
            grid_h: 1,
            tiles: vec![rotated],
        },
        user_data: None,
        z_index: 0,
    });
    let AsepriteReadOutput { cels, .. } = round_trip(&sprite, &cels);
    let cel = cels
        .get(LayerId::new(0), FrameIndex::new(0))
        .expect("rotated tilemap cel present");
    let CelData::Tilemap { tiles, .. } = &cel.data else {
        panic!("expected tilemap cel");
    };
    assert!(tiles[0].rotate_90);
}

#[test]
fn tilemap_round_trips_non_square_tiles_and_grid_on_its_own_tileset() {
    // Two tilesets with distinct ids, names and non-square tile sizes;
    // each tilemap layer points at the *other* one in file order, and the
    // cel grid is non-square with every flag combination present.
    let tileset = |id: u32, name: &str, (w, h): (u32, u32), count: u8| {
        let mut ts = Tileset::new(TilesetId::new(id), name, (w, h));
        ts.tiles = (0..count)
            .map(|t| TileImage {
                pixels: rgba_buffer(
                    w,
                    h,
                    (0..w * h * 4).map(|i| (i as u8) ^ (t * 40 + 1)).collect(),
                ),
            })
            .collect();
        ts
    };
    let tile = |id: u32, flip_x: bool, flip_y: bool, rotate_90: bool| TileRef {
        tile_id: id,
        flip_x,
        flip_y,
        rotate_90,
    };
    let tiles = vec![
        tile(0, false, false, false),
        tile(2, true, false, false),
        tile(1, false, true, false),
        tile(2, false, false, true),
        tile(1, true, true, true),
        tile(0, true, false, true),
    ];
    let sprite = Sprite::builder(12, 8)
        .add_layer(Layer::tilemap(LayerId::new(0), "a", TilesetId::new(9)))
        .add_layer(Layer::tilemap(LayerId::new(1), "b", TilesetId::new(4)))
        .add_tileset(tileset(4, "four", (3, 2), 2))
        .add_tileset(tileset(9, "nine", (2, 4), 3))
        .add_frame(Frame::new(100))
        .build()
        .expect("sprite builds");
    let mut cels = CelMap::new();
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Tilemap {
            grid_w: 3,
            grid_h: 2,
            tiles: tiles.clone(),
        },
        user_data: None,
        z_index: 0,
    });

    let AsepriteReadOutput {
        sprite: read,
        cels: read_cels,
    } = round_trip(&sprite, &cels);

    assert_eq!(
        read.layers[0].kind,
        LayerKind::Tilemap {
            tileset_id: TilesetId::new(9)
        }
    );
    assert_eq!(
        read.layers[1].kind,
        LayerKind::Tilemap {
            tileset_id: TilesetId::new(4)
        }
    );
    assert_eq!(read.tilesets, sprite.tilesets, "names, tile sizes, pixels");
    let cel = read_cels
        .get(LayerId::new(0), FrameIndex::new(0))
        .expect("tilemap cel present");
    assert_eq!(
        cel.data,
        CelData::Tilemap {
            grid_w: 3,
            grid_h: 2,
            tiles,
        }
    );
}

#[test]
fn slices_round_trip_plain_and_nine_patch_with_pivot() {
    // Editor ids that are neither 0-based nor in file order: the reader
    // must hand out fresh ids 0, 1 by position.
    let plain = Slice {
        id: SliceId::new(7),
        name: "hitbox".into(),
        color: Rgba::new(200, 30, 40, 255),
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::new(1, 2, 3, 4),
            center: None,
            pivot: None,
        }],
        user_data: None,
    };
    let panel = Slice {
        id: SliceId::new(3),
        name: "panel".into(),
        color: Rgba::new(10, 20, 30, 128),
        keys: vec![
            SliceKey {
                frame: FrameIndex::new(0),
                bounds: Rect::new(0, 0, 16, 16),
                center: Some(Rect::new(4, 4, 8, 8)),
                pivot: Some((2, 3)),
            },
            SliceKey {
                frame: FrameIndex::new(1),
                bounds: Rect::new(1, 1, 14, 14),
                center: Some(Rect::new(3, 3, 8, 8)),
                pivot: Some((-1, -1)),
            },
        ],
        user_data: None,
    };
    let sprite = Sprite::builder(16, 16)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_frame(Frame::new(100))
        .add_frame(Frame::new(100))
        .add_slice(plain.clone())
        .add_slice(panel.clone())
        .build()
        .unwrap();
    let cels = CelMap::new();

    let AsepriteReadOutput { sprite, .. } = round_trip(&sprite, &cels);

    assert_eq!(sprite.slices.len(), 2);
    // Slice IDs are reassigned by appearance order on read; the on-disk
    // format does not carry the editor-only id.
    assert_eq!(sprite.slices[0].id, SliceId::new(0));
    assert_eq!(sprite.slices[1].id, SliceId::new(1));
    assert_eq!(sprite.slices[0].name, plain.name);
    assert_eq!(sprite.slices[0].keys, plain.keys);
    assert_eq!(sprite.slices[1].name, panel.name);
    assert_eq!(sprite.slices[1].keys, panel.keys);
    // Overlay color round-trips through the trailing User Data chunk
    // (0x2020), including a non-opaque alpha.
    assert_eq!(sprite.slices[0].color, plain.color);
    assert_eq!(sprite.slices[1].color, panel.color);
}

#[test]
fn reordered_layers_round_trip_with_cels_intact() {
    // M13.4: a reorder permutes `sprite.layers` but keeps each
    // `LayerId`; the write adapter maps ids → current position, so a
    // round-trip must preserve z-order *and* keep every cel attached to
    // its own layer. On reload ids are renumbered by position (the
    // format stores no editor id), so we assert by name + pixel content.
    let mut bg = Layer::image(LayerId::new(0), "bg");
    bg.opacity = 255;
    let fg = Layer::image(LayerId::new(1), "fg");
    let sprite = Sprite::builder(2, 2)
        .add_layer(bg) // z 0 (bottom)
        .add_layer(fg) // z 1 (top)
        .add_frame(Frame::new(100))
        .build()
        .unwrap();
    let mut cels = CelMap::new();
    // bg: red at (0,0); fg: green at (1,1) — distinct, non-overlapping.
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(rgba_buffer(
            2,
            2,
            flat_pixels(&[
                rgba(200, 0, 0, 255),
                rgba(0, 0, 0, 0),
                rgba(0, 0, 0, 0),
                rgba(0, 0, 0, 0),
            ]),
        )),
        user_data: None,
        z_index: 0,
    });
    cels.insert(Cel {
        layer: LayerId::new(1),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(rgba_buffer(
            2,
            2,
            flat_pixels(&[
                rgba(0, 0, 0, 0),
                rgba(0, 0, 0, 0),
                rgba(0, 0, 0, 0),
                rgba(0, 200, 0, 255),
            ]),
        )),
        user_data: None,
        z_index: 0,
    });

    // Move bg (id 0) up past fg → layer order becomes [fg, bg].
    let mut sprite = sprite;
    MoveLayer::new(LayerId::new(0), MoveDirection::Up)
        .apply(&mut sprite, &mut cels)
        .expect("reorder");
    assert_eq!(
        sprite
            .layers
            .iter()
            .map(|l| l.name.as_str())
            .collect::<Vec<_>>(),
        vec!["fg", "bg"],
    );

    let AsepriteReadOutput { sprite, cels } = round_trip(&sprite, &cels);

    // Z-order survived: bottom is now "fg", top is "bg".
    assert_eq!(sprite.layers[0].name, "fg");
    assert_eq!(sprite.layers[1].name, "bg");
    // Ids are renumbered by position: fg (was id 1) is now 0, bg 1.
    assert_eq!(sprite.layers[0].id, LayerId::new(0));
    assert_eq!(sprite.layers[1].id, LayerId::new(1));

    // Each cel still belongs to its own (reordered) layer.
    let fg_id = sprite.layers[0].id;
    let bg_id = sprite.layers[1].id;
    let CelData::Image(fg_buf) = &cels.get(fg_id, FrameIndex::new(0)).expect("fg cel").data else {
        panic!("fg cel is not an image");
    };
    let CelData::Image(bg_buf) = &cels.get(bg_id, FrameIndex::new(0)).expect("bg cel").data else {
        panic!("bg cel is not an image");
    };
    // fg kept its green at (1,1); bg kept its red at (0,0). Byte offset
    // of (1,1) in a 2-wide RGBA buffer is (2 + 1) * 4 = 12.
    assert_eq!(&fg_buf.data[12..16], &[0, 200, 0, 255]);
    assert_eq!(&bg_buf.data[0..4], &[200, 0, 0, 255]);
}

#[test]
fn zero_frame_sprite_fails_to_write_instead_of_emitting_invalid_file() {
    // `SpriteBuilder::build` permits a frameless sprite (a valid
    // in-memory Pincel object), but the Aseprite format requires at least
    // one frame. The writer must reject it with a clear error rather than
    // emit a header that readers can't parse.
    let sprite = Sprite::builder(8, 8)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .build()
        .unwrap();
    assert!(sprite.frames.is_empty());
    let cels = CelMap::new();
    let mut bytes = Vec::new();
    let err = write_aseprite(&sprite, &cels, &mut bytes).unwrap_err();
    assert!(
        err.to_string().contains("zero frames"),
        "expected a zero-frames error, got: {err}"
    );
    assert!(bytes.is_empty(), "nothing written on rejection");
}

#[test]
fn header_flags_z_index_and_user_data_round_trip() {
    let note = |text: &str, c: u8| UserData {
        text: Some(text.into()),
        color: Some(Rgba::new(c, c, c, 255)),
        properties: None,
    };
    let mut layer = Layer::image(LayerId::new(0), "bg");
    // Lock movement + background + collapsed + reference.
    layer.extra_flags = 0x4 | 0x8 | 0x20 | 0x40;
    layer.user_data = Some(note("layer", 1));
    let mut tag_a = Tag::new("a", FrameIndex::new(0), FrameIndex::new(0));
    tag_a.user_data = None;
    let mut tag_b = Tag::new("b", FrameIndex::new(0), FrameIndex::new(0));
    tag_b.user_data = Some(note("tag b", 2));
    let mut sprite = Sprite::builder(2, 2)
        .add_layer(layer)
        .add_frame(Frame::new(100))
        .add_tag(tag_a)
        .add_tag(tag_b)
        .palette(Palette::from_entries(vec![PaletteEntry::new(Rgba::BLACK)]))
        .build()
        .expect("sprite builds");
    sprite.metadata.pixel_ratio = (2, 1);
    sprite.metadata.grid = Some(Rect::new(3, 4, 8, 6));
    sprite.metadata.user_data = Some(note("sprite", 3));

    let mut cels = CelMap::new();
    let mut cel = Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        rgba_buffer(1, 1, flat_pixels(&[rgba(1, 2, 3, 255)])),
    );
    cel.z_index = -3;
    cel.user_data = Some(note("cel", 4));
    cels.insert(cel.clone());

    let out = round_trip(&sprite, &cels);
    let s = &out.sprite;
    assert_eq!(s.metadata.pixel_ratio, (2, 1));
    assert_eq!(s.metadata.grid, Some(Rect::new(3, 4, 8, 6)));
    assert_eq!(s.metadata.user_data, Some(note("sprite", 3)));
    assert_eq!(s.layers[0].extra_flags, 0x4 | 0x8 | 0x20 | 0x40);
    assert_eq!(s.layers[0].user_data, Some(note("layer", 1)));
    assert_eq!(s.tags[0].user_data, None);
    assert_eq!(s.tags[1].user_data, Some(note("tag b", 2)));
    let back = out
        .cels
        .get(LayerId::new(0), FrameIndex::new(0))
        .expect("cel");
    assert_eq!(back.z_index, -3);
    assert_eq!(back.user_data, Some(note("cel", 4)));
}

/// A property block with a nested map (type 0x12), which carries no key
/// DWORD of its own: `{ hp: int32 7, stats: { str: string "x" } }`.
fn nested_properties() -> Vec<u8> {
    let mut b = Vec::new();
    let string = |b: &mut Vec<u8>, s: &str| {
        b.extend_from_slice(&(s.len() as u16).to_le_bytes());
        b.extend_from_slice(s.as_bytes());
    };
    b.extend_from_slice(&0u32.to_le_bytes()); // size, patched below
    b.extend_from_slice(&1u32.to_le_bytes()); // one map
    b.extend_from_slice(&0u32.to_le_bytes()); // key 0
    b.extend_from_slice(&2u32.to_le_bytes()); // two properties
    string(&mut b, "hp");
    b.extend_from_slice(&0x0006u16.to_le_bytes());
    b.extend_from_slice(&7i32.to_le_bytes());
    string(&mut b, "stats");
    b.extend_from_slice(&0x0012u16.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    string(&mut b, "str");
    b.extend_from_slice(&0x000Du16.to_le_bytes());
    string(&mut b, "x");
    let len = b.len() as u32;
    b[..4].copy_from_slice(&len.to_le_bytes());
    b
}

#[test]
fn user_data_properties_and_unknown_tag_direction_round_trip() {
    let props = nested_properties();
    let with_props = |text: Option<&str>| UserData {
        text: text.map(str::to_string),
        color: None,
        properties: Some(props.clone()),
    };
    let mut layer = Layer::image(LayerId::new(0), "bg");
    layer.user_data = Some(with_props(Some("layer")));
    let mut tag = Tag::new("future", FrameIndex::new(0), FrameIndex::new(0));
    tag.raw_direction = Some(5);
    tag.user_data = Some(with_props(None));
    let mut sprite = Sprite::builder(2, 2)
        .add_layer(layer)
        .add_frame(Frame::new(100))
        .add_tag(tag)
        .palette(Palette::from_entries(vec![PaletteEntry::new(Rgba::BLACK)]))
        .build()
        .expect("sprite builds");
    sprite.metadata.user_data = Some(with_props(None));
    let mut cels = CelMap::new();
    let mut cel = Cel::image(
        LayerId::new(0),
        FrameIndex::new(0),
        rgba_buffer(1, 1, flat_pixels(&[rgba(1, 2, 3, 255)])),
    );
    cel.user_data = Some(with_props(Some("cel")));
    cels.insert(cel);

    // Twice through, so the second read sees Pincel's own output.
    let once = round_trip(&sprite, &cels);
    let out = round_trip(&once.sprite, &once.cels);
    let s = &out.sprite;
    assert_eq!(s.layers[0].user_data, Some(with_props(Some("layer"))));
    assert_eq!(s.metadata.user_data, Some(with_props(None)));
    assert_eq!(s.tags[0].user_data, Some(with_props(None)));
    assert_eq!(s.tags[0].raw_direction, Some(5));
    assert_eq!(s.tags[0].direction, TagDirection::Forward);
    let back = out
        .cels
        .get(LayerId::new(0), FrameIndex::new(0))
        .expect("cel");
    assert_eq!(back.user_data, Some(with_props(Some("cel"))));
}

#[test]
fn layer_moved_into_a_group_keeps_its_parent_through_a_round_trip() {
    use pincel_core::{Command, LayerPlacement, MoveLayerTo};
    let mut group = Layer::group(LayerId::new(1), "grp");
    group.parent = None;
    let mut child = Layer::image(LayerId::new(2), "child");
    child.parent = Some(LayerId::new(1));
    let mut sprite = Sprite::builder(2, 2)
        .add_layer(Layer::image(LayerId::new(0), "loose"))
        .add_layer(group)
        .add_layer(child)
        .add_frame(Frame::new(100))
        .build()
        .expect("sprite builds");
    let mut cels = CelMap::new();
    MoveLayerTo::new(LayerId::new(0), LayerId::new(1), LayerPlacement::Into)
        .apply(&mut sprite, &mut cels)
        .expect("move into group");
    let out = round_trip(&sprite, &cels);
    let shape: Vec<(&str, Option<&str>)> = out
        .sprite
        .layers
        .iter()
        .map(|l| {
            let parent = l
                .parent
                .and_then(|p| out.sprite.layer(p))
                .map(|p| p.name.as_str());
            (l.name.as_str(), parent)
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            ("grp", None),
            ("child", Some("grp")),
            ("loose", Some("grp"))
        ]
    );
}

#[test]
fn external_files_tileset_tile_and_slice_user_data_round_trip() {
    let props = nested_properties();
    let ud = |text: &str, color: Option<Rgba>| UserData {
        text: Some(text.into()),
        color,
        properties: Some(props.clone()),
    };
    let tile = |v: u8| TileImage {
        pixels: PixelBuffer {
            width: 2,
            height: 2,
            color_mode: ColorMode::Rgba,
            data: [v, 0, 0, 255].repeat(4),
        },
    };
    let mut tileset = Tileset::new(TilesetId::new(0), "ts", (2, 2));
    tileset.tiles = vec![tile(0), tile(1), tile(2), tile(3)];
    tileset.user_data = Some(ud("tileset", Some(Rgba::new(9, 8, 7, 6))));
    // Sparse: tile 2 only; tiles 0, 1 and 3 have none.
    tileset
        .tile_user_data
        .insert(2, ud("tile 2", Some(Rgba::new(1, 2, 3, 4))));
    let slice = Slice {
        id: SliceId::new(0),
        name: "hitbox".into(),
        color: Rgba::new(200, 30, 40, 255),
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::new(1, 2, 3, 4),
            center: None,
            pivot: None,
        }],
        user_data: Some(ud("slice note", None)),
    };
    let mut sprite = Sprite::builder(8, 8)
        .add_layer(Layer::image(LayerId::new(0), "img"))
        .add_layer(Layer::tilemap(LayerId::new(1), "tiles", TilesetId::new(0)))
        .add_tileset(tileset)
        .add_slice(slice)
        .add_frame(Frame::new(100))
        .palette(Palette::from_entries(vec![PaletteEntry::new(Rgba::BLACK)]))
        .build()
        .expect("sprite builds");
    sprite.metadata.external_files = vec![
        ExternalFile {
            id: 1,
            kind: 2,
            name: "my-extension".into(),
        },
        ExternalFile {
            id: 2,
            kind: 3,
            name: "tile-ext".into(),
        },
    ];
    let cels = CelMap::new();

    let once = round_trip(&sprite, &cels);
    let out = round_trip(&once.sprite, &once.cels);
    let s = &out.sprite;
    assert_eq!(s.metadata.external_files, sprite.metadata.external_files);
    let ts = &s.tilesets[0];
    assert_eq!(
        ts.user_data,
        Some(ud("tileset", Some(Rgba::new(9, 8, 7, 6))))
    );
    assert_eq!(ts.tile_user_data.len(), 1);
    assert_eq!(
        ts.tile_user_data.get(&2),
        Some(&ud("tile 2", Some(Rgba::new(1, 2, 3, 4))))
    );
    assert_eq!(s.slices[0].color, Rgba::new(200, 30, 40, 255));
    assert_eq!(s.slices[0].user_data, Some(ud("slice note", None)));
}
