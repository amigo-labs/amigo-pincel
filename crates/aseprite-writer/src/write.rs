//! Serialize an [`AseFile`] to the Aseprite v1.3 binary format.
//!
//! Spec: <https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md>
//!
//! Layout convention used by this writer:
//!
//! - Layer chunks, palette chunk, and the tags chunk are emitted into
//!   the **first frame**, matching how Aseprite itself writes them.
//! - Per-frame chunks (cels) live in their respective frame.
//! - The header `frames` field is overwritten from `frames.len()`; the
//!   header `file_size` is computed at write time.

use std::io::Write;

use flate2::Compression;
use flate2::write::ZlibEncoder;

use crate::bytes::{write_byte, write_dword, write_short, write_string, write_word, write_zeros};
use crate::error::WriteError;
use crate::file::{
    AseFile, CelChunk, CelContent, Frame, Header, LayerChunk, PaletteChunk, Tag, TilesetChunk,
};
use crate::types::{LayerType, PaletteEntryFlags};

const HEADER_MAGIC: u16 = 0xA5E0;
const FRAME_MAGIC: u16 = 0xF1FA;
const HEADER_SIZE: usize = 128;
const FRAME_HEADER_SIZE: usize = 16;
const CHUNK_HEADER_SIZE: usize = 6; // dword size + word type

const CHUNK_TYPE_LAYER: u16 = 0x2004;
const CHUNK_TYPE_CEL: u16 = 0x2005;
const CHUNK_TYPE_TAGS: u16 = 0x2018;
const CHUNK_TYPE_PALETTE: u16 = 0x2019;
const CHUNK_TYPE_TILESET: u16 = 0x2023;

const CEL_TYPE_LINKED: u16 = 1;
const CEL_TYPE_COMPRESSED_IMAGE: u16 = 2;
const CEL_TYPE_COMPRESSED_TILEMAP: u16 = 3;

/// `TilesetFlags::TILES` bit — set when `tile_pixels` is emitted inline.
const TILESET_FLAG_TILES: u32 = 0x2;
/// `TilesetFlags::TILE_0_EMPTY` bit — Aseprite convention: tile id 0 is
/// the empty / transparent tile. Pincel preserves the convention so the
/// writer always sets it.
const TILESET_FLAG_TILE_0_EMPTY: u32 = 0x4;

/// Write `file` to `out` in the Aseprite v1.3 format.
///
/// The file is staged in memory first so that the header `file_size`
/// and per-frame size fields can be filled in. For typical sprite
/// sizes this is well within budget; very large sprites should use a
/// streaming writer (not yet implemented — see `STATUS.md`).
pub fn write<W: Write>(file: &AseFile, out: &mut W) -> Result<(), WriteError> {
    let frame_count: u16 = file
        .frames
        .len()
        .try_into()
        .map_err(|_| WriteError::TooMany {
            what: "frames",
            count: file.frames.len() as u64,
            max: u16::MAX as u64,
        })?;

    let frame_blocks = encode_frames(file)?;
    let total_u64: u64 = frame_blocks
        .iter()
        .map(|f| f.len() as u64)
        .sum::<u64>()
        .saturating_add(HEADER_SIZE as u64);
    let total_size: u32 = total_u64.try_into().map_err(|_| WriteError::TooMany {
        what: "file bytes",
        count: total_u64,
        max: u32::MAX as u64,
    })?;

    write_header(out, &file.header, frame_count, total_size)?;
    for block in &frame_blocks {
        out.write_all(block)?;
    }
    Ok(())
}

fn encode_frames(file: &AseFile) -> Result<Vec<Vec<u8>>, WriteError> {
    let bytes_per_pixel = file.header.color_depth.bytes_per_pixel();
    let mut blocks = Vec::with_capacity(file.frames.len());
    for (idx, frame) in file.frames.iter().enumerate() {
        let mut chunks: Vec<Vec<u8>> = Vec::new();
        if idx == 0 {
            for layer in &file.layers {
                chunks.push(encode_chunk(CHUNK_TYPE_LAYER, |buf| {
                    write_layer_body(buf, layer)
                })?);
            }
            if let Some(palette) = &file.palette {
                chunks.push(encode_chunk(CHUNK_TYPE_PALETTE, |buf| {
                    write_palette_body(buf, palette)
                })?);
            }
            if !file.tags.is_empty() {
                chunks.push(encode_chunk(CHUNK_TYPE_TAGS, |buf| {
                    write_tags_body(buf, &file.tags)
                })?);
            }
            for tileset in &file.tilesets {
                validate_tileset(tileset)?;
                chunks.push(encode_chunk(CHUNK_TYPE_TILESET, |buf| {
                    write_tileset_body(buf, tileset)
                })?);
            }
        }
        for cel in &frame.cels {
            validate_cel(cel, file, bytes_per_pixel)?;
            chunks.push(encode_chunk(CHUNK_TYPE_CEL, |buf| {
                write_cel_body(buf, cel)
            })?);
        }
        blocks.push(encode_frame(frame, &chunks)?);
    }
    Ok(blocks)
}

