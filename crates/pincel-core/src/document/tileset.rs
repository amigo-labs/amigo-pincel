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

impl Tileset {
    /// Create an empty tileset (no tile images) with Aseprite-default
    /// `base_index = 1` and no external-file reference.
    pub fn new(id: TilesetId, name: impl Into<String>, tile_size: (u32, u32)) -> Self {
        Self {
            id,
            name: name.into(),
            tile_size: (tile_size.0, tile_size.1),
            tiles: Vec::new(),
            base_index: 1,
            external_file: None,
        }
    }

    /// Number of tile images explicitly stored in [`Tileset::tiles`]. By
    /// Aseprite convention tile id `0` is the empty / transparent tile and
    /// is normally *not* stored — `compose()` and the codecs short-circuit
    /// id `0` without consulting the tileset. `Tileset::new` therefore
    /// starts with `tile_count() == 0`; tile 0 is implicit. A file loaded
    /// from disk may include an explicit tile-0 entry, in which case it is
    /// counted here.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Look up a tile image by its numeric id. Returns `None` if `tile_id`
    /// is past the end of [`Tileset::tiles`].
    pub fn tile(&self, tile_id: u32) -> Option<&TileImage> {
        self.tiles.get(tile_id as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::color_mode::ColorMode;

    #[test]
    fn new_seeds_empty_tile_list_and_base_index_one() {
        let t = Tileset::new(TilesetId::new(2), "ground", (16, 16));
        assert_eq!(t.id, TilesetId::new(2));
        assert_eq!(t.name, "ground");
        assert_eq!(t.tile_size, (16, 16));
        assert_eq!(t.tile_count(), 0);
        assert_eq!(t.base_index, 1);
        assert!(t.external_file.is_none());
    }

    #[test]
    fn tile_lookup_returns_none_past_end() {
        let mut t = Tileset::new(TilesetId::new(0), "t", (8, 8));
        t.tiles.push(TileImage {
            pixels: PixelBuffer::empty(8, 8, ColorMode::Rgba),
        });
        assert!(t.tile(0).is_some());
        assert!(t.tile(1).is_none());
        assert!(t.tile(u32::MAX).is_none());
    }
}
