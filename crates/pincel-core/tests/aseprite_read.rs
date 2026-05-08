//! Integration tests for the Aseprite read adapter (M4).
//!
//! Fixtures are built by hand at the byte level so the test does not depend
//! on the (unfinished) writer crate. The format reference is
//! <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>;
//! see `docs/specs/pincel.md` §7.1.

use pincel_core::{
    AsepriteReadOutput, BlendMode, CelData, ColorMode, FrameIndex, LayerId, LayerKind,
    read_aseprite,
};

const ASEPRITE_FILE_MAGIC: u16 = 0xA5E0;
const ASEPRITE_FRAME_MAGIC: u16 = 0xF1FA;
const CHUNK_TYPE_LAYER: u16 = 0x2004;
const CHUNK_TYPE_CEL: u16 = 0x2005;
const COLOR_DEPTH_RGBA: u16 = 32;

/// Hand-crafted .aseprite builder. Produces uncompressed RGBA files only —
/// just enough surface area for the M4 adapter test (replaced by the M5
/// `aseprite-writer` crate once it lands).
struct FixtureBuilder {
    width: u16,
    height: u16,
    layers: Vec<FixtureLayer>,
    frames: Vec<FixtureFrame>,
}

struct FixtureLayer {
    name: String,
    visible: bool,
    opacity: u8,
    blend_mode: u16,
    /// 0 = Normal, 1 = Group, 2 = Tilemap. Defaults to Normal.
    layer_type: u16,
    /// Indentation depth used by Aseprite to encode group nesting.
    child_level: u16,
}

impl FixtureLayer {
    fn image(name: &str) -> Self {
        Self {
            name: name.to_string(),
            visible: true,
            opacity: 255,
            blend_mode: 0,
            layer_type: 0,
            child_level: 0,
        }
    }

    fn group(name: &str) -> Self {
        Self {
            layer_type: 1,
            ..Self::image(name)
        }
    }

    fn at_depth(mut self, depth: u16) -> Self {
        self.child_level = depth;
        self
    }
}

struct FixtureFrame {
    duration_ms: u16,
    cels: Vec<FixtureCel>,
}

enum FixtureCel {
    Image {
        layer_index: u16,
        x: i16,
        y: i16,
        opacity: u8,
        width: u16,
        height: u16,
        /// Raw RGBA8 row-major pixels, length must equal `width * height * 4`.
        pixels: Vec<u8>,
    },
    Linked {
        layer_index: u16,
        x: i16,
        y: i16,
        opacity: u8,
        frame_position: u16,
    },
}

impl FixtureBuilder {
    fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            layers: Vec::new(),
            frames: Vec::new(),
        }
    }

    fn layer(mut self, layer: FixtureLayer) -> Self {
        self.layers.push(layer);
        self
    }

    fn frame(mut self, frame: FixtureFrame) -> Self {
        self.frames.push(frame);
        self
    }

    fn build(self) -> Vec<u8> {
        let mut frame_blobs = Vec::with_capacity(self.frames.len());
        let is_first_frame_layers_attached = !self.layers.is_empty();
        for (i, frame) in self.frames.iter().enumerate() {
            let layers = if i == 0 && is_first_frame_layers_attached {
                self.layers.as_slice()
            } else {
                &[]
            };
            frame_blobs.push(encode_frame(frame, layers));
        }

        let body_len: u32 = frame_blobs.iter().map(|b| b.len() as u32).sum();
        let file_size = 128u32 + body_len;

        let mut out = Vec::with_capacity(file_size as usize);
        encode_header(
            &mut out,
            file_size,
            self.frames.len() as u16,
            self.width,
            self.height,
        );
        for blob in frame_blobs {
            out.extend_from_slice(&blob);
        }
        debug_assert_eq!(
            out.len() as u32,
            file_size,
            "computed file size mismatches encoded length"
        );
        out
    }
}

