//! `compose()` follows linked cels — feature spec
//! `docs/specs/linked-cels-group-compose.md` AC-01 … AC-07, design
//! `docs/specs/pincel.md` §3.2 / §4.2. A link must render exactly like a
//! copy of its target cel, so most tests compare against that copy.

use pincel_core::{
    AtlasOptions, BlendMode, Cel, CelData, CelMap, ColorMode, ComposeRequest, Frame, FrameIndex,
    Layer, LayerId, OnionSkin, PixelBuffer, RenderError, Sprite, TileImage, TileRef, Tileset,
    TilesetId, compose, export_atlas_png, export_frame_png, read_aseprite, write_aseprite,
};

fn solid(w: u32, h: u32, rgba: [u8; 4]) -> PixelBuffer {
    let mut buf = PixelBuffer::empty(w, h, ColorMode::Rgba);
    for px in buf.data.as_chunks_mut::<4>().0 {
        *px = rgba;
    }
    buf
}

fn compose_req(
    sprite: &Sprite,
    cels: &CelMap,
    req: &ComposeRequest,
) -> Result<Vec<u8>, RenderError> {
    let mut out = Vec::new();
    compose(sprite, cels, req, &mut out)?;
    Ok(out)
}

fn compose_frame(sprite: &Sprite, cels: &CelMap, frame: u32) -> Result<Vec<u8>, RenderError> {
    compose_req(
        sprite,
        cels,
        &ComposeRequest::full(FrameIndex::new(frame), sprite.width, sprite.height),
    )
}

/// Replace every linked cel by a copy of its target (position, opacity and
/// data), the way AC-01 defines a link.
fn unlink(cels: &CelMap) -> CelMap {
    let mut out = CelMap::new();
    for (_, cel) in cels.iter() {
        let mut c = cel.clone();
        if let CelData::Linked(target) = cel.data {
            let t = cels.get(cel.layer, target).expect("test link has a target");
            c.position = t.position;
            c.opacity = t.opacity;
            c.data = t.data.clone();
        }
        out.insert(c);
    }
    out
}

/// A 4×3, 4-frame sprite: an opaque grey background layer, an image layer
/// at opacity 200 / Multiply whose frames 1 and 3 link to frame 0 (each
/// with its own position and opacity, which the link must ignore), and a
/// tilemap layer whose frame 2 links to frame 0.
fn linked_sprite() -> (Sprite, CelMap) {
    let mut art = Layer::image(LayerId::new(1), "art");
    art.opacity = 200;
    art.blend_mode = BlendMode::Multiply;
    let mut ts = Tileset::new(TilesetId::new(0), "tiles", (1, 1));
    ts.tiles.push(TileImage {
        pixels: PixelBuffer::empty(1, 1, ColorMode::Rgba),
    });
    ts.tiles.push(TileImage {
        pixels: solid(1, 1, [0, 200, 100, 255]),
    });
    let mut b = Sprite::builder(4, 3)
        .add_layer(Layer::image(LayerId::new(0), "bg"))
        .add_layer(art)
        .add_layer(Layer::tilemap(LayerId::new(2), "map", TilesetId::new(0)))
        .add_tileset(ts);
    for _ in 0..4 {
        b = b.add_frame(Frame::new(100));
    }
    let sprite = b.build().expect("sprite builds");

    let mut cels = CelMap::new();
    for f in 0..4 {
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(f),
            solid(4, 3, [128, 128, 128, 255]),
        ));
    }
    let mut target = Cel::image(
        LayerId::new(1),
        FrameIndex::new(0),
        solid(2, 2, [255, 64, 0, 255]),
    );
    target.position = (1, 1);
    target.opacity = 160;
    cels.insert(target);
    for f in [1, 3] {
        let mut link = Cel::image(LayerId::new(1), FrameIndex::new(f), solid(1, 1, [0; 4]));
        link.data = CelData::Linked(FrameIndex::new(0));
        link.position = (0, 0);
        link.opacity = 255;
        cels.insert(link);
    }
    let tilemap = |frame: u32, data: CelData| Cel {
        layer: LayerId::new(2),
        frame: FrameIndex::new(frame),
        position: (2, 0),
        opacity: 255,
        data,
        user_data: None,
        z_index: 0,
    };
    let mut t0 = tilemap(
        0,
        CelData::Tilemap {
            grid_w: 2,
            grid_h: 1,
            tiles: vec![TileRef::new(1), TileRef::EMPTY],
        },
    );
    t0.opacity = 100;
    cels.insert(t0);
    let mut t2 = tilemap(2, CelData::Linked(FrameIndex::new(0)));
    t2.position = (0, 2);
    cels.insert(t2);
    (sprite, cels)
}