fn validate_cel(cel: &CelChunk, file: &AseFile, bytes_per_pixel: u8) -> Result<(), WriteError> {
    if (cel.layer_index as usize) >= file.layers.len() {
        return Err(WriteError::CelLayerIndexOutOfRange {
            layer_index: cel.layer_index,
            layers: file.layers.len(),
        });
    }
    match &cel.content {
        CelContent::Image {
            width,
            height,
            data,
        } => {
            // Use checked arithmetic so a 32-bit `usize` doesn't silently
            // wrap on a 65535x65535 cel.
            let expected = (*width as usize)
                .checked_mul(*height as usize)
                .and_then(|n| n.checked_mul(bytes_per_pixel as usize))
                .ok_or(WriteError::TooMany {
                    what: "cel image bytes",
                    count: u64::from(*width) * u64::from(*height) * u64::from(bytes_per_pixel),
                    max: usize::MAX as u64,
                })?;
            if data.len() != expected {
                return Err(WriteError::CelImageSizeMismatch {
                    width: *width,
                    height: *height,
                    bytes_per_pixel,
                    expected,
                    actual: data.len(),
                });
            }
        }
        CelContent::Linked { frame_position } => {
            if (*frame_position as usize) >= file.frames.len() {
                return Err(WriteError::CelLinkedFrameOutOfRange {
                    frame_position: *frame_position,
                    frames: file.frames.len(),
                });
            }
        }
        CelContent::Tilemap {
            width,
            height,
            bits_per_tile,
            tiles,
            ..
        } => {
            if *bits_per_tile != 32 {
                return Err(WriteError::TilemapBitsPerTileUnsupported {
                    bits: *bits_per_tile,
                });
            }
            let expected =
                (*width as usize)
                    .checked_mul(*height as usize)
                    .ok_or(WriteError::TooMany {
                        what: "tilemap tile count",
                        count: u64::from(*width) * u64::from(*height),
                        max: usize::MAX as u64,
                    })?;
            if tiles.len() != expected {
                return Err(WriteError::TilemapTileCountMismatch {
                    width: *width,
                    height: *height,
                    expected,
                    actual: tiles.len(),
                });
            }
        }
    }
    Ok(())
}

fn validate_tileset(tileset: &TilesetChunk) -> Result<(), WriteError> {
    let expected = (tileset.tile_width as usize)
        .checked_mul(tileset.tile_height as usize)
        .and_then(|n| n.checked_mul(tileset.number_of_tiles as usize))
        .and_then(|n| n.checked_mul(4))
        .ok_or(WriteError::TooMany {
            what: "tileset tile-image bytes",
            count: u64::from(tileset.tile_width)
                * u64::from(tileset.tile_height)
                * u64::from(tileset.number_of_tiles)
                * 4,
            max: usize::MAX as u64,
        })?;
    if tileset.tile_pixels.len() != expected {
        return Err(WriteError::TilesetPixelsSizeMismatch {
            id: tileset.id,
            tile_w: tileset.tile_width,
            tile_h: tileset.tile_height,
            tiles: tileset.number_of_tiles,
            expected,
            actual: tileset.tile_pixels.len(),
        });
    }
    Ok(())
}

fn write_header<W: Write>(
    w: &mut W,
    header: &Header,
    frame_count: u16,
    file_size: u32,
) -> Result<(), WriteError> {
    write_dword(w, file_size)?;
    write_word(w, HEADER_MAGIC)?;
    write_word(w, frame_count)?;
    write_word(w, header.width)?;
    write_word(w, header.height)?;
    write_word(w, header.color_depth.bpp())?;
    write_dword(w, header.flags)?;
    write_word(w, header.speed)?;
    write_zeros(w, 4)?; // reserved DWORD
    write_zeros(w, 4)?; // reserved DWORD
    write_byte(w, header.transparent_index)?;
    write_zeros(w, 3)?;
    write_word(w, header.color_count)?;
    write_byte(w, header.pixel_width)?;
    write_byte(w, header.pixel_height)?;
    write_short(w, header.grid_x)?;
    write_short(w, header.grid_y)?;
    write_word(w, header.grid_width)?;
    write_word(w, header.grid_height)?;
    write_zeros(w, 84)?; // reserved tail of 128-byte header
    Ok(())
}

