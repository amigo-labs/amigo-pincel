//! Integration test for the M8.4 tilemap read path.
//!
//! The Phase 1 writer (M5) does not yet emit Tileset (`0x2023`) or
//! Compressed-Tilemap (Cel Type 3) chunks — that work lands in M8.5. To
//! exercise the read path before the writer catches up, this test
//! assembles a minimal Aseprite v1.3 byte stream by hand and runs it
//! through [`read_aseprite`].
//!
//! Fixture shape:
//!
//! - 8×8 RGBA canvas, one frame (100 ms)
//! - layer 0: image "img" (no cel — empty layer)
//! - layer 1: tilemap "tiles" with `tileset_index = 0`
//! - tileset 0: 2×2 tile size, 2 inline tiles (tile 0 transparent, tile
//!   1 a four-pixel red square)
//! - tilemap cel on layer 1: 2×2 grid, tiles
//!   `[0, 1, 1 | x_flip, 0 | y_flip]`
//!
//! Tested invariants:
//!
//! - `LayerType::Tilemap` (`0x2`) no longer raises `UnsupportedLayerKind`
//! - Tileset chunks are recovered from the raw chunk stream
//! - Cel Type 3 (`CelContent::CompressedTilemap`) hydrates into
//!   [`CelData::Tilemap`] with the bitmask-decoded `TileRef` fields
//!
//! Spec: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use pincel_core::{
    AsepriteReadOutput, CelData, ColorMode, FrameIndex, LayerId, LayerKind, TilesetId,
    read_aseprite,
};

// --- byte-level fixture helpers ---------------------------------------------

fn push_word(buf: &mut Vec<u8>, value: u16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn push_short(buf: &mut Vec<u8>, value: i16) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn push_dword(buf: &mut Vec<u8>, value: u32) {
    buf.extend_from_slice(&value.to_le_bytes());
}

fn push_string(buf: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    push_word(buf, bytes.len() as u16);
    buf.extend_from_slice(bytes);
}

fn zeros(buf: &mut Vec<u8>, count: usize) {
    buf.extend(std::iter::repeat_n(0u8, count));
}

fn zlib_compress(bytes: &[u8]) -> Vec<u8> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(bytes).expect("zlib write");
    enc.finish().expect("zlib finish")
}

/// Wrap a chunk body in its 6-byte envelope: `DWORD size | WORD type | body`.
fn chunk(chunk_type: u16, body: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(6 + body.len());
    let total = 6 + body.len() as u32;
    push_dword(&mut buf, total);
    push_word(&mut buf, chunk_type);
    buf.extend_from_slice(body);
    buf
}

fn layer_chunk_image(name: &str) -> Vec<u8> {
    let mut body = Vec::new();
    push_word(&mut body, 0x3); // flags: VISIBLE | EDITABLE
    push_word(&mut body, 0); // layer_type: Normal
    push_word(&mut body, 0); // child_level
    push_word(&mut body, 0); // default_width  (deprecated)
    push_word(&mut body, 0); // default_height (deprecated)
    push_word(&mut body, 0); // blend_mode: Normal
    body.push(255); // opacity
    zeros(&mut body, 3); // reserved
    push_string(&mut body, name);
    chunk(0x2004, &body)
}

fn layer_chunk_tilemap(name: &str, tileset_index: u32) -> Vec<u8> {
    let mut body = Vec::new();
    push_word(&mut body, 0x3); // flags: VISIBLE | EDITABLE
    push_word(&mut body, 2); // layer_type: Tilemap
    push_word(&mut body, 0); // child_level
    push_word(&mut body, 0); // default_width
    push_word(&mut body, 0); // default_height
    push_word(&mut body, 0); // blend_mode: Normal
    body.push(200); // opacity
    zeros(&mut body, 3); // reserved
    push_string(&mut body, name);
    push_dword(&mut body, tileset_index);
    chunk(0x2004, &body)
}

/// Build a tileset chunk (`0x2023`) with inline tile data.
///
/// `tiles` is laid out per-tile, top-to-bottom; each tile is
/// `tile_w * tile_h * 4` bytes (RGBA8). Tile 0 is conventionally
/// transparent (all zero), but this helper writes whatever the caller
/// provides verbatim.
fn tileset_chunk(
    id: u32,
    name: &str,
    tile_w: u16,
    tile_h: u16,
    base_index: i16,
    tile_pixels: &[u8],
    n_tiles: u32,
) -> Vec<u8> {
    let mut body = Vec::new();
    push_dword(&mut body, id);
    // flags: TILES (2) | TILE_0_EMPTY (4) = 6
    push_dword(&mut body, 6);
    push_dword(&mut body, n_tiles);
    push_word(&mut body, tile_w);
    push_word(&mut body, tile_h);
    push_short(&mut body, base_index);
    zeros(&mut body, 14); // reserved
    push_string(&mut body, name);
    // TILES flag set: `DWORD compressed_size | BYTE[] zlib_data`
    let compressed = zlib_compress(tile_pixels);
    push_dword(&mut body, compressed.len() as u32);
    body.extend_from_slice(&compressed);
    chunk(0x2023, &body)
}