#[test]
fn compose_linked_cel_renders_like_a_copy_of_its_target() {
    // AC-01: image and tilemap links render as their target (data,
    // position, cel opacity) under the linking layer's opacity and blend.
    let (sprite, cels) = linked_sprite();
    let copies = unlink(&cels);
    for f in 0..4 {
        let got = compose_frame(&sprite, &cels, f).expect("linked frame composes");
        let want = compose_frame(&sprite, &copies, f).expect("copy composes");
        assert_eq!(got, want, "frame {f}");
    }
    // Frame 1 equals frame 0 on the art layer: the link's own (0, 0)
    // position and opacity 255 are not used.
    let f0 = compose_frame(&sprite, &cels, 0).unwrap();
    let f1 = compose_frame(&sprite, &cels, 1).unwrap();
    assert_eq!(&f1[0..4], &[128, 128, 128, 255], "(0, 0) is not covered");
    let at = |buf: &[u8], x: usize, y: usize| buf[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4].to_vec();
    assert_eq!(at(&f1, 1, 1), at(&f0, 1, 1));
    assert_ne!(
        at(&f1, 1, 1),
        vec![128, 128, 128, 255],
        "the target renders"
    );
    // Frame 2's tilemap link renders the tile at the target's (2, 0).
    let f2 = compose_frame(&sprite, &cels, 2).unwrap();
    assert_eq!(at(&f2, 2, 0), at(&f0, 2, 0));
    assert_eq!(at(&f2, 0, 2), vec![128, 128, 128, 255]);
}

#[test]
fn compose_onion_skin_renders_linked_neighbours() {
    // AC-02: frame 2 with frames 1 (linked) and 3 (linked) as onion
    // neighbours equals the same request on the unlinked copy.
    let (sprite, cels) = linked_sprite();
    let copies = unlink(&cels);
    let req = ComposeRequest {
        onion_skin: Some(OnionSkin {
            previous_alpha: 200,
            next_alpha: 150,
            frames_back: 2,
            frames_forward: 1,
        }),
        ..ComposeRequest::full(FrameIndex::new(2), 4, 3)
    };
    let mut no_bg = sprite.clone();
    no_bg.layers[0].visible = false;
    for s in [&sprite, &no_bg] {
        let got = compose_req(s, &cels, &req).expect("onion with links composes");
        let want = compose_req(s, &copies, &req).expect("onion with copies composes");
        assert_eq!(got, want);
    }
}

fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("png header parses");
    let size = reader.output_buffer_size().expect("known output size");
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).expect("frame decodes");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

#[test]
fn export_frames_with_linked_cels_equal_compose() {
    // AC-03: PNG frames and atlas cells of linked frames equal compose().
    let (sprite, cels) = linked_sprite();
    let atlas = export_atlas_png(&sprite, &cels, &AtlasOptions::new(4)).expect("atlas exports");
    let (aw, _, sheet) = decode_png(&atlas.png);
    for f in 0..4u32 {
        let want = compose_frame(&sprite, &cels, f).unwrap();
        let png = export_frame_png(&sprite, &cels, FrameIndex::new(f)).expect("frame exports");
        assert_eq!(decode_png(&png), (4, 3, want.clone()), "frame {f}");
        for y in 0..3usize {
            let row = (y * aw as usize + f as usize * 4) * 4;
            assert_eq!(
                &sheet[row..row + 16],
                &want[y * 16..y * 16 + 16],
                "atlas {f} row {y}"
            );
        }
    }
}