fn encode_header(out: &mut Vec<u8>, file_size: u32, frames: u16, width: u16, height: u16) {
    let start = out.len();
    out.extend_from_slice(&file_size.to_le_bytes()); // file_size
    out.extend_from_slice(&ASEPRITE_FILE_MAGIC.to_le_bytes()); // magic
    out.extend_from_slice(&frames.to_le_bytes()); // frame count
    out.extend_from_slice(&width.to_le_bytes()); // width
    out.extend_from_slice(&height.to_le_bytes()); // height
    out.extend_from_slice(&COLOR_DEPTH_RGBA.to_le_bytes()); // color depth
    out.extend_from_slice(&1u32.to_le_bytes()); // flags: layer opacity valid
    out.extend_from_slice(&0u16.to_le_bytes()); // deprecated speed
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    out.push(0); // transparent palette index
    out.extend_from_slice(&[0u8; 3]); // reserved
    out.extend_from_slice(&0u16.to_le_bytes()); // num colors
    out.push(1); // pixel width
    out.push(1); // pixel height
    out.extend_from_slice(&0i16.to_le_bytes()); // grid x
    out.extend_from_slice(&0i16.to_le_bytes()); // grid y
    out.extend_from_slice(&0u16.to_le_bytes()); // grid width
    out.extend_from_slice(&0u16.to_le_bytes()); // grid height
    out.extend_from_slice(&[0u8; 84]); // reserved tail
    debug_assert_eq!(out.len() - start, 128);
}

fn encode_frame(frame: &FixtureFrame, layers: &[FixtureLayer]) -> Vec<u8> {
    let mut chunks = Vec::new();
    for layer in layers {
        chunks.extend_from_slice(&encode_layer_chunk(layer));
    }
    for cel in &frame.cels {
        chunks.extend_from_slice(&encode_cel_chunk(cel));
    }
    let chunk_count = (layers.len() + frame.cels.len()) as u32;

    let frame_size = 16u32 + chunks.len() as u32;
    let mut out = Vec::with_capacity(frame_size as usize);
    out.extend_from_slice(&frame_size.to_le_bytes());
    out.extend_from_slice(&ASEPRITE_FRAME_MAGIC.to_le_bytes());
    // chunk_count_old; the new dword field below takes precedence when non-zero.
    out.extend_from_slice(
        &u16::try_from(chunk_count.min(u32::from(u16::MAX)))
            .unwrap()
            .to_le_bytes(),
    );
    out.extend_from_slice(&frame.duration_ms.to_le_bytes());
    out.extend_from_slice(&[0u8; 2]); // reserved
    out.extend_from_slice(&chunk_count.to_le_bytes());
    out.extend_from_slice(&chunks);
    out
}

fn encode_layer_chunk(layer: &FixtureLayer) -> Vec<u8> {
    let name_bytes = layer.name.as_bytes();
    // body: flags(2)+type(2)+childlvl(2)+def_w(2)+def_h(2)+blend(2)+opacity(1)+reserved(3)+name(2+N)
    let body_len = 16 + 2 + name_bytes.len();
    let chunk_size = 4u32 + 2 + body_len as u32;
    let mut out = Vec::with_capacity(chunk_size as usize);
    out.extend_from_slice(&chunk_size.to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_LAYER.to_le_bytes());

    let mut flags: u16 = 0;
    if layer.visible {
        flags |= 0x0001;
    }
    flags |= 0x0002; // editable
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&layer.layer_type.to_le_bytes());
    out.extend_from_slice(&layer.child_level.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // deprecated default w
    out.extend_from_slice(&0u16.to_le_bytes()); // deprecated default h
    out.extend_from_slice(&layer.blend_mode.to_le_bytes());
    out.push(layer.opacity);
    out.extend_from_slice(&[0u8; 3]); // reserved

    out.extend_from_slice(&u16::try_from(name_bytes.len()).unwrap().to_le_bytes());
    out.extend_from_slice(name_bytes);

    debug_assert_eq!(out.len() as u32, chunk_size);
    out
}

