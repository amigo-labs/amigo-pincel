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
}

struct FixtureFrame {
    duration_ms: u16,
    cels: Vec<FixtureCel>,
}

struct FixtureCel {
    layer_index: u16,
    x: i16,
    y: i16,
    opacity: u8,
    width: u16,
    height: u16,
    /// Raw RGBA8 row-major pixels, length must equal `width * height * 4`.
    pixels: Vec<u8>,
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
    out.extend_from_slice(&0u16.to_le_bytes()); // type: normal
    out.extend_from_slice(&0u16.to_le_bytes()); // child level
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
    assert_eq!(
        cel.pixels.len(),
        usize::from(cel.width) * usize::from(cel.height) * 4,
        "fixture cel pixel buffer must be RGBA8 row-major"
    );
    // body: layer_idx(2)+x(2)+y(2)+opacity(1)+cel_type(2)+z(2)+reserved(5)+w(2)+h(2)+pixels
    let body_len = 20 + cel.pixels.len();
    let chunk_size = 4u32 + 2 + body_len as u32;

    let mut out = Vec::with_capacity(chunk_size as usize);
    out.extend_from_slice(&chunk_size.to_le_bytes());
    out.extend_from_slice(&CHUNK_TYPE_CEL.to_le_bytes());
    out.extend_from_slice(&cel.layer_index.to_le_bytes());
    out.extend_from_slice(&cel.x.to_le_bytes());
    out.extend_from_slice(&cel.y.to_le_bytes());
    out.push(cel.opacity);
    out.extend_from_slice(&0u16.to_le_bytes()); // cel type: 0 = raw image
    out.extend_from_slice(&0i16.to_le_bytes()); // z-index
    out.extend_from_slice(&[0u8; 5]); // reserved
    out.extend_from_slice(&cel.width.to_le_bytes());
    out.extend_from_slice(&cel.height.to_le_bytes());
    out.extend_from_slice(&cel.pixels);

    debug_assert_eq!(out.len() as u32, chunk_size);
    out
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
            name: "Background".to_string(),
            visible: true,
            opacity: 200,
            blend_mode: 0,
        })
        .frame(FixtureFrame {
            duration_ms: 120,
            cels: vec![FixtureCel {
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
        .layer(FixtureLayer {
            name: "bg".to_string(),
            visible: true,
            opacity: 255,
            blend_mode: 0, // Normal
        })
        .layer(FixtureLayer {
            name: "fx".to_string(),
            visible: false,
            opacity: 180,
            blend_mode: 1, // Multiply
        })
        .frame(FixtureFrame {
            duration_ms: 100,
            cels: vec![
                FixtureCel {
                    layer_index: 0,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    width: 2,
                    height: 2,
                    pixels: bg,
                },
                FixtureCel {
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
            name: "x".to_string(),
            visible: true,
            opacity: 255,
            blend_mode: 0x1337, // bogus
        })
        .frame(FixtureFrame {
            duration_ms: 16,
            cels: vec![FixtureCel {
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
