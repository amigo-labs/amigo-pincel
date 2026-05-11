//! `PlaceTile` command — replace a single `TileRef` in a tilemap cel.
//!
//! Targets a `(layer, frame)` whose cel data is `CelData::Tilemap`, replaces
//! the `TileRef` at grid coordinate `(x, y)` with a new ref, and captures
//! the prior `TileRef` for revert. The Tilemap Stamp tool (M8.7) is the
//! primary caller.

use crate::document::{CelData, CelMap, FrameIndex, LayerId, Sprite, TileRef};

use super::Command;
use super::error::CommandError;

/// Place a single tile reference at `(grid_x, grid_y)` in the tilemap cel
/// at `(layer, frame)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceTile {
    layer: LayerId,
    frame: FrameIndex,
    grid_x: u32,
    grid_y: u32,
    new_tile: TileRef,
    /// `Some` after a successful `apply`; the prior `TileRef` used by `revert`.
    previous: Option<TileRef>,
}

impl PlaceTile {
    pub fn new(layer: LayerId, frame: FrameIndex, grid_x: u32, grid_y: u32, tile: TileRef) -> Self {
        Self {
            layer,
            frame,
            grid_x,
            grid_y,
            new_tile: tile,
            previous: None,
        }
    }
}

impl Command for PlaceTile {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let cel = cels
            .get_mut(self.layer, self.frame)
            .ok_or(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            })?;
        let CelData::Tilemap {
            grid_w,
            grid_h,
            tiles,
        } = &mut cel.data
        else {
            return Err(CommandError::NotATilemapCel {
                layer: self.layer,
                frame: self.frame,
            });
        };
        if self.grid_x >= *grid_w || self.grid_y >= *grid_h {
            return Err(CommandError::TileCoordOutOfBounds {
                x: self.grid_x,
                y: self.grid_y,
                grid_w: *grid_w,
                grid_h: *grid_h,
            });
        }
        let index = (self.grid_y * *grid_w + self.grid_x) as usize;
        self.previous = Some(tiles[index]);
        tiles[index] = self.new_tile;
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(prior) = self.previous.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Tilemap {
            grid_w,
            grid_h,
            tiles,
        } = &mut cel.data
        else {
            return;
        };
        if self.grid_x >= *grid_w || self.grid_y >= *grid_h {
            return;
        }
        let index = (self.grid_y * *grid_w + self.grid_x) as usize;
        tiles[index] = prior;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, Layer, TilesetId};

    fn fixture(grid_w: u32, grid_h: u32) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(grid_w * 8, grid_h * 8)
            .add_layer(Layer::tilemap(LayerId::new(1), "tm", TilesetId::new(0)))
            .add_frame(crate::document::Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::tilemap(
            LayerId::new(1),
            FrameIndex::new(0),
            grid_w,
            grid_h,
        ));
        (sprite, cels)
    }

    fn tile_at(cels: &CelMap, grid_w: u32, x: u32, y: u32) -> TileRef {
        let cel = cels
            .get(LayerId::new(1), FrameIndex::new(0))
            .expect("cel exists");
        match &cel.data {
            CelData::Tilemap { tiles, .. } => tiles[(y * grid_w + x) as usize],
            _ => panic!("expected tilemap cel"),
        }
    }

    #[test]
    fn apply_writes_tile_and_records_previous() {
        let (mut sprite, mut cels) = fixture(3, 3);
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 1, 2, TileRef::new(7));
        cmd.apply(&mut sprite, &mut cels).expect("apply succeeds");
        assert_eq!(tile_at(&cels, 3, 1, 2), TileRef::new(7));
        assert_eq!(cmd.previous, Some(TileRef::EMPTY));
    }

    #[test]
    fn revert_restores_previous_tile() {
        let (mut sprite, mut cels) = fixture(2, 2);
        // Pre-seed (0, 0) with a non-empty tile.
        if let Some(cel) = cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            && let CelData::Tilemap { tiles, .. } = &mut cel.data
        {
            tiles[0] = TileRef::new(3);
        }
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 0, 0, TileRef::new(9));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        assert_eq!(tile_at(&cels, 2, 0, 0), TileRef::new(9));
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(tile_at(&cels, 2, 0, 0), TileRef::new(3));
    }

    #[test]
    fn missing_cel_yields_error() {
        let (mut sprite, mut cels) = fixture(2, 2);
        let mut cmd = PlaceTile::new(LayerId::new(99), FrameIndex::new(0), 0, 0, TileRef::new(1));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::MissingCel {
                layer: LayerId::new(99),
                frame: FrameIndex::new(0),
            })
        );
    }

    #[test]
    fn image_cel_yields_not_a_tilemap_cel_error() {
        let sprite = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(1), "bg"))
            .add_frame(crate::document::Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            crate::document::PixelBuffer::empty(8, 8, crate::document::ColorMode::Rgba),
        ));
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 0, 0, TileRef::new(1));
        assert_eq!(
            cmd.apply(&mut sprite.clone(), &mut cels),
            Err(CommandError::NotATilemapCel {
                layer: LayerId::new(1),
                frame: FrameIndex::new(0),
            })
        );
    }

    #[test]
    fn out_of_bounds_coord_yields_error() {
        let (mut sprite, mut cels) = fixture(2, 2);
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 5, 0, TileRef::new(1));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::TileCoordOutOfBounds {
                x: 5,
                y: 0,
                grid_w: 2,
                grid_h: 2,
            })
        );
    }

    #[test]
    fn does_not_merge_with_other_place_tile() {
        let mut a = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 0, 0, TileRef::new(1));
        let b = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 1, 0, TileRef::new(2));
        assert!(!a.merge(&b));
    }
}
