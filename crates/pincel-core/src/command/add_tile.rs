//! `AddTile` command — append an empty (transparent) tile image to a
//! tileset.
//!
//! Tile ids are positional (`tiles.len()` at insertion time) and nothing
//! removes tiles, so with linear undo history the appended tile is
//! guaranteed to still be the last entry whenever this command is
//! reverted — `revert` can safely pop. A `PlaceTile` referencing the new
//! id later in history is always undone first, so no dangling reference
//! can survive the revert.

use crate::document::{CelMap, ColorMode, PixelBuffer, Sprite, TileImage, TilesetId};

use super::Command;
use super::error::CommandError;

/// Append a transparent [`TileImage`] (sized to the tileset's `tile_size`)
/// to the tileset's tile list. `revert` pops it back off.
#[derive(Debug, Clone)]
pub struct AddTile {
    tileset: TilesetId,
    /// `true` after a successful `apply`; consumed on `revert`.
    applied: bool,
}

impl AddTile {
    /// Append a fresh transparent tile to the tileset with id `tileset`.
    pub fn new(tileset: TilesetId) -> Self {
        Self {
            tileset,
            applied: false,
        }
    }
}

impl Command for AddTile {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let tileset = doc
            .tilesets
            .iter_mut()
            .find(|t| t.id == self.tileset)
            .ok_or(CommandError::UnknownTileset(self.tileset.0))?;
        let (tile_w, tile_h) = tileset.tile_size;
        tileset.tiles.push(TileImage {
            pixels: PixelBuffer::empty(tile_w, tile_h, ColorMode::Rgba),
        });
        self.applied = true;
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if !self.applied {
            return;
        }
        self.applied = false;
        if let Some(tileset) = doc.tilesets.iter_mut().find(|t| t.id == self.tileset) {
            tileset.tiles.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Tileset;

    fn doc_with_tileset() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 8)
            .add_tileset(Tileset::new(TilesetId::new(1), "ground", (4, 4)))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_appends_transparent_tile_sized_to_tileset() {
        let (mut sprite, mut cels) = doc_with_tileset();
        let mut cmd = AddTile::new(TilesetId::new(1));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        let tiles = &sprite.tilesets[0].tiles;
        assert_eq!(tiles.len(), 1);
        assert_eq!((tiles[0].pixels.width, tiles[0].pixels.height), (4, 4));
        assert!(tiles[0].pixels.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn apply_unknown_tileset_yields_error() {
        let (mut sprite, mut cels) = doc_with_tileset();
        let mut cmd = AddTile::new(TilesetId::new(9));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::UnknownTileset(9))
        );
        assert!(sprite.tilesets[0].tiles.is_empty());
    }

    #[test]
    fn revert_pops_appended_tile() {
        let (mut sprite, mut cels) = doc_with_tileset();
        let mut cmd = AddTile::new(TilesetId::new(1));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.tilesets[0].tiles.is_empty());
    }

    #[test]
    fn revert_without_apply_is_a_no_op() {
        let (mut sprite, mut cels) = doc_with_tileset();
        sprite.tilesets[0].tiles.push(TileImage {
            pixels: PixelBuffer::empty(4, 4, ColorMode::Rgba),
        });
        let mut cmd = AddTile::new(TilesetId::new(1));
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(sprite.tilesets[0].tiles.len(), 1);
    }

    #[test]
    fn apply_revert_apply_round_trip_restores_state() {
        let (mut sprite, mut cels) = doc_with_tileset();
        let mut cmd = AddTile::new(TilesetId::new(1));
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        cmd.revert(&mut sprite, &mut cels);
        assert!(sprite.tilesets[0].tiles.is_empty());
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(sprite.tilesets[0].tiles.len(), 1);
    }
}