fn encode_frame(frame: &Frame, chunks: &[Vec<u8>]) -> Result<Vec<u8>, WriteError> {
    let body_size: usize = chunks.iter().map(|c| c.len()).sum();
    let total: u32 =
        (FRAME_HEADER_SIZE + body_size)
            .try_into()
            .map_err(|_| WriteError::TooMany {
                what: "frame bytes",
                count: (FRAME_HEADER_SIZE + body_size) as u64,
                max: u32::MAX as u64,
            })?;
    let chunk_count_u32: u32 = chunks.len().try_into().map_err(|_| WriteError::TooMany {
        what: "chunks per frame",
        count: chunks.len() as u64,
        max: u32::MAX as u64,
    })?;
    let chunk_count_u16 = u16::try_from(chunks.len()).unwrap_or(0xFFFF);

    let mut buf = Vec::with_capacity(FRAME_HEADER_SIZE + body_size);
    write_dword(&mut buf, total)?;
    write_word(&mut buf, FRAME_MAGIC)?;
    write_word(&mut buf, chunk_count_u16)?;
    write_word(&mut buf, frame.duration)?;
    write_zeros(&mut buf, 2)?;
    write_dword(&mut buf, chunk_count_u32)?;
    for chunk in chunks {
        buf.extend_from_slice(chunk);
    }
    Ok(buf)
}

/// Wraps `body_writer`'s output in a chunk envelope (`size` `type` body).
fn encode_chunk<F>(chunk_type: u16, body_writer: F) -> Result<Vec<u8>, WriteError>
where
    F: FnOnce(&mut Vec<u8>) -> Result<(), WriteError>,
{
    let mut body = Vec::new();
    body_writer(&mut body)?;
    let total: u32 =
        (CHUNK_HEADER_SIZE + body.len())
            .try_into()
            .map_err(|_| WriteError::TooMany {
                what: "chunk bytes",
                count: (CHUNK_HEADER_SIZE + body.len()) as u64,
                max: u32::MAX as u64,
            })?;
    let mut buf = Vec::with_capacity(CHUNK_HEADER_SIZE + body.len());
    write_dword(&mut buf, total)?;
    write_word(&mut buf, chunk_type)?;
    buf.extend_from_slice(&body);
    Ok(buf)
}

// ---- chunk bodies -----------------------------------------------------------

fn write_layer_body<W: Write>(w: &mut W, layer: &LayerChunk) -> Result<(), WriteError> {
    if layer.layer_type == LayerType::Tilemap && layer.tileset_index.is_none() {
        return Err(WriteError::MissingTilesetIndex {
            name: layer.name.clone(),
        });
    }
    write_word(w, layer.flags.bits())?;
    write_word(w, layer.layer_type.as_u16())?;
    write_word(w, layer.child_level)?;
    write_word(w, 0)?; // default layer width  (deprecated, ignored)
    write_word(w, 0)?; // default layer height (deprecated, ignored)
    write_word(w, layer.blend_mode.as_u16())?;
    write_byte(w, layer.opacity)?;
    write_zeros(w, 3)?; // reserved
    write_string(w, &layer.name)?;
    if layer.layer_type == LayerType::Tilemap {
        write_dword(w, layer.tileset_index.unwrap_or(0))?;
    }
    Ok(())
}

