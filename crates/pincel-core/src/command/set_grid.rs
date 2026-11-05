//! `SetGrid` command — change the sprite's editor grid (cell size and
//! offset). See `docs/specs/pincel.md` §7.1 and
//! `docs/specs/pixel-grid.md`.
//!
//! The grid is sprite state: it round-trips through the `.aseprite`
//! header (`grid_x` / `grid_y` as `i16`, `grid_width` / `grid_height` as
//! `u16`), so the command refuses anything the header cannot store and a
//! zero-sized cell. It is view decoration and contributes nothing to the
//! composite, so it reports [`DirtyRegion::None`].

use crate::document::{CelMap, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Set the sprite's grid to `grid` (`x` / `y` offset, `width` × `height`
/// cell size, all in sprite pixels).
#[derive(Debug, Clone)]
pub struct SetGrid {
    grid: Rect,
    /// Prior `Metadata::grid`, captured on `apply` for `revert`.
    prev: Option<Option<Rect>>,
}

impl SetGrid {
    /// A command that sets the grid to `grid`. Validation happens on
    /// `apply`.
    pub fn new(grid: Rect) -> Self {
        Self { grid, prev: None }
    }
}

/// True when `grid` fits the `.aseprite` header and has a non-empty cell:
/// size 1–65535, offset −32768–32767 on both axes.
fn grid_is_valid(grid: Rect) -> bool {
    let size_ok = |v: u32| (1..=u32::from(u16::MAX)).contains(&v);
    let offset_ok = |v: i32| i16::try_from(v).is_ok();
    size_ok(grid.width) && size_ok(grid.height) && offset_ok(grid.x) && offset_ok(grid.y)
}

impl Command for SetGrid {
    fn apply(&mut self, doc: &mut Sprite, _cels: &mut CelMap) -> Result<(), CommandError> {
        if !grid_is_valid(self.grid) {
            return Err(CommandError::InvalidGrid {
                x: self.grid.x,
                y: self.grid.y,
                width: self.grid.width,
                height: self.grid.height,
            });
        }
        self.prev = Some(doc.metadata.grid);
        doc.metadata.grid = Some(self.grid);
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, _cels: &mut CelMap) {
        if let Some(prev) = self.prev.take() {
            doc.metadata.grid = prev;
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        // The grid is drawn by the UI overlay; the composite is unchanged.
        DirtyRegion::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{read_aseprite, write_aseprite};
    use crate::command::Bus;
    use crate::document::{Frame, Layer, LayerId, Metadata};

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        (sprite, CelMap::new())
    }

    #[test]
    fn apply_sets_and_revert_restores_grid() {
        let (mut s, mut c) = doc();
        let mut cmd = SetGrid::new(Rect::new(3, -2, 8, 4));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(s.metadata.grid, Some(Rect::new(3, -2, 8, 4)));
        cmd.revert(&mut s, &mut c);
        assert_eq!(s.metadata.grid, None, "the unset grid comes back unset");
    }

    #[test]
    fn bus_undo_and_redo_step_the_grid_in_one_entry() {
        let (mut s, mut c) = doc();
        s.metadata.grid = Some(Rect::new(0, 0, 16, 16));
        let mut bus = Bus::new();
        bus.execute(SetGrid::new(Rect::new(4, 4, 32, 8)).into(), &mut s, &mut c)
            .expect("execute");
        assert_eq!(bus.undo_depth(), 1);
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(s.metadata.grid, Some(Rect::new(0, 0, 16, 16)));
        assert!(bus.redo(&mut s, &mut c).expect("redo"));
        assert_eq!(s.metadata.grid, Some(Rect::new(4, 4, 32, 8)));
    }

    #[test]
    fn apply_with_out_of_range_values_returns_err_and_keeps_grid() {
        for bad in [
            Rect::new(0, 0, 0, 16),
            Rect::new(0, 0, 16, 0),
            Rect::new(0, 0, 65536, 16),
            Rect::new(0, 0, 16, 65536),
            Rect::new(32768, 0, 16, 16),
            Rect::new(0, -32769, 16, 16),
        ] {
            let (mut s, mut c) = doc();
            s.metadata.grid = Some(Rect::new(1, 1, 16, 16));
            let mut cmd = SetGrid::new(bad);
            assert!(
                matches!(
                    cmd.apply(&mut s, &mut c),
                    Err(CommandError::InvalidGrid { .. })
                ),
                "{bad:?} refused"
            );
            assert_eq!(s.metadata.grid, Some(Rect::new(1, 1, 16, 16)), "{bad:?}");
            cmd.revert(&mut s, &mut c);
            assert_eq!(s.metadata.grid, Some(Rect::new(1, 1, 16, 16)), "{bad:?}");
        }
    }

    #[test]
    fn apply_accepts_the_range_edges() {
        let (mut s, mut c) = doc();
        for ok in [
            Rect::new(-32768, 32767, 1, 65535),
            Rect::new(32767, -32768, 65535, 1),
        ] {
            SetGrid::new(ok).apply(&mut s, &mut c).expect("in range");
            assert_eq!(s.metadata.grid, Some(ok));
        }
    }

    #[test]
    fn set_grid_survives_aseprite_save_and_reopen() {
        let (mut s, mut c) = doc();
        SetGrid::new(Rect::new(5, -3, 12, 20))
            .apply(&mut s, &mut c)
            .expect("apply");
        let mut bytes = Vec::new();
        write_aseprite(&s, &c, &mut bytes).expect("write");
        let back = read_aseprite(&bytes).expect("read");
        assert_eq!(back.sprite.metadata.grid, Some(Rect::new(5, -3, 12, 20)));
    }

    #[test]
    fn default_grid_is_sixteen_square_at_origin() {
        assert_eq!(Metadata::DEFAULT_GRID, Rect::new(0, 0, 16, 16));
        let (s, _) = doc();
        assert_eq!(s.metadata.grid_or_default(), Rect::new(0, 0, 16, 16));
    }

    #[test]
    fn dirty_region_is_none() {
        assert_eq!(
            SetGrid::new(Rect::new(0, 0, 16, 16)).dirty_region(),
            DirtyRegion::None
        );
    }
}
