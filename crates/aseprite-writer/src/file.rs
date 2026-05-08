//! Owning data model for an `.aseprite` file.
//!
//! Field names mirror the structures returned by `aseprite-loader::binary::file::File`,
//! but every borrow is replaced with an owning equivalent (`&str` → `String`,
//! `&[u8]` → `Vec<u8>`, `Range` → owned bounds) so callers can build a file
//! programmatically without a backing buffer.
//!
//! Layout convention: layer chunks, the palette chunk, and the tags chunk
//! are written inside the **first frame**, matching how Aseprite itself
//! emits them. The `frames` vector then carries one entry per animation
//! frame, each holding its per-frame chunks (cels, ...).

use crate::types::{
    AnimationDirection, BlendMode, Color, ColorDepth, LayerFlags, LayerType, PaletteEntryFlags,
};

/// Top-level file. Mirrors `aseprite-loader::binary::file::File`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AseFile {
    pub header: Header,
    pub layers: Vec<LayerChunk>,
    pub palette: Option<PaletteChunk>,
    pub tags: Vec<Tag>,
    pub frames: Vec<Frame>,
}

/// 128-byte file header. Matches `aseprite-loader::binary::header::Header`,
/// minus the loader's `frames` field — the writer always derives the
/// frame count from `AseFile::frames.len()` instead of trusting a
/// caller-supplied value, so exposing a redundant field on the public
/// API would only invite mistakes.
///
/// `file_size` is **computed at write time** and not stored on this
/// struct. The deprecated `speed` field is preserved for round-trip
/// parity.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Header {
    pub width: u16,
    pub height: u16,
    pub color_depth: ColorDepth,
    /// Header flags. Bit 0 (= 0x1) indicates layer opacity is honored.
    pub flags: u32,
    /// Deprecated speed (ms between frames). Use per-frame `duration` instead.
    pub speed: u16,
    /// Palette entry treated as transparent in non-background indexed layers.
    pub transparent_index: u8,
    pub color_count: u16,
    pub pixel_width: u8,
    pub pixel_height: u8,
    pub grid_x: i16,
    pub grid_y: i16,
    pub grid_width: u16,
    pub grid_height: u16,
}

impl Header {
    /// Header for a sprite with the given canvas size and color mode.
    /// All other fields default to Aseprite's "no extra info" values.
    pub fn new(width: u16, height: u16, color_depth: ColorDepth) -> Self {
        Self {
            width,
            height,
            color_depth,
            flags: 0x1,
            speed: 100,
            transparent_index: 0,
            color_count: 0,
            pixel_width: 1,
            pixel_height: 1,
            grid_x: 0,
            grid_y: 0,
            grid_width: 16,
            grid_height: 16,
        }
    }
}

/// Animation frame envelope. Matches `aseprite-loader::binary::frame::Frame`.
///
/// `duration` is in milliseconds. Per-frame chunks (cels) live in this
/// struct in future milestones; for M5 the field is empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub duration: u16,
    // Cels deferred to a later milestone (M5 adds image cels).
}

impl Frame {
    pub fn new(duration_ms: u16) -> Self {
        Self {
            duration: duration_ms,
        }
    }
}

/// Layer chunk. Matches `aseprite-loader::binary::chunks::layer::LayerChunk`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerChunk {
    pub flags: LayerFlags,
    pub layer_type: LayerType,
    pub child_level: u16,
    pub blend_mode: BlendMode,
    pub opacity: u8,
    pub name: String,
    /// Tileset index — required for `LayerType::Tilemap`, ignored otherwise.
    pub tileset_index: Option<u32>,
}

/// Palette chunk (modern, 0x2019). Matches
/// `aseprite-loader::binary::chunks::palette::PaletteChunk`.
///
/// The on-disk encoding writes a count and a `[first..=last]` range.
/// Here `first_color` is the index of the first entry; `entries.len()`
/// gives the count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteChunk {
    pub first_color: u32,
    pub entries: Vec<PaletteEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub color: Color,
    pub name: Option<String>,
}

impl PaletteEntry {
    pub fn flags(&self) -> PaletteEntryFlags {
        if self.name.is_some() {
            PaletteEntryFlags::HAS_NAME
        } else {
            PaletteEntryFlags::empty()
        }
    }
}

/// Animation tag. Matches `aseprite-loader::binary::chunks::tags::Tag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub from_frame: u16,
    pub to_frame: u16,
    pub direction: AnimationDirection,
    pub repeat: u16,
    /// Deprecated label color. Preserved for round-trip parity.
    pub color: [u8; 3],
    pub name: String,
}
