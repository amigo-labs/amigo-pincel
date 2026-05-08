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

use crate::bytes::{write_byte, write_dword, write_short, write_string, write_word, write_zeros};
use crate::error::WriteError;
use crate::file::{AseFile, Frame, Header, LayerChunk, PaletteChunk, Tag};
use crate::types::{LayerType, PaletteEntryFlags};

const HEADER_MAGIC: u16 = 0xA5E0;
const FRAME_MAGIC: u16 = 0xF1FA;
const HEADER_SIZE: usize = 128;
const FRAME_HEADER_SIZE: usize = 16;
const CHUNK_HEADER_SIZE: usize = 6; // dword size + word type

const CHUNK_TYPE_LAYER: u16 = 0x2004;
const CHUNK_TYPE_TAGS: u16 = 0x2018;
const CHUNK_TYPE_PALETTE: u16 = 0x2019;

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
        }
        blocks.push(encode_frame(frame, &chunks)?);
    }
    Ok(blocks)
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
}
