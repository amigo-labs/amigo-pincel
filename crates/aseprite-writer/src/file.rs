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
    pub tilesets: Vec<TilesetChunk>,
    pub slices: Vec<SliceChunk>,
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
/// `duration` is in milliseconds. `cels` carries the per-frame cel chunks
/// emitted into this frame's chunk list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Frame {
    pub duration: u16,
    pub cels: Vec<CelChunk>,
}

impl Frame {
    pub fn new(duration_ms: u16) -> Self {
        Self {
            duration: duration_ms,
            cels: Vec::new(),
        }
    }
}

/// Cel chunk (`0x2005`). Mirrors
/// `aseprite-loader::binary::chunks::cel::CelChunk`, with owned content.
///
/// `layer_index` is the zero-based position of the layer in
/// [`AseFile::layers`]. `(x, y)` is the top-left of the cel image in
/// sprite-space; negative values are valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CelChunk {
    pub layer_index: u16,
    pub x: i16,
    pub y: i16,
    pub opacity: u8,
    pub z_index: i16,
    pub content: CelContent,
}

/// Variant of cel data inside a [`CelChunk`].
///
/// `Image` becomes Cel Type 2 (Compressed Image) on disk; pixel data is
/// zlib-compressed. `Linked` becomes Cel Type 1 (Linked Cel) and points
/// at another frame that owns the cel for this layer. `Tilemap` becomes
/// Cel Type 3 (Compressed Tilemap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CelContent {
    /// Image cel, written as Cel Type 2 (Compressed Image).
    ///
    /// `data` is the uncompressed pixel buffer in the file's color
    /// depth: `width * height * bytes_per_pixel` bytes, where
    /// `bytes_per_pixel` is 4 for RGBA, 2 for grayscale, and 1 for
    /// indexed (see [`crate::types::ColorDepth::bytes_per_pixel`]).
    /// The writer compresses it with zlib before emission and rejects
    /// buffers whose length does not match the expected size.
    Image {
        width: u16,
        height: u16,
        data: Vec<u8>,
    },
    /// Cel Type 1 — points at the cel in `frame_position` for the same
    /// layer index.
    Linked { frame_position: u16 },
    /// Cel Type 3 (Compressed Tilemap). `tiles` is a row-major list of
    /// 32-bit raw tile entries; each entry already packs the tile id and
    /// any flip / rotate bits per the supplied bitmasks. Field names and
    /// layout mirror
    /// `aseprite-loader::binary::chunks::cel::CelContent::CompressedTilemap`
    /// so the writer can re-emit reader output without translation.
    ///
    /// `width` and `height` are the grid dimensions in **tiles**, not
    /// pixels. `bits_per_tile` is fixed at 32 in Aseprite v1.3 and the
    /// writer rejects anything else.
    Tilemap {
        width: u16,
        height: u16,
        bits_per_tile: u16,
        bitmask_tile_id: u32,
        bitmask_x_flip: u32,
        bitmask_y_flip: u32,
        bitmask_diagonal_flip: u32,
        /// Row-major `width * height` tile entries. Each `u32` is the
        /// raw little-endian DWORD that will be emitted on disk.
        tiles: Vec<u32>,
    },
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

/// Tileset chunk (`0x2023`). Matches
/// `aseprite-loader::binary::chunks::tileset::TilesetChunk` minus the
/// borrowing.
///
/// The writer emits the chunk in the **first frame**, alongside layer
/// chunks. Phase 1 supports inline `TILES` data only: pass an RGBA8
/// buffer of `tile_w * tile_h * number_of_tiles * 4` bytes via
/// [`TilesetChunk::tile_pixels`]; the writer zlib-compresses it before
/// emission. The on-disk `TILES` flag is set automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TilesetChunk {
    pub id: u32,
    pub number_of_tiles: u32,
    pub tile_width: u16,
    pub tile_height: u16,
    /// Display base index. Typically `1` (tile `0` = empty); set to `0`
    /// for zero-based indexing. Stored as `SHORT` on disk.
    pub base_index: i16,
    pub name: String,
    /// Uncompressed tile-image data: `tile_w * tile_h * number_of_tiles * 4`
    /// RGBA8 bytes, tiles stacked vertically. The writer zlib-compresses
    /// this before emission and rejects buffers whose length does not
    /// match the expected size.
    pub tile_pixels: Vec<u8>,
}

/// Slice chunk (`0x2022`). Mirrors
/// `aseprite-loader::binary::chunks::slice::SliceChunk` with an owned
/// `name` and per-key 9-patch / pivot optionals.
///
/// A slice is a named rectangle (optionally 9-patch and / or pivoted)
/// overlaid on the sprite. Its geometry can vary by frame: each
/// [`SliceKey`] applies starting at `frame` until the next key's `frame`,
/// matching Aseprite semantics.
///
/// The writer derives the on-disk `flags` field from the keys: the
/// `NINE_PATCH` bit is set when **any** key carries a `nine_patch`, and
/// the `PIVOT` bit is set when **any** key carries a `pivot`. Aseprite
/// stores the flag set on the chunk rather than per key, so every key
/// must agree on which optional fields are present once a slice opts
/// into one — passing a key without `nine_patch` while another key has
/// one (and vice versa for `pivot`) is rejected at write time with
/// [`crate::WriteError::SliceFlagsInconsistent`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceChunk {
    pub name: String,
    /// Per-frame keys, sorted ascending by `frame`. The writer rejects
    /// an empty `keys` vec or a non-monotonic ordering.
    pub keys: Vec<SliceKey>,
}

/// Per-frame geometry for a [`SliceChunk`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SliceKey {
    /// First frame this key applies to.
    pub frame: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// 9-patch inner rectangle, when this slice is a 9-patch.
    pub nine_patch: Option<NinePatch>,
    /// Pivot point, when this slice has a pivot.
    pub pivot: Option<Pivot>,
}

/// 9-patch inner rectangle inside a [`SliceKey`]. Coordinates are
/// relative to the slice's `(x, y)` origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NinePatch {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Pivot point inside a [`SliceKey`]. Coordinates are relative to the
/// slice's `(x, y)` origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pivot {
    pub x: i32,
    pub y: i32,
}