fn write_palette_body<W: Write>(w: &mut W, palette: &PaletteChunk) -> Result<(), WriteError> {
    if palette.entries.is_empty() {
        return Err(WriteError::EmptyPalette);
    }
    let len = palette.entries.len();
    let last_index = palette.first_color.checked_add((len - 1) as u32).ok_or(
        WriteError::PaletteRangeOverflow {
            first: palette.first_color,
            len,
        },
    )?;
    let palette_size: u32 = len.try_into().map_err(|_| WriteError::TooMany {
        what: "palette entries",
        count: len as u64,
        max: u32::MAX as u64,
    })?;
    write_dword(w, palette_size)?;
    write_dword(w, palette.first_color)?;
    write_dword(w, last_index)?;
    write_zeros(w, 8)?; // reserved
    for entry in &palette.entries {
        let flags = entry.flags();
        write_word(w, flags.bits())?;
        write_byte(w, entry.color.red)?;
        write_byte(w, entry.color.green)?;
        write_byte(w, entry.color.blue)?;
        write_byte(w, entry.color.alpha)?;
        if flags.contains(PaletteEntryFlags::HAS_NAME) {
            // Safe: HAS_NAME is set iff name is Some (see PaletteEntry::flags).
            let name = entry.name.as_deref().unwrap_or("");
            write_string(w, name)?;
        }
    }
    Ok(())
}

fn write_cel_body<W: Write>(w: &mut W, cel: &CelChunk) -> Result<(), WriteError> {
    write_word(w, cel.layer_index)?;
    write_short(w, cel.x)?;
    write_short(w, cel.y)?;
    write_byte(w, cel.opacity)?;
    let cel_type = match &cel.content {
        CelContent::Image { .. } => CEL_TYPE_COMPRESSED_IMAGE,
        CelContent::Linked { .. } => CEL_TYPE_LINKED,
        CelContent::Tilemap { .. } => CEL_TYPE_COMPRESSED_TILEMAP,
    };
    write_word(w, cel_type)?;
    write_short(w, cel.z_index)?;
    write_zeros(w, 5)?;
    match &cel.content {
        CelContent::Image {
            width,
            height,
            data,
        } => {
            write_word(w, *width)?;
            write_word(w, *height)?;
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data)?;
            let compressed = encoder.finish()?;
            w.write_all(&compressed)?;
        }
        CelContent::Linked { frame_position } => {
            write_word(w, *frame_position)?;
        }
        CelContent::Tilemap {
            width,
            height,
            bits_per_tile,
            bitmask_tile_id,
            bitmask_x_flip,
            bitmask_y_flip,
            bitmask_diagonal_flip,
            tiles,
        } => {
            write_word(w, *width)?;
            write_word(w, *height)?;
            write_word(w, *bits_per_tile)?;
            write_dword(w, *bitmask_tile_id)?;
            // On-disk bitmask order matches `aseprite-loader`'s parse
            // order (`y_flip` precedes `x_flip`). The Aseprite spec text
            // labels the second / third dwords differently, but the
            // loader 0.4.2 source is authoritative for round-trip
            // compatibility — see the M8.4 read path's discussion.
            write_dword(w, *bitmask_y_flip)?;
            write_dword(w, *bitmask_x_flip)?;
            write_dword(w, *bitmask_diagonal_flip)?;
            write_zeros(w, 10)?; // reserved
            let mut raw = Vec::with_capacity(tiles.len() * 4);
            for entry in tiles {
                raw.extend_from_slice(&entry.to_le_bytes());
            }
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(&raw)?;
            let compressed = encoder.finish()?;
            w.write_all(&compressed)?;
        }
    }
    Ok(())
}

/// Emit a Tileset chunk body (`0x2023`).
///
/// Phase 1 supports inline tile data only: the `TILES` and
/// `TILE_0_EMPTY` flags are both set, and `tileset.tile_pixels` is
/// zlib-compressed and appended after a `DWORD compressed_size`. The
/// `EXTERNAL_FILE` flag is never emitted (the writer has no public
/// surface for it).
fn write_tileset_body<W: Write>(w: &mut W, tileset: &TilesetChunk) -> Result<(), WriteError> {
    write_dword(w, tileset.id)?;
    write_dword(w, TILESET_FLAG_TILES | TILESET_FLAG_TILE_0_EMPTY)?;
    write_dword(w, tileset.number_of_tiles)?;
    write_word(w, tileset.tile_width)?;
    write_word(w, tileset.tile_height)?;
    write_short(w, tileset.base_index)?;
    write_zeros(w, 14)?; // reserved
    write_string(w, &tileset.name)?;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tileset.tile_pixels)?;
    let compressed = encoder.finish()?;
    let compressed_size: u32 = compressed
        .len()
        .try_into()
        .map_err(|_| WriteError::TooMany {
            what: "tileset compressed tile bytes",
            count: compressed.len() as u64,
            max: u32::MAX as u64,
        })?;
    write_dword(w, compressed_size)?;
    w.write_all(&compressed)?;
    Ok(())
}