const TILE_ID_MASK: u32 = 0x1fffffff;
const Y_FLIP_MASK: u32 = 0x20000000;
const X_FLIP_MASK: u32 = 0x40000000;
const DIAG_FLIP_MASK: u32 = 0x80000000;

/// Build a Cel chunk (`0x2005`) of Cel Type 3 (Compressed Tilemap).
///
/// `tiles_raw` is a row-major list of 32-bit raw tile entries (already
/// containing tile_id bits and any flip / rotate bits set). The masks
/// emitted here are the canonical Aseprite layout
/// (`tile_id | y_flip | x_flip | diagonal_flip`); they match what
/// `aseprite-loader` 0.4.2 parses.
fn tilemap_cel_chunk(layer_index: u16, grid_w: u16, grid_h: u16, tiles_raw: &[u32]) -> Vec<u8> {
    assert_eq!(
        tiles_raw.len(),
        (grid_w as usize) * (grid_h as usize),
        "tile count must match grid_w * grid_h",
    );
    let mut body = Vec::new();
    push_word(&mut body, layer_index);
    push_short(&mut body, 0); // x
    push_short(&mut body, 0); // y
    body.push(255); // opacity
    push_word(&mut body, 3); // cel_type: CompressedTilemap
    push_short(&mut body, 0); // z_index
    zeros(&mut body, 5); // reserved
    push_word(&mut body, grid_w);
    push_word(&mut body, grid_h);
    push_word(&mut body, 32); // bits_per_tile
    push_dword(&mut body, TILE_ID_MASK);
    push_dword(&mut body, Y_FLIP_MASK);
    push_dword(&mut body, X_FLIP_MASK);
    push_dword(&mut body, DIAG_FLIP_MASK);
    zeros(&mut body, 10); // reserved
    let mut raw = Vec::with_capacity(tiles_raw.len() * 4);
    for entry in tiles_raw {
        raw.extend_from_slice(&entry.to_le_bytes());
    }
    let compressed = zlib_compress(&raw);
    body.extend_from_slice(&compressed);
    chunk(0x2005, &body)
}

/// Wrap a list of chunks in a frame envelope (16-byte header).
fn frame(duration_ms: u16, chunks: &[Vec<u8>]) -> Vec<u8> {
    let body_size: usize = chunks.iter().map(|c| c.len()).sum();
    let total = (16 + body_size) as u32;
    let chunk_count_u32 = chunks.len() as u32;
    let chunk_count_u16: u16 = chunks.len().try_into().unwrap_or(0xFFFF);

    let mut buf = Vec::with_capacity(16 + body_size);
    push_dword(&mut buf, total);
    push_word(&mut buf, 0xF1FA); // FRAME_MAGIC
    push_word(&mut buf, chunk_count_u16);
    push_word(&mut buf, duration_ms);
    zeros(&mut buf, 2); // reserved
    push_dword(&mut buf, chunk_count_u32);
    for c in chunks {
        buf.extend_from_slice(c);
    }
    buf
}

/// Build the 128-byte file header.
fn header(width: u16, height: u16, frame_count: u16, file_size: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(128);
    push_dword(&mut buf, file_size);
    push_word(&mut buf, 0xA5E0); // HEADER_MAGIC
    push_word(&mut buf, frame_count);
    push_word(&mut buf, width);
    push_word(&mut buf, height);
    push_word(&mut buf, 32); // color_depth: RGBA
    push_dword(&mut buf, 0x1); // flags: layer opacity honored
    push_word(&mut buf, 100); // speed (deprecated)
    zeros(&mut buf, 4); // reserved dword
    zeros(&mut buf, 4); // reserved dword
    buf.push(0); // transparent_index
    zeros(&mut buf, 3); // reserved
    push_word(&mut buf, 0); // color_count
    buf.push(1); // pixel_width
    buf.push(1); // pixel_height
    push_short(&mut buf, 0); // grid_x
    push_short(&mut buf, 0); // grid_y
    push_word(&mut buf, 16); // grid_width
    push_word(&mut buf, 16); // grid_height
    zeros(&mut buf, 84); // reserved tail
    buf
}

