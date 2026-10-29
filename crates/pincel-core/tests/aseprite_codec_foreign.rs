//! Read-side tests against hand-assembled Aseprite byte streams: files
//! Pincel did not write, carrying chunks it does not model or content it
//! cannot load. See `docs/specs/aseprite-round-trip.md` (AC-14 … AC-17).
//!
//! Format reference:
//! <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use pincel_core::{CodecError, read_aseprite, write_aseprite};

// --- byte-level fixture helpers ---------------------------------------------

const COLOR_DEPTH_RGBA: u16 = 32;
const COLOR_DEPTH_GRAYSCALE: u16 = 16;
const COLOR_DEPTH_INDEXED: u16 = 8;

fn word(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn short(buf: &mut Vec<u8>, value: i16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn dword(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn string(buf: &mut Vec<u8>, value: &str) {
    word(buf, value.len() as u16);
    buf.extend_from_slice(value.as_bytes());
}

fn zeros(buf: &mut Vec<u8>, count: usize) {
    buf.extend(std::iter::repeat_n(0u8, count));
}

/// `DWORD size | WORD type | body`.
fn chunk(chunk_type: u16, body: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    dword(&mut buf, 6 + body.len() as u32);
    word(&mut buf, chunk_type);
    buf.extend_from_slice(body);
    buf
}

/// Layer chunk (`0x2004`) with a free layer type and blend mode code.
fn layer(name: &str, layer_type: u16, blend_mode: u16) -> Vec<u8> {
    let mut body = Vec::new();
    word(&mut body, 0x3); // visible | editable
    word(&mut body, layer_type);
    word(&mut body, 0); // child level
    word(&mut body, 0); // default width
    word(&mut body, 0); // default height
    word(&mut body, blend_mode);
    body.push(255);
    zeros(&mut body, 3);
    string(&mut body, name);
    if layer_type == 2 {
        dword(&mut body, 0); // tileset index
    }
    chunk(0x2004, &body)
}

/// The fixed cel header shared by every cel type.
fn cel_header(layer_index: u16, cel_type: u16) -> Vec<u8> {
    let mut body = Vec::new();
    word(&mut body, layer_index);
    short(&mut body, 1); // x
    short(&mut body, 2); // y
    body.push(255);
    word(&mut body, cel_type);
    short(&mut body, 0); // z-index
    zeros(&mut body, 5);
    body
}

/// Raw (uncompressed) image cel, Cel Type 0.
fn raw_cel(layer_index: u16, width: u16, height: u16, pixels: &[u8]) -> Vec<u8> {
    let mut body = cel_header(layer_index, 0);
    word(&mut body, width);
    word(&mut body, height);
    body.extend_from_slice(pixels);
    chunk(0x2005, &body)
}

/// Linked cel, Cel Type 1.
fn linked_cel(layer_index: u16, frame_position: u16) -> Vec<u8> {
    let mut body = cel_header(layer_index, 1);
    word(&mut body, frame_position);
    chunk(0x2005, &body)
}

/// A cel with a type code Pincel does not know, plus a few payload bytes.
fn unknown_type_cel(layer_index: u16, cel_type: u16) -> Vec<u8> {
    let mut body = cel_header(layer_index, cel_type);
    body.extend_from_slice(&[1, 2, 3, 4]);
    chunk(0x2005, &body)
}

/// Compressed tilemap cel, Cel Type 3, with a free `bits_per_tile`.
fn tilemap_cel(layer_index: u16, bits_per_tile: u16) -> Vec<u8> {
    let mut body = cel_header(layer_index, 3);
    word(&mut body, 1); // grid width
    word(&mut body, 1); // grid height
    word(&mut body, bits_per_tile);
    dword(&mut body, 0x1fff_ffff);
    dword(&mut body, 0x2000_0000);
    dword(&mut body, 0x4000_0000);
    dword(&mut body, 0x8000_0000);
    zeros(&mut body, 10);
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&[0u8; 4]).expect("zlib write");
    body.extend_from_slice(&enc.finish().expect("zlib finish"));
    chunk(0x2005, &body)
}

/// Tileset chunk (`0x2023`) that only points at an external file.
fn external_tileset(id: u32) -> Vec<u8> {
    let mut body = Vec::new();
    dword(&mut body, id);
    dword(&mut body, 0x1); // EXTERNAL_FILE, no inline TILES
    dword(&mut body, 4); // number of tiles
    word(&mut body, 8);
    word(&mut body, 8);
    short(&mut body, 1); // base index
    zeros(&mut body, 14);
    string(&mut body, "shared");
    dword(&mut body, 1); // external file id
    dword(&mut body, 0); // tileset id inside that file
    chunk(0x2023, &body)
}

/// New palette chunk (`0x2019`), one unnamed entry per color.
fn palette(colors: &[[u8; 4]]) -> Vec<u8> {
    let mut body = Vec::new();
    dword(&mut body, colors.len() as u32);
    dword(&mut body, 0);
    dword(&mut body, colors.len() as u32 - 1);
    zeros(&mut body, 8);
    for c in colors {
        word(&mut body, 0); // no name
        body.extend_from_slice(c);
    }
    chunk(0x2019, &body)
}

/// Legacy palette chunk (`0x0004` 0..255 or `0x0011` 0..63): one packet.
fn legacy_palette(chunk_type: u16, colors: &[[u8; 3]]) -> Vec<u8> {
    let mut body = Vec::new();
    word(&mut body, 1); // one packet
    body.push(0); // skip
    body.push(colors.len() as u8);
    for c in colors {
        body.extend_from_slice(c);
    }
    chunk(chunk_type, &body)
}

/// Color Profile chunk (`0x2007`), sRGB without a fixed gamma.
fn color_profile() -> Vec<u8> {
    let mut body = Vec::new();
    word(&mut body, 1); // sRGB
    word(&mut body, 0); // flags
    dword(&mut body, 0); // gamma
    zeros(&mut body, 8);
    chunk(0x2007, &body)
}

/// Frame envelope: 16-byte header, then the chunks.
fn frame(duration_ms: u16, chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    let mut buf = Vec::new();
    dword(&mut buf, 16 + body.len() as u32);
    word(&mut buf, 0xF1FA);
    word(&mut buf, chunks.len() as u16);
    word(&mut buf, duration_ms);
    zeros(&mut buf, 2);
    dword(&mut buf, chunks.len() as u32);
    buf.extend_from_slice(&body);
    buf
}

/// 128-byte header plus the given frames.
fn file(color_depth: u16, frames: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = frames.concat();
    let mut buf = Vec::new();
    dword(&mut buf, 128 + body.len() as u32);
    word(&mut buf, 0xA5E0);
    word(&mut buf, frames.len() as u16);
    word(&mut buf, 4); // width
    word(&mut buf, 3); // height
    word(&mut buf, color_depth);
    dword(&mut buf, 0x1);
    word(&mut buf, 100);
    zeros(&mut buf, 8);
    buf.push(0); // transparent index
    zeros(&mut buf, 3);
    word(&mut buf, 0); // color count
    buf.push(1);
    buf.push(1);
    short(&mut buf, 0);
    short(&mut buf, 0);
    word(&mut buf, 16);
    word(&mut buf, 16);
    zeros(&mut buf, 84);
    buf.extend_from_slice(&body);
    buf
}

/// A one-frame RGBA file holding one image layer and the given cel.
fn one_layer_file(cel: Vec<u8>) -> Vec<u8> {
    file(COLOR_DEPTH_RGBA, &[frame(100, &[layer("L0", 0, 0), cel])])
}

/// Every chunk type in `bytes`, in file order (envelope walk).
fn chunk_types(bytes: &[u8]) -> Vec<u16> {
    let read_u32 = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    let mut types = Vec::new();
    let mut frame_at = 128;
    while frame_at < bytes.len() {
        let frame_end = frame_at + read_u32(frame_at) as usize;
        let mut at = frame_at + 16;
        while at < frame_end {
            types.push(u16::from_le_bytes([bytes[at + 4], bytes[at + 5]]));
            at += read_u32(at) as usize;
        }
        frame_at = frame_end;
    }
    types
}

const RED: [u8; 4] = [255, 0, 0, 255];

// --- AC-14: unmodeled chunks are dropped ------------------------------------

#[test]
fn read_aseprite_with_unmodeled_chunks_drops_them_on_open_and_save() {
    let pixels = RED.repeat(2);
    let modeled = vec![
        layer("L0", 0, 0),
        palette(&[[10, 20, 30, 255], [40, 50, 60, 128]]),
        raw_cel(0, 2, 1, &pixels),
    ];
    let mut with_extras = modeled.clone();
    with_extras.splice(
        2..2,
        [
            legacy_palette(0x0004, &[[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
            legacy_palette(0x0011, &[[11, 12, 13]]),
            color_profile(),
            chunk(0x7F00, &[0xDE, 0xAD, 0xBE, 0xEF]),
        ],
    );
    let clean = file(COLOR_DEPTH_RGBA, &[frame(100, &modeled)]);
    let foreign = file(COLOR_DEPTH_RGBA, &[frame(100, &with_extras)]);
    assert!(chunk_types(&foreign).contains(&0x2007), "fixture sanity");

    // Open: the extra chunks leave no trace in the document.
    let opened = read_aseprite(&foreign).expect("file with unmodeled chunks opens");
    assert_eq!(opened, read_aseprite(&clean).expect("clean file opens"));

    // Save: none of them is written back.
    let mut saved = Vec::new();
    write_aseprite(&opened.sprite, &opened.cels, &mut saved).expect("save");
    let types = chunk_types(&saved);
    for dropped in [0x0004, 0x0011, 0x2007, 0x7F00] {
        assert!(
            !types.contains(&dropped),
            "chunk {dropped:#06x} must not be saved, got {types:#06x?}"
        );
    }
}

// --- AC-15: not an Aseprite file --------------------------------------------

#[test]
fn read_aseprite_non_aseprite_bytes_returns_parse_error() {
    // A PNG signature followed by padding: plenty of bytes, wrong format.
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.resize(256, 0);
    let err = read_aseprite(&png).expect_err("PNG bytes are not an Aseprite file");
    assert!(matches!(err, CodecError::Parse(_)), "got {err:?}");
}

// --- AC-16: indexed / grayscale ---------------------------------------------

#[test]
fn read_aseprite_indexed_or_grayscale_returns_unsupported_color_mode() {
    // Aseprite always writes a palette; the loader needs it for indexed.
    let chunks = [layer("L0", 0, 0), palette(&[[0, 0, 0, 255], RED])];
    for depth in [COLOR_DEPTH_INDEXED, COLOR_DEPTH_GRAYSCALE] {
        let bytes = file(depth, &[frame(100, &chunks)]);
        let err = read_aseprite(&bytes).expect_err("non-RGBA file is refused");
        assert!(
            matches!(err, CodecError::UnsupportedColorMode),
            "depth {depth}: got {err:?}"
        );
        assert!(err.to_string().contains("unsupported aseprite color mode"));
    }
    // Control: the same file in RGBA opens.
    let rgba = file(COLOR_DEPTH_RGBA, &[frame(100, &chunks)]);
    read_aseprite(&rgba).expect("RGBA control opens");
}

// --- AC-17: valid file, unloadable content ----------------------------------

#[test]
fn read_aseprite_external_only_tileset_returns_tileset_unsupported() {
    let bytes = file(
        COLOR_DEPTH_RGBA,
        &[frame(100, &[layer("tiles", 2, 0), external_tileset(3)])],
    );
    let err = read_aseprite(&bytes).expect_err("external tileset is refused");
    assert!(
        matches!(err, CodecError::TilesetUnsupported { id: 3, .. }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("external-file"), "{err}");
}

#[test]
fn read_aseprite_unknown_layer_type_returns_unsupported_layer_kind() {
    let bytes = file(COLOR_DEPTH_RGBA, &[frame(100, &[layer("odd", 7, 0)])]);
    let err = read_aseprite(&bytes).expect_err("unknown layer type is refused");
    assert!(
        matches!(err, CodecError::UnsupportedLayerKind { kind: 7 }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("layer kind: 7"), "{err}");
}

#[test]
fn read_aseprite_unknown_blend_mode_returns_unsupported_blend_mode() {
    let bytes = file(COLOR_DEPTH_RGBA, &[frame(100, &[layer("L0", 0, 99)])]);
    let err = read_aseprite(&bytes).expect_err("unknown blend mode is refused");
    assert!(
        matches!(err, CodecError::UnsupportedBlendMode { mode: 99 }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("blend mode: 99"), "{err}");
}

#[test]
fn read_aseprite_unknown_cel_type_returns_unsupported_cel_kind() {
    let err = read_aseprite(&one_layer_file(unknown_type_cel(0, 9)))
        .expect_err("unknown cel type is refused");
    assert!(
        matches!(err, CodecError::UnsupportedCelKind { kind: 9 }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("cel kind: 9"), "{err}");
}

#[test]
fn read_aseprite_linked_cel_past_last_frame_returns_linked_frame_not_found() {
    // Frame 0 holds the source image, frame 1 links to frame 5 of 2.
    let bytes = file(
        COLOR_DEPTH_RGBA,
        &[
            frame(100, &[layer("L0", 0, 0), raw_cel(0, 1, 1, &RED)]),
            frame(100, &[linked_cel(0, 5)]),
        ],
    );
    let err = read_aseprite(&bytes).expect_err("dangling link is refused");
    assert!(
        matches!(err, CodecError::LinkedFrameNotFound { index: 5 }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("frame 5"), "{err}");
}

#[test]
fn read_aseprite_cel_on_missing_layer_returns_parse_error_naming_layer_index() {
    // `aseprite-loader` cross-checks the cel's layer index while parsing,
    // so the failure surfaces as its parse error, which names the problem.
    let err = read_aseprite(&one_layer_file(raw_cel(4, 1, 1, &RED)))
        .expect_err("cel on layer 4 of 1 is refused");
    let CodecError::Parse(message) = &err else {
        panic!("expected a parse error, got {err:?}");
    };
    assert!(message.contains("LayerIndexOutOfBounds"), "{message}");
    // Control: the same cel on layer 0 opens.
    read_aseprite(&one_layer_file(raw_cel(0, 1, 1, &RED))).expect("cel on layer 0 opens");
}

#[test]
fn read_aseprite_tilemap_cel_not_32_bits_returns_bits_per_tile_unsupported() {
    let tilemap = |bits| {
        file(
            COLOR_DEPTH_RGBA,
            &[frame(
                100,
                &[
                    layer("tiles", 2, 0),
                    // Tileset 0 with neither flag: zero tiles, still resolves.
                    {
                        let mut body = Vec::new();
                        dword(&mut body, 0);
                        dword(&mut body, 0);
                        dword(&mut body, 0);
                        word(&mut body, 8);
                        word(&mut body, 8);
                        short(&mut body, 1);
                        zeros(&mut body, 14);
                        string(&mut body, "ts");
                        chunk(0x2023, &body)
                    },
                    tilemap_cel(0, bits),
                ],
            )],
        )
    };
    let err = read_aseprite(&tilemap(16)).expect_err("16-bit tilemap cel is refused");
    assert!(
        matches!(err, CodecError::TilemapBitsPerTileUnsupported { bits: 16 }),
        "got {err:?}"
    );
    assert!(err.to_string().contains("bits_per_tile 16"), "{err}");
    // Control: the same cel at 32 bits per tile opens.
    read_aseprite(&tilemap(32)).expect("32-bit tilemap cel opens");
}