#[test]
fn linked_cels_compose_the_same_after_aseprite_round_trip() {
    // AC-04: write → read keeps every frame's pixels. The writer only
    // writes image links, so the tilemap link is dropped for this test.
    let (sprite, mut cels) = linked_sprite();
    cels.remove(LayerId::new(2), FrameIndex::new(2));
    let mut bytes = Vec::new();
    write_aseprite(&sprite, &cels, &mut bytes).expect("writer accepts links");
    let back = read_aseprite(&bytes).expect("reader accepts our output");
    assert!(matches!(
        back.cels
            .get(LayerId::new(1), FrameIndex::new(3))
            .map(|c| &c.data),
        Some(CelData::Linked(_))
    ));
    for f in 0..4 {
        assert_eq!(
            compose_frame(&back.sprite, &back.cels, f).expect("read-back composes"),
            compose_frame(&sprite, &cels, f).expect("original composes"),
            "frame {f}"
        );
    }
}

#[test]
fn compose_linked_cel_with_missing_target_fails_naming_frames() {
    // AC-05: a target frame past the sprite, and a target frame without a
    // cel on the layer.
    let (sprite, mut cels) = linked_sprite();
    let mut link = Cel::image(LayerId::new(1), FrameIndex::new(1), solid(1, 1, [0; 4]));
    link.data = CelData::Linked(FrameIndex::new(9));
    cels.insert(link);
    assert_eq!(
        compose_frame(&sprite, &cels, 1),
        Err(RenderError::LinkedCelTargetMissing {
            layer: LayerId::new(1),
            frame: FrameIndex::new(1),
            target: FrameIndex::new(9),
        })
    );

    let (sprite, mut cels) = linked_sprite();
    cels.remove(LayerId::new(1), FrameIndex::new(0));
    assert_eq!(
        compose_frame(&sprite, &cels, 3),
        Err(RenderError::LinkedCelTargetMissing {
            layer: LayerId::new(1),
            frame: FrameIndex::new(3),
            target: FrameIndex::new(0),
        })
    );
}

#[test]
fn compose_chained_or_self_linked_cel_fails_without_looping() {
    // AC-06: frame 3 → frame 1 → frame 0 is a chain; frame 1 → frame 1
    // links to itself.
    let (sprite, mut cels) = linked_sprite();
    let mut chained = Cel::image(LayerId::new(1), FrameIndex::new(3), solid(1, 1, [0; 4]));
    chained.data = CelData::Linked(FrameIndex::new(1));
    cels.insert(chained);
    assert_eq!(
        compose_frame(&sprite, &cels, 3),
        Err(RenderError::LinkedCelChained {
            layer: LayerId::new(1),
            frame: FrameIndex::new(3),
            target: FrameIndex::new(1),
        })
    );

    let (sprite, mut cels) = linked_sprite();
    let mut own = Cel::image(LayerId::new(1), FrameIndex::new(1), solid(1, 1, [0; 4]));
    own.data = CelData::Linked(FrameIndex::new(1));
    cels.insert(own);
    assert_eq!(
        compose_frame(&sprite, &cels, 1),
        Err(RenderError::LinkedCelChained {
            layer: LayerId::new(1),
            frame: FrameIndex::new(1),
            target: FrameIndex::new(1),
        })
    );
}

#[test]
fn compose_linked_target_of_wrong_kind_fails_with_type_mismatch() {
    // AC-07: an image layer linking to tile data, and a tilemap layer
    // linking to image data.
    let (sprite, mut cels) = linked_sprite();
    let mut tile_target = Cel::image(LayerId::new(1), FrameIndex::new(0), solid(1, 1, [0; 4]));
    tile_target.data = CelData::Tilemap {
        grid_w: 1,
        grid_h: 1,
        tiles: vec![TileRef::new(1)],
    };
    cels.insert(tile_target);
    assert_eq!(
        compose_frame(&sprite, &cels, 1),
        Err(RenderError::CelTypeMismatch {
            layer: LayerId::new(1),
            frame: FrameIndex::new(1),
        })
    );

    let (sprite, mut cels) = linked_sprite();
    cels.insert(Cel::image(
        LayerId::new(2),
        FrameIndex::new(0),
        solid(1, 1, [1, 2, 3, 255]),
    ));
    assert_eq!(
        compose_frame(&sprite, &cels, 2),
        Err(RenderError::CelTypeMismatch {
            layer: LayerId::new(2),
            frame: FrameIndex::new(2),
        })
    );
}