fn encode_cel_chunk(cel: &FixtureCel) -> Vec<u8> {
    match cel {
        FixtureCel::Image {
            layer_index,
            x,
            y,
            opacity,
            width,
            height,
            pixels,
        } => {
            assert_eq!(
                pixels.len(),
                usize::from(*width) * usize::from(*height) * 4,
                "fixture cel pixel buffer must be RGBA8 row-major"
            );
            // body: layer_idx(2)+x(2)+y(2)+opacity(1)+cel_type(2)+z(2)+reserved(5)+w(2)+h(2)+pixels
            let body_len = 20 + pixels.len();
            let chunk_size = 4u32 + 2 + body_len as u32;
            let mut out = Vec::with_capacity(chunk_size as usize);
            out.extend_from_slice(&chunk_size.to_le_bytes());
            out.extend_from_slice(&CHUNK_TYPE_CEL.to_le_bytes());
            out.extend_from_slice(&layer_index.to_le_bytes());
            out.extend_from_slice(&x.to_le_bytes());
            out.extend_from_slice(&y.to_le_bytes());
            out.push(*opacity);
            out.extend_from_slice(&0u16.to_le_bytes()); // cel type: 0 = raw image
            out.extend_from_slice(&0i16.to_le_bytes()); // z-index
            out.extend_from_slice(&[0u8; 5]); // reserved
            out.extend_from_slice(&width.to_le_bytes());
            out.extend_from_slice(&height.to_le_bytes());
            out.extend_from_slice(pixels);
            debug_assert_eq!(out.len() as u32, chunk_size);
            out
        }
        FixtureCel::Linked {
            layer_index,
            x,
            y,
            opacity,
            frame_position,
        } => {
            // body: layer_idx(2)+x(2)+y(2)+opacity(1)+cel_type(2)+z(2)+reserved(5)+frame_pos(2)
            let body_len = 18;
            let chunk_size = 4u32 + 2 + body_len as u32;
            let mut out = Vec::with_capacity(chunk_size as usize);
            out.extend_from_slice(&chunk_size.to_le_bytes());
            out.extend_from_slice(&CHUNK_TYPE_CEL.to_le_bytes());
            out.extend_from_slice(&layer_index.to_le_bytes());
            out.extend_from_slice(&x.to_le_bytes());
            out.extend_from_slice(&y.to_le_bytes());
            out.push(*opacity);
            out.extend_from_slice(&1u16.to_le_bytes()); // cel type: 1 = linked
            out.extend_from_slice(&0i16.to_le_bytes()); // z-index
            out.extend_from_slice(&[0u8; 5]); // reserved
            out.extend_from_slice(&frame_position.to_le_bytes());
            debug_assert_eq!(out.len() as u32, chunk_size);
            out
        }
    }
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    [r, g, b, a]
}

fn flat_pixels(colors: &[[u8; 4]]) -> Vec<u8> {
    colors.iter().flatten().copied().collect()
}