fn write_tags_body<W: Write>(w: &mut W, tags: &[Tag]) -> Result<(), WriteError> {
    let count: u16 = tags.len().try_into().map_err(|_| WriteError::TooMany {
        what: "tags",
        count: tags.len() as u64,
        max: u16::MAX as u64,
    })?;
    write_word(w, count)?;
    write_zeros(w, 8)?; // reserved
    for tag in tags {
        if tag.from_frame > tag.to_frame {
            return Err(WriteError::InvalidTagRange {
                name: tag.name.clone(),
                from: tag.from_frame,
                to: tag.to_frame,
            });
        }
        write_word(w, tag.from_frame)?;
        write_word(w, tag.to_frame)?;
        write_byte(w, tag.direction.as_u8())?;
        write_word(w, tag.repeat)?;
        write_zeros(w, 6)?;
        write_byte(w, tag.color[0])?;
        write_byte(w, tag.color[1])?;
        write_byte(w, tag.color[2])?;
        write_byte(w, 0)?; // extra byte
        write_string(w, &tag.name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::Frame;
    use crate::types::ColorDepth;

    #[test]
    fn write_emits_128_byte_header_plus_frames() {
        let file = AseFile {
            header: Header::new(8, 8, ColorDepth::Rgba),
            layers: Vec::new(),
            palette: None,
            tags: Vec::new(),
            tilesets: Vec::new(),
            frames: vec![Frame::new(100)],
        };
        let mut buf = Vec::new();
        write(&file, &mut buf).unwrap();
        // 128-byte header + 16-byte empty frame header.
        assert_eq!(buf.len(), HEADER_SIZE + FRAME_HEADER_SIZE);
        // Magic at offset 4.
        assert_eq!(&buf[4..6], &HEADER_MAGIC.to_le_bytes());
        // Frame magic at offset 128 + 4.
        assert_eq!(&buf[132..134], &FRAME_MAGIC.to_le_bytes());
    }

    #[test]
    fn write_rejects_more_frames_than_u16_can_hold() {
        let mut file = AseFile {
            header: Header::new(1, 1, ColorDepth::Rgba),
            layers: Vec::new(),
            palette: None,
            tags: Vec::new(),
            tilesets: Vec::new(),
            frames: Vec::with_capacity(u16::MAX as usize + 1),
        };
        for _ in 0..(u16::MAX as usize + 1) {
            file.frames.push(Frame::new(0));
        }
        let mut buf = Vec::new();
        let err = write(&file, &mut buf).unwrap_err();
        assert!(matches!(err, WriteError::TooMany { what: "frames", .. }));
    }

    #[test]
    fn empty_palette_is_rejected() {
        let mut buf = Vec::new();
        let palette = PaletteChunk {
            first_color: 0,
            entries: Vec::new(),
        };
        let err = write_palette_body(&mut buf, &palette).unwrap_err();
        assert!(matches!(err, WriteError::EmptyPalette));
    }

    #[test]
    fn tilemap_layer_without_index_is_rejected() {
        let mut buf = Vec::new();
        let layer = LayerChunk {
            flags: crate::types::LayerFlags::VISIBLE,
            layer_type: LayerType::Tilemap,
            child_level: 0,
            blend_mode: crate::types::BlendMode::Normal,
            opacity: 255,
            name: "tiles".into(),
            tileset_index: None,
        };
        let err = write_layer_body(&mut buf, &layer).unwrap_err();
        assert!(matches!(err, WriteError::MissingTilesetIndex { .. }));
    }

    #[test]
    fn invalid_tag_range_is_rejected() {
        let mut buf = Vec::new();
        let tags = vec![Tag {
            from_frame: 5,
            to_frame: 2,
            direction: crate::types::AnimationDirection::Forward,
            repeat: 0,
            color: [0, 0, 0],
            name: "bad".into(),
        }];
        let err = write_tags_body(&mut buf, &tags).unwrap_err();
        assert!(matches!(err, WriteError::InvalidTagRange { .. }));
    }

    fn rgba_layer(name: &str) -> LayerChunk {
        LayerChunk {
            flags: crate::types::LayerFlags::VISIBLE,
            layer_type: LayerType::Normal,
            child_level: 0,
            blend_mode: crate::types::BlendMode::Normal,
            opacity: 255,
            name: name.into(),
            tileset_index: None,
        }
    }

    #[test]
    fn cel_image_size_mismatch_is_rejected() {
        let file = AseFile {
            header: Header::new(8, 8, ColorDepth::Rgba),
            layers: vec![rgba_layer("L0")],
            palette: None,
            tags: Vec::new(),
            tilesets: Vec::new(),
            frames: vec![Frame {
                duration: 100,
                cels: vec![CelChunk {
                    layer_index: 0,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    z_index: 0,
                    content: CelContent::Image {
                        width: 2,
                        height: 2,
                        // 2x2 RGBA = 16 bytes; supply 8.
                        data: vec![0; 8],
                    },
                }],
            }],
        };
        let mut buf = Vec::new();
        let err = write(&file, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            WriteError::CelImageSizeMismatch {
                width: 2,
                height: 2,
                bytes_per_pixel: 4,
                expected: 16,
                actual: 8,
            }
        ));
    }

    #[test]
    fn cel_layer_index_out_of_range_is_rejected() {
        let file = AseFile {
            header: Header::new(8, 8, ColorDepth::Rgba),
            layers: vec![rgba_layer("L0")],
            palette: None,
            tags: Vec::new(),
            tilesets: Vec::new(),
            frames: vec![Frame {
                duration: 100,
                cels: vec![CelChunk {
                    layer_index: 5,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    z_index: 0,
                    content: CelContent::Image {
                        width: 1,
                        height: 1,
                        data: vec![0, 0, 0, 0],
                    },
                }],
            }],
        };
        let mut buf = Vec::new();
        let err = write(&file, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            WriteError::CelLayerIndexOutOfRange {
                layer_index: 5,
                layers: 1,
            }
        ));
    }

    #[test]
    fn linked_cel_frame_out_of_range_is_rejected() {
        let file = AseFile {
            header: Header::new(8, 8, ColorDepth::Rgba),
            layers: vec![rgba_layer("L0")],
            palette: None,
            tags: Vec::new(),
            tilesets: Vec::new(),
            frames: vec![Frame {
                duration: 100,
                cels: vec![CelChunk {
                    layer_index: 0,
                    x: 0,
                    y: 0,
                    opacity: 255,
                    z_index: 0,
                    content: CelContent::Linked { frame_position: 7 },
                }],
            }],
        };
        let mut buf = Vec::new();
        let err = write(&file, &mut buf).unwrap_err();
        assert!(matches!(
            err,
            WriteError::CelLinkedFrameOutOfRange {
                frame_position: 7,
                frames: 1,
            }
        ));
    }

    #[test]
    fn cel_chunk_envelope_carries_header_fields() {
        // Single 1x1 RGBA cel; verify the on-disk header byte layout
        // (layer_index, x, y, opacity, cel_type) by parsing the output
        // back through aseprite-loader's raw chunk parser. Scanning the
        // raw byte buffer for 0x2005 would be flaky because that
        // sequence can appear inside zlib-compressed pixel data.
        use aseprite_loader::binary::chunk::Chunk;
        use aseprite_loader::binary::chunks::cel::CelContent as LoaderCelContent;
        use aseprite_loader::binary::raw_file::parse_raw_file;

        let file = AseFile {
            header: Header::new(1, 1, ColorDepth::Rgba),
            layers: vec![rgba_layer("L0")],
            palette: None,
            tags: Vec::new(),
            tilesets: Vec::new(),
            frames: vec![Frame {
                duration: 100,
                cels: vec![CelChunk {
                    layer_index: 0,
                    x: -3,
                    y: 7,
                    opacity: 200,
                    z_index: 0,
                    content: CelContent::Image {
                        width: 1,
                        height: 1,
                        data: vec![0xAA, 0xBB, 0xCC, 0xFF],
                    },
                }],
            }],
        };
        let mut buf = Vec::new();
        write(&file, &mut buf).unwrap();

        let raw = parse_raw_file(&buf).expect("raw parse");
        let cel = raw.frames[0]
            .chunks
            .iter()
            .find_map(|c| match c {
                Chunk::Cel(cel) => Some(cel),
                _ => None,
            })
            .expect("cel chunk emitted into frame 0");
        assert_eq!(cel.layer_index, 0);
        assert_eq!(cel.x, -3);
        assert_eq!(cel.y, 7);
        assert_eq!(cel.opacity, 200);
        assert!(matches!(&cel.content, LoaderCelContent::Image(img) if img.compressed));
    }
}
