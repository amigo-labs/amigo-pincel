//! Tilesets and tile images. See `docs/specs/pincel.md` §3.4.

use super::cel::PixelBuffer;

/// Stable identifier for a tileset within a sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TilesetId(pub u32);

impl TilesetId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

/// Reference to an external tileset file (Aseprite supports shared tilesets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRef(pub String);

/// A single tile in a tileset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileImage {
    pub pixels: PixelBuffer,
}

/// A tileset. Tile id `0` is the empty / transparent tile by Aseprite
/// convention; Pincel preserves that convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tileset {
    pub id: TilesetId,
    pub name: String,
    pub tile_size: (u32, u32),
    pub tiles: Vec<TileImage>,
    /// Display base index (typically `1`; tile `0` = empty).
    pub base_index: i32,
    pub external_file: Option<PathRef>,
}
