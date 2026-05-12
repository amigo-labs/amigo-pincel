//! `SetTilePixel` command — write a single RGBA pixel into a tile image
//! stored inside a [`Tileset`].
//!
//! Sister command to [`SetPixel`], targeting `Tileset::tiles[tile_id].pixels`
//! instead of an image cel. Used by the Tileset Editor sub-mode
//! (see `docs/specs/pincel.md` §5.3 / CLAUDE.md M8.7d).

use crate::document::{CelMap, ColorMode, Rgba, Sprite, TilesetId};

use super::Command;
use super::error::CommandError;

/// Write a single pixel into a tile image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetTilePixel {
    tileset: TilesetId,
    tile_id: u32,
    x: u32,
    y: u32,
    new_color: Rgba,
    /// `Some` after a successful `apply`; carries the prior pixel value
    /// used by `revert`.
    previous: Option<Rgba>,
}

impl SetTilePixel {
    /// Build a new `SetTilePixel` targeting `(x, y)` (tile-local) on
    /// `tileset::tiles[tile_id]`.
    pub fn new(tileset: TilesetId, tile_id: u32, x: u32, y: u32, color: Rgba) -> Self {
        Self {
            tileset,
            tile_id,
            x,
            y,
            new_color: color,
            previous: None,
        }
    }
}

impl Command for SetTilePixel {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        let tileset = doc
            .tilesets
            .iter_mut()
            .find(|t| t.id == self.tileset)
            .ok_or(CommandError::UnknownTileset(self.tileset.0))?;
        let tile = tileset
            .tiles
            .get_mut(self.tile_id as usize)
            .ok_or(CommandError::UnknownTile {
                tileset: self.tileset.0,
                tile_id: self.tile_id,
            })?;
        if !matches!(tile.pixels.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }
        if self.x >= tile.pixels.width || self.y >= tile.pixels.height {
            return Err(CommandError::TilePixelOutOfBounds {
                tileset: self.tileset.0,
                tile_id: self.tile_id,
                x: self.x,
                y: self.y,
                width: tile.pixels.width,
                height: tile.pixels.height,
            });
        }
        let offset = ((self.y * tile.pixels.width + self.x) * 4) as usize;
        let prior = Rgba {
            r: tile.pixels.data[offset],
            g: tile.pixels.data[offset + 1],
            b: tile.pixels.data[offset + 2],
            a: tile.pixels.data[offset + 3],
        };
        tile.pixels.data[offset] = self.new_color.r;
        tile.pixels.data[offset + 1] = self.new_color.g;
        tile.pixels.data[offset + 2] = self.new_color.b;
        tile.pixels.data[offset + 3] = self.new_color.a;
        self.previous = Some(prior);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        let Some(prior) = self.previous.take() else {
            return;
        };
        let Some(tileset) = doc.tilesets.iter_mut().find(|t| t.id == self.tileset) else {
            return;
        };
        let Some(tile) = tileset.tiles.get_mut(self.tile_id as usize) else {
            return;
        };
        if self.x >= tile.pixels.width || self.y >= tile.pixels.height {
            return;
        }
        let offset = ((self.y * tile.pixels.width + self.x) * 4) as usize;
        tile.pixels.data[offset] = prior.r;
        tile.pixels.data[offset + 1] = prior.g;
        tile.pixels.data[offset + 2] = prior.b;
        tile.pixels.data[offset + 3] = prior.a;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{PixelBuffer, TileImage, Tileset};

    fn fixture() -> (Sprite, CelMap) {
        let mut sprite = Sprite::builder(8, 8)
            .add_layer(crate::document::Layer::image(
                crate::document::LayerId::new(1),
                "bg",
            ))
            .add_frame(crate::document::Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut tileset = Tileset::new(TilesetId::new(0), "t", (2, 2));
        tileset.tiles.push(TileImage {
            pixels: PixelBuffer::empty(2, 2, ColorMode::Rgba),
        });
        sprite.tilesets.push(tileset);
        (sprite, CelMap::new())
    }

    fn pixel(sprite: &Sprite, ts: TilesetId, tile_id: u32, x: u32, y: u32) -> Rgba {
        let tileset = sprite.tileset(ts).expect("tileset present");
        let tile = tileset.tile(tile_id).expect("tile present");
        let off = ((y * tile.pixels.width + x) * 4) as usize;
        Rgba {
            r: tile.pixels.data[off],
            g: tile.pixels.data[off + 1],
            b: tile.pixels.data[off + 2],
            a: tile.pixels.data[off + 3],
        }
    }

    #[test]
    fn apply_writes_pixel_and_records_previous() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetTilePixel::new(TilesetId::new(0), 0, 1, 0, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(pixel(&sprite, TilesetId::new(0), 0, 1, 0), Rgba::WHITE);
        assert_eq!(cmd.previous, Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn revert_restores_previous_pixel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetTilePixel::new(TilesetId::new(0), 0, 1, 1, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(
            pixel(&sprite, TilesetId::new(0), 0, 1, 1),
            Rgba::TRANSPARENT
        );
    }

    #[test]
    fn unknown_tileset_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetTilePixel::new(TilesetId::new(99), 0, 0, 0, Rgba::WHITE);
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels).unwrap_err(),
            CommandError::UnknownTileset(99)
        );
    }

    #[test]
    fn unknown_tile_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetTilePixel::new(TilesetId::new(0), 5, 0, 0, Rgba::WHITE);
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels).unwrap_err(),
            CommandError::UnknownTile {
                tileset: 0,
                tile_id: 5,
            }
        );
    }

    #[test]
    fn out_of_bounds_pixel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetTilePixel::new(TilesetId::new(0), 0, 5, 5, Rgba::WHITE);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert!(matches!(err, CommandError::TilePixelOutOfBounds { .. }));
    }
}