#[test]
fn handcrafted_single_layer_single_frame_round_trip() {
    let pixels = flat_pixels(&[
        rgba(255, 0, 0, 255),
        rgba(0, 255, 0, 255),
        rgba(0, 0, 255, 255),
        rgba(255, 255, 255, 128),
    ]);
    let bytes = FixtureBuilder::new(2, 2)
        .layer(FixtureLayer {
            opacity: 200,
            ..FixtureLayer::image("Background")
        })
        .frame(FixtureFrame {
            duration_ms: 120,
            cels: vec![FixtureCel::Image {
                layer_index: 0,
                x: 0,
                y: 0,
                opacity: 255,
                width: 2,
                height: 2,
                pixels: pixels.clone(),
            }],
        })
        .build();

    let AsepriteReadOutput { sprite, cels } =
        read_aseprite(&bytes).expect("hand-crafted fixture should parse");

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
    let bg = flat_pixels(&[rgba(10, 20, 30, 255); 4]);
    let fg = flat_pixels(&[rgba(200, 100, 50, 128)]);
    let bytes = FixtureBuilder::new(4, 4)
        .layer(FixtureLayer::image("bg"))
        .layer(FixtureLayer {
            visible: false,
            opacity: 180,
            blend_mode: 1, // Multiply
            ..FixtureLayer::image("fx")
        })
        .frame(FixtureFrame {
            duration_ms: 100,
            cels: vec![
                FixtureCel::Image {
                    layer_index: 0,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    width: 2,
                    height: 2,
                    pixels: bg,
                },
                FixtureCel::Image {
                    layer_index: 1,
                    x: 1,
                    y: 2,
                    opacity: 200,
                    width: 1,
                    height: 1,
                    pixels: fg,
                },
            ],
        })
        .build();

    let AsepriteReadOutput { sprite, cels } =
        read_aseprite(&bytes).expect("multi-layer fixture should parse");

    assert_eq!(sprite.layers.len(), 2);
    assert_eq!(sprite.layers[1].blend_mode, BlendMode::Multiply);
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
fn unsupported_blend_mode_is_rejected() {
    let bytes = FixtureBuilder::new(1, 1)
        .layer(FixtureLayer {
            blend_mode: 0x1337, // bogus
            ..FixtureLayer::image("x")
        })
        .frame(FixtureFrame {
            duration_ms: 16,
            cels: vec![FixtureCel::Image {
                layer_index: 0,
                x: 0,
                y: 0,
                opacity: 255,
                width: 1,
                height: 1,
                pixels: rgba(0, 0, 0, 255).to_vec(),
            }],
        })
        .build();
    let err = read_aseprite(&bytes).expect_err("bogus blend mode should fail");
    assert!(
        format!("{err:?}").contains("UnsupportedBlendMode"),
        "expected blend-mode error, got {err:?}"
    );
}

#[test]
fn group_hierarchy_is_reconstructed_from_child_level() {
    // Layer order in Aseprite is bottom-up; the child_level field encodes
    // group depth. We exercise:
    //   group "outer"        depth 0
    //     image "child_a"    depth 1   parent = outer
    //     group "inner"      depth 1   parent = outer
    //       image "child_b"  depth 2   parent = inner
    //   image "sibling"      depth 0   parent = None
    let bytes = FixtureBuilder::new(2, 2)
        .layer(FixtureLayer::group("outer"))
        .layer(FixtureLayer::image("child_a").at_depth(1))
        .layer(FixtureLayer::group("inner").at_depth(1))
        .layer(FixtureLayer::image("child_b").at_depth(2))
        .layer(FixtureLayer::image("sibling"))
        .frame(FixtureFrame {
            duration_ms: 100,
            cels: vec![
                FixtureCel::Image {
                    layer_index: 1,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    width: 2,
                    height: 2,
                    pixels: flat_pixels(&[rgba(0, 0, 0, 255); 4]),
                },
                FixtureCel::Image {
                    layer_index: 3,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    width: 2,
                    height: 2,
                    pixels: flat_pixels(&[rgba(255, 255, 255, 255); 4]),
                },
                FixtureCel::Image {
                    layer_index: 4,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    width: 2,
                    height: 2,
                    pixels: flat_pixels(&[rgba(127, 127, 127, 255); 4]),
                },
            ],
        })
        .build();

    let AsepriteReadOutput { sprite, .. } =
        read_aseprite(&bytes).expect("nested-group fixture should parse");

    assert_eq!(sprite.layers.len(), 5);
    assert_eq!(sprite.layers[0].kind, LayerKind::Group);
    assert_eq!(sprite.layers[0].parent, None);
    assert_eq!(sprite.layers[1].parent, Some(LayerId::new(0)));
    assert_eq!(sprite.layers[2].kind, LayerKind::Group);
    assert_eq!(sprite.layers[2].parent, Some(LayerId::new(0)));
    assert_eq!(sprite.layers[3].parent, Some(LayerId::new(2)));
    assert_eq!(
        sprite.layers[4].parent, None,
        "depth-0 sibling after a nested group should have no parent"
    );
}

#[test]
fn linked_cel_pointing_past_frame_count_is_rejected_by_loader() {
    // The aseprite-loader validates linked cels via its internal image map
    // before the adapter sees them; this fixture exercises that boundary so
    // that "out-of-range link → user-visible error" stays a pinned guarantee.
    let bytes = FixtureBuilder::new(2, 2)
        .layer(FixtureLayer::image("only"))
        .frame(FixtureFrame {
            duration_ms: 100,
            cels: vec![FixtureCel::Image {
                layer_index: 0,
                x: 0,
                y: 0,
                opacity: 255,
                width: 2,
                height: 2,
                pixels: flat_pixels(&[rgba(0, 0, 0, 255); 4]),
            }],
        })
        .frame(FixtureFrame {
            duration_ms: 100,
            cels: vec![FixtureCel::Linked {
                layer_index: 0,
                x: 0,
                y: 0,
                opacity: 255,
                frame_position: 5,
            }],
        })
        .build();
    let err = read_aseprite(&bytes).expect_err("out-of-range linked frame should fail");
    assert!(
        matches!(err, pincel_core::CodecError::Parse(_)),
        "expected loader-level Parse error, got {err:?}"
    );
}

#[test]
fn linked_cel_within_range_is_preserved() {
    let bytes = FixtureBuilder::new(2, 2)
        .layer(FixtureLayer::image("only"))
        .frame(FixtureFrame {
            duration_ms: 100,
            cels: vec![FixtureCel::Image {
                layer_index: 0,
                x: 0,
                y: 0,
                opacity: 255,
                width: 2,
                height: 2,
                pixels: flat_pixels(&[rgba(0, 0, 0, 255); 4]),
            }],
        })
        .frame(FixtureFrame {
            duration_ms: 100,
            cels: vec![FixtureCel::Linked {
                layer_index: 0,
                x: 0,
                y: 0,
                opacity: 255,
                frame_position: 0,
            }],
        })
        .build();

    let AsepriteReadOutput { cels, .. } =
        read_aseprite(&bytes).expect("linked-cel fixture should parse");
    let linked = cels
        .get(LayerId::new(0), FrameIndex::new(1))
        .expect("frame 1 should hold a linked cel");
    assert!(
        matches!(linked.data, CelData::Linked(idx) if idx == FrameIndex::new(0)),
        "expected CelData::Linked(0), got {:?}",
        linked.data
    );
}
