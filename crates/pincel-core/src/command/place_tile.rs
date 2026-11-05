//! `PlaceTile` command — replace a single `TileRef` in a tilemap cel.
//!
//! Targets a `(layer, frame)` whose cel data is `CelData::Tilemap`, replaces
//! the `TileRef` at grid coordinate `(x, y)` with a new ref, and captures
//! the prior `TileRef` for revert. The Tilemap Stamp tool (M8.7) is the
//! primary caller.
//!
//! A tilemap layer has no grid on a frame added after it (`AddFrame` writes
//! no empty cels). Placing a tile there first creates the layer's empty
//! grid for that frame, in the same undo step (docs/specs/tilemaps.md
//! AC-21).

use crate::document::{Cel, CelData, CelMap, FrameIndex, LayerId, LayerKind, Sprite, TileRef};

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
    /// `true` after an `apply` that created the empty grid; `revert`
    /// removes that grid again.
    created_grid: bool,
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
            created_grid: false,
        }
    }

    /// The empty grid of the target layer for a frame that has none:
    /// ⌈sprite size ∕ tile size⌉ cells, as `AddTilemapLayer` seeds it.
    /// `None` when the layer is not a tilemap layer, its tileset is gone
    /// or the frame does not exist.
    fn empty_grid(&self, doc: &Sprite) -> Option<Cel> {
        if self.frame.0 as usize >= doc.frames.len() {
            return None;
        }
        let LayerKind::Tilemap { tileset_id } = doc.layer(self.layer)?.kind else {
            return None;
        };
        let (tile_w, tile_h) = doc.tileset(tileset_id)?.tile_size;
        if tile_w == 0 || tile_h == 0 {
            return None;
        }
        Some(Cel::tilemap(
            self.layer,
            self.frame,
            doc.width.div_ceil(tile_w),
            doc.height.div_ceil(tile_h),
        ))
    }

    /// Writes the tile into the existing grid and records the prior ref.
    fn place(&mut self, cels: &mut CelMap) -> Result<(), CommandError> {
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
        // Compute in `usize` so the multiply can't overflow on a 64-bit
        // target even for absurdly large grids. The post-bounds-check
        // length validation guarantees indexing is in range.
        let expected_len = (*grid_w as usize) * (*grid_h as usize);
        if tiles.len() != expected_len {
            return Err(CommandError::MalformedTilemapCel {
                layer: self.layer,
                frame: self.frame,
                grid_w: *grid_w,
                grid_h: *grid_h,
                tiles_len: tiles.len(),
            });
        }
        let index = (self.grid_y as usize) * (*grid_w as usize) + (self.grid_x as usize);
        self.previous = Some(tiles[index]);
        tiles[index] = self.new_tile;
        Ok(())
    }
}

impl Command for PlaceTile {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        self.created_grid = false;
        if cels.get(self.layer, self.frame).is_none()
            && let Some(grid) = self.empty_grid(doc)
        {
            cels.insert(grid);
            self.created_grid = true;
        }
        let result = self.place(cels);
        if result.is_err() && self.created_grid {
            // A refused placement leaves no empty grid behind.
            cels.remove(self.layer, self.frame);
            self.created_grid = false;
        }
        result
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(prior) = self.previous.take() else {
            return;
        };
        if self.created_grid {
            cels.remove(self.layer, self.frame);
            self.created_grid = false;
            return;
        }
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
        let expected_len = (*grid_w as usize) * (*grid_h as usize);
        if tiles.len() != expected_len {
            return;
        }
        let index = (self.grid_y as usize) * (*grid_w as usize) + (self.grid_x as usize);
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
    fn malformed_tilemap_cel_is_rejected() {
        // Manually corrupt the cel so `tiles.len() != grid_w * grid_h`.
        let (mut sprite, mut cels) = fixture(2, 2);
        if let Some(cel) = cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            && let CelData::Tilemap { tiles, .. } = &mut cel.data
        {
            tiles.pop();
        }
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 0, 0, TileRef::new(1));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::MalformedTilemapCel {
                layer: LayerId::new(1),
                frame: FrameIndex::new(0),
                grid_w: 2,
                grid_h: 2,
                tiles_len: 3,
            })
        );
    }

    #[test]
    fn does_not_merge_with_other_place_tile() {
        let mut a = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 0, 0, TileRef::new(1));
        let b = PlaceTile::new(LayerId::new(1), FrameIndex::new(0), 1, 0, TileRef::new(2));
        assert!(!a.merge(&b));
    }

    /// An 8×6 sprite with 4×4 tiles (a 2×2 grid), a tilemap layer and two
    /// frames, of which only frame 0 has a grid — frame 1 was added after
    /// the layer.
    fn fixture_frame_without_grid() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 6)
            .add_tileset(crate::document::Tileset::new(
                TilesetId::new(0),
                "ts",
                (4, 4),
            ))
            .add_layer(Layer::tilemap(LayerId::new(1), "tm", TilesetId::new(0)))
            .add_frame(crate::document::Frame::new(100))
            .add_frame(crate::document::Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::tilemap(LayerId::new(1), FrameIndex::new(0), 2, 2));
        (sprite, cels)
    }

    #[test]
    fn apply_on_a_frame_without_a_grid_creates_the_empty_grid_and_places() {
        let (mut sprite, mut cels) = fixture_frame_without_grid();
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(1), 1, 1, TileRef::new(5));
        cmd.apply(&mut sprite, &mut cels)
            .expect("apply creates the grid");
        let cel = cels
            .get(LayerId::new(1), FrameIndex::new(1))
            .expect("grid created for frame 1");
        let CelData::Tilemap {
            grid_w,
            grid_h,
            tiles,
        } = &cel.data
        else {
            panic!("expected tilemap cel");
        };
        // ⌈8 / 4⌉ × ⌈6 / 4⌉, as AddTilemapLayer seeds it.
        assert_eq!((*grid_w, *grid_h), (2, 2));
        assert_eq!(
            tiles.as_slice(),
            &[
                TileRef::EMPTY,
                TileRef::EMPTY,
                TileRef::EMPTY,
                TileRef::new(5)
            ]
        );
    }

    #[test]
    fn revert_removes_the_grid_apply_created() {
        let (mut sprite, mut cels) = fixture_frame_without_grid();
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(1), 0, 0, TileRef::new(5));
        cmd.apply(&mut sprite, &mut cels).expect("apply");
        cmd.revert(&mut sprite, &mut cels);
        assert!(cels.get(LayerId::new(1), FrameIndex::new(1)).is_none());
        // The grid that existed before stays.
        assert!(cels.get(LayerId::new(1), FrameIndex::new(0)).is_some());
        // Redo creates it again.
        cmd.apply(&mut sprite, &mut cels).expect("redo");
        assert!(cels.get(LayerId::new(1), FrameIndex::new(1)).is_some());
    }

    #[test]
    fn refused_placement_on_a_frame_without_a_grid_creates_nothing() {
        let (mut sprite, mut cels) = fixture_frame_without_grid();
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(1), 2, 0, TileRef::new(5));
        assert!(matches!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::TileCoordOutOfBounds { .. })
        ));
        assert!(cels.get(LayerId::new(1), FrameIndex::new(1)).is_none());
    }

    #[test]
    fn frame_past_the_end_still_yields_missing_cel() {
        let (mut sprite, mut cels) = fixture_frame_without_grid();
        let mut cmd = PlaceTile::new(LayerId::new(1), FrameIndex::new(2), 0, 0, TileRef::new(5));
        assert_eq!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::MissingCel {
                layer: LayerId::new(1),
                frame: FrameIndex::new(2),
            })
        );
    }
}
