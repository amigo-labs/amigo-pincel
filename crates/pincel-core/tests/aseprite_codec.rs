//! Integration tests for the Aseprite codec pair (M4 read + M5 write).
//!
//! Each test builds a Pincel [`Sprite`] / [`CelMap`], writes it to bytes
//! via [`write_aseprite`], reads the bytes back via [`read_aseprite`],
//! and asserts the result preserves the original document. The
//! round-trip is the canonical Phase 1 integration test for the codec
//! pair; see `docs/specs/pincel.md` §7.1.

use pincel_core::{
    AsepriteReadOutput, BlendMode, Cel, CelData, CelMap, ColorMode, Frame, FrameIndex, Layer,
    LayerId, LayerKind, PixelBuffer, Rect, Rgba, Slice, SliceId, SliceKey, Sprite, Tag,
    TagDirection, TileImage, TileRef, Tileset, TilesetId, read_aseprite, write_aseprite,
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
fn multi_layer_blend_modes_and_offsets_are_preserved() {
    let bg_pixels = flat_pixels(&[rgba(10, 20, 30, 255); 4]);
    let fx_pixels = flat_pixels(&[rgba(200, 100, 50, 128)]);

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
    });
    cels.insert(Cel {
        layer: LayerId::new(1),
        frame: FrameIndex::new(0),
        position: (1, 2),
        opacity: 200,
        data: CelData::Image(rgba_buffer(1, 1, fx_pixels)),
    });

    let AsepriteReadOutput { sprite, cels } = round_trip(&sprite, &cels);

    assert_eq!(sprite.layers.len(), 2);
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
    assert_eq!((buf.width, buf.height), (1, 1));
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
    });
    cels.insert(Cel {
        layer: LayerId::new(3),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(opaque_2x2.clone()),
    });
    cels.insert(Cel {
        layer: LayerId::new(4),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(opaque_2x2),
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
    let sprite = Sprite::builder(2, 2)
        .add_layer(Layer::image(LayerId::new(0), "only"))
        .add_frame(Frame::new(100))
        .add_frame(Frame::new(100))
        .build()
        .unwrap();
    let mut cels = CelMap::new();
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(0),
        position: (0, 0),
        opacity: 255,
        data: CelData::Image(rgba_buffer(2, 2, flat_pixels(&[rgba(0, 0, 0, 255); 4]))),
    });
    cels.insert(Cel {
        layer: LayerId::new(0),
        frame: FrameIndex::new(1),
        position: (0, 0),
        opacity: 255,
        data: CelData::Linked(FrameIndex::new(0)),
    });

    let AsepriteReadOutput { cels, .. } = round_trip(&sprite, &cels);
    let linked = cels
        .get(LayerId::new(0), FrameIndex::new(1))
        .expect("frame 1 should hold a linked cel");
    assert!(
        matches!(linked.data, CelData::Linked(idx) if idx == FrameIndex::new(0)),
        "expected CelData::Linked(0), got {:?}",
        linked.data,
    );
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
            color: Rgba::WHITE,
            repeats: 0,
        })
        .add_tag(Tag {
            name: "wave".into(),
            from: FrameIndex::new(1),
            to: FrameIndex::new(2),
            direction: TagDirection::Pingpong,
            color: Rgba::WHITE,
            repeats: 3,
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
    assert_eq!(sprite.tags[1].name, "wave");
    assert_eq!(sprite.tags[1].direction, TagDirection::Pingpong);
    assert_eq!(sprite.tags[1].repeats, 3);
}

// Palette round-trip is covered by `aseprite-writer`'s integration tests
// against `parse_raw_file`. The high-level `aseprite-loader` API used by
// `read_aseprite` drops RGBA palettes (see STATUS.md), so a write→read
// pass through this adapter cannot currently observe them.

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
fn slices_round_trip_plain_and_nine_patch_with_pivot() {
    let plain = Slice {
        id: SliceId::new(0),
        name: "hitbox".into(),
        color: Rgba::WHITE,
        keys: vec![SliceKey {
            frame: FrameIndex::new(0),
            bounds: Rect::new(1, 2, 3, 4),
            center: None,
            pivot: None,
        }],
    };
    let panel = Slice {
        id: SliceId::new(1),
        name: "panel".into(),
        color: Rgba::WHITE,
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
}