/// Assemble the minimal tilemap fixture documented at the top of the file.
fn build_minimal_tilemap_fixture() -> Vec<u8> {
    // Two 2×2 RGBA tiles: tile 0 transparent, tile 1 four red pixels.
    let tile0 = vec![0u8; 2 * 2 * 4];
    let tile1: Vec<u8> = (0..4).flat_map(|_| [255, 0, 0, 255]).collect();
    let mut tile_pixels = Vec::with_capacity(tile0.len() + tile1.len());
    tile_pixels.extend_from_slice(&tile0);
    tile_pixels.extend_from_slice(&tile1);

    let layer0 = layer_chunk_image("img");
    let layer1 = layer_chunk_tilemap("tiles", 0);
    let tileset = tileset_chunk(0, "ts", 2, 2, 1, &tile_pixels, 2);
    let cel = tilemap_cel_chunk(
        1,
        2,
        2,
        &[
            0,               // (0,0) empty
            1,               // (1,0) tile 1
            1 | X_FLIP_MASK, // (0,1) tile 1, x-flip
            Y_FLIP_MASK,     // (1,1) tile 0, y-flip (still empty)
        ],
    );

    let f0 = frame(100, &[layer0, layer1, tileset, cel]);
    let body_size = f0.len();
    let file_size = 128 + body_size as u32;

    let mut out = Vec::with_capacity(128 + body_size);
    out.extend(header(8, 8, 1, file_size));
    out.extend(f0);
    out
}

// --- tests ------------------------------------------------------------------

#[test]
fn tilemap_layer_hydrates_with_tileset_id() {
    let bytes = build_minimal_tilemap_fixture();
    let AsepriteReadOutput { sprite, .. } = read_aseprite(&bytes).expect("fixture parses");
    assert_eq!(sprite.layers.len(), 2);
    assert_eq!(sprite.layers[0].kind, LayerKind::Image);
    assert_eq!(
        sprite.layers[1].kind,
        LayerKind::Tilemap {
            tileset_id: TilesetId::new(0)
        },
    );
}

#[test]
fn tileset_hydrates_with_inline_tile_images() {
    let bytes = build_minimal_tilemap_fixture();
    let AsepriteReadOutput { sprite, .. } = read_aseprite(&bytes).expect("fixture parses");
    assert_eq!(sprite.tilesets.len(), 1);
    let ts = &sprite.tilesets[0];
    assert_eq!(ts.id, TilesetId::new(0));
    assert_eq!(ts.name, "ts");
    assert_eq!(ts.tile_size, (2, 2));
    assert_eq!(ts.base_index, 1);
    assert_eq!(ts.tile_count(), 2);
    // Tile 0 is the empty / transparent tile by Aseprite convention,
    // stored verbatim from the file (all-zero pixels).
    let tile0 = ts.tile(0).expect("tile 0 stored verbatim");
    assert_eq!(tile0.pixels.width, 2);
    assert_eq!(tile0.pixels.height, 2);
    assert_eq!(tile0.pixels.color_mode, ColorMode::Rgba);
    assert!(tile0.pixels.data.iter().all(|&b| b == 0));
    // Tile 1 was four red pixels.
    let tile1 = ts.tile(1).expect("tile 1 present");
    assert_eq!(tile1.pixels.data, [255, 0, 0, 255].repeat(4));
}

#[test]
fn tilemap_cel_hydrates_into_grid_with_decoded_flips() {
    let bytes = build_minimal_tilemap_fixture();
    let AsepriteReadOutput { cels, .. } = read_aseprite(&bytes).expect("fixture parses");
    let cel = cels
        .get(LayerId::new(1), FrameIndex::new(0))
        .expect("tilemap cel present on layer 1 / frame 0");
    let CelData::Tilemap {
        grid_w,
        grid_h,
        tiles,
    } = &cel.data
    else {
        panic!("expected CelData::Tilemap, got {:?}", cel.data);
    };
    assert_eq!(*grid_w, 2);
    assert_eq!(*grid_h, 2);
    assert_eq!(tiles.len(), 4);

    assert_eq!(tiles[0].tile_id, 0);
    assert!(!tiles[0].flip_x && !tiles[0].flip_y && !tiles[0].rotate_90);

    assert_eq!(tiles[1].tile_id, 1);
    assert!(!tiles[1].flip_x && !tiles[1].flip_y);

    assert_eq!(tiles[2].tile_id, 1);
    assert!(tiles[2].flip_x);
    assert!(!tiles[2].flip_y);

    assert_eq!(tiles[3].tile_id, 0);
    assert!(!tiles[3].flip_x);
    assert!(tiles[3].flip_y);
}
