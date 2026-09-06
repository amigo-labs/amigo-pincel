//! `ReplaceCelPixels` command — swap an image cel's entire RGBA buffer for
//! a precomputed one, undoably.
//!
//! This is the generic "whole-cel result" command behind effects and
//! adjustments (spec §13.4): the host computes the new pixels (typically
//! via `pincel-effects`), and this command owns the undo bookkeeping. It
//! never interprets the pixels, so it also serves any future same-size
//! transform. `apply` and `revert` both *swap* the held buffer with the
//! cel's, so the command stores exactly one copy at all times and redo
//! costs nothing extra.

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// Replace the pixel buffer of the image cel at `(layer, frame)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaceCelPixels {
    layer: LayerId,
    frame: FrameIndex,
    /// Before `apply`: the incoming pixels. After `apply`: the prior
    /// pixels (swapped). `revert` swaps back.
    pixels: Vec<u8>,
    /// Sprite-space rect the host knows to be the changed area, if
    /// narrower than the whole cel (e.g. an effect restricted to the
    /// selection). `None` reports the full cel rect.
    dirty_hint: Option<Rect>,
    /// Cel rect captured on the last successful `apply`, for
    /// [`Command::dirty_region`].
    cel_rect: Option<Rect>,
    applied: bool,
}

impl ReplaceCelPixels {
    /// New command carrying the replacement `pixels` (RGBA8, row-major,
    /// must match the cel buffer's byte length exactly).
    pub fn new(layer: LayerId, frame: FrameIndex, pixels: Vec<u8>) -> Self {
        Self {
            layer,
            frame,
            pixels,
            dirty_hint: None,
            cel_rect: None,
            applied: false,
        }
    }

    /// Narrow the reported dirty region to `rect` (sprite space). The
    /// pixels are still replaced wholesale; only the repaint hint shrinks.
    pub fn with_dirty_hint(mut self, rect: Rect) -> Self {
        self.dirty_hint = Some(rect);
        self
    }
}

impl Command for ReplaceCelPixels {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return Err(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            });
        };
        let cel_pos = cel.position;
        let CelData::Image(buffer) = &mut cel.data else {
            return Err(CommandError::NotAnImageCel {
                layer: self.layer,
                frame: self.frame,
            });
        };
        if !matches!(buffer.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }
        if buffer.data.len() != self.pixels.len() {
            return Err(CommandError::BufferLengthMismatch {
                expected: buffer.data.len(),
                got: self.pixels.len(),
            });
        }
        std::mem::swap(&mut buffer.data, &mut self.pixels);
        self.cel_rect = Some(Rect::new(cel_pos.0, cel_pos.1, buffer.width, buffer.height));
        self.applied = true;
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        if !self.applied {
            return;
        }
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        if buffer.data.len() == self.pixels.len() {
            std::mem::swap(&mut buffer.data, &mut self.pixels);
        }
        self.applied = false;
    }

    fn dirty_region(&self) -> DirtyRegion {
        match (self.dirty_hint, self.cel_rect) {
            (Some(hint), Some(cel)) => {
                DirtyRegion::layer_rect(self.layer, self.frame, hint.intersect(cel))
            }
            (None, Some(cel)) => DirtyRegion::layer_rect(self.layer, self.frame, cel),
            (_, None) => DirtyRegion::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Bus;
    use crate::document::{Cel, Layer, PixelBuffer};

    const L: LayerId = LayerId::new(0);
    const F: FrameIndex = FrameIndex::new(0);

    fn doc() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(2, 2)
            .add_layer(Layer::image(L, "a"))
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        let mut buf = PixelBuffer::empty(2, 2, ColorMode::Rgba);
        buf.data = (0u8..16).collect();
        cels.insert(Cel::image(L, F, buf));
        (sprite, cels)
    }

    fn data(cels: &CelMap) -> Vec<u8> {
        match &cels.get(L, F).unwrap().data {
            CelData::Image(b) => b.data.clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn apply_swaps_in_new_pixels_and_revert_restores() {
        let (mut sprite, mut cels) = doc();
        let before = data(&cels);
        let new: Vec<u8> = vec![255; 16];
        let mut cmd = ReplaceCelPixels::new(L, F, new.clone());
        cmd.apply(&mut sprite, &mut cels).unwrap();
        assert_eq!(data(&cels), new);
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(data(&cels), before);
    }

    #[test]
    fn apply_with_wrong_length_is_rejected_and_leaves_cel_untouched() {
        let (mut sprite, mut cels) = doc();
        let before = data(&cels);
        let mut cmd = ReplaceCelPixels::new(L, F, vec![0; 8]);
        assert!(matches!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::BufferLengthMismatch {
                expected: 16,
                got: 8
            })
        ));
        assert_eq!(data(&cels), before);
        assert!(cmd.dirty_region().is_none());
    }

    #[test]
    fn apply_missing_cel_errors() {
        let (mut sprite, mut cels) = doc();
        let mut cmd = ReplaceCelPixels::new(LayerId::new(9), F, vec![0; 16]);
        assert!(matches!(
            cmd.apply(&mut sprite, &mut cels),
            Err(CommandError::MissingCel { .. })
        ));
    }

    #[test]
    fn revert_before_apply_is_a_noop() {
        let (mut sprite, mut cels) = doc();
        let before = data(&cels);
        let mut cmd = ReplaceCelPixels::new(L, F, vec![7; 16]);
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(data(&cels), before);
    }

    #[test]
    fn dirty_region_is_cel_rect_or_the_intersected_hint() {
        let (mut sprite, mut cels) = doc();
        let mut cmd = ReplaceCelPixels::new(L, F, vec![1; 16]);
        cmd.apply(&mut sprite, &mut cels).unwrap();
        assert_eq!(
            cmd.dirty_region(),
            DirtyRegion::layer_rect(L, F, Rect::new(0, 0, 2, 2))
        );
        let mut cmd =
            ReplaceCelPixels::new(L, F, vec![2; 16]).with_dirty_hint(Rect::new(1, 1, 5, 5));
        cmd.apply(&mut sprite, &mut cels).unwrap();
        assert_eq!(
            cmd.dirty_region(),
            DirtyRegion::layer_rect(L, F, Rect::new(1, 1, 1, 1))
        );
    }

    #[test]
    fn bus_undo_redo_round_trip_preserves_state() {
        let (mut sprite, mut cels) = doc();
        let before = data(&cels);
        let new: Vec<u8> = vec![9; 16];
        let mut bus = Bus::new();
        bus.execute(
            ReplaceCelPixels::new(L, F, new.clone()).into(),
            &mut sprite,
            &mut cels,
        )
        .unwrap();
        assert_eq!(data(&cels), new);
        assert!(bus.undo(&mut sprite, &mut cels));
        assert_eq!(data(&cels), before);
        assert!(bus.redo(&mut sprite, &mut cels).unwrap());
        assert_eq!(data(&cels), new);
    }
}
