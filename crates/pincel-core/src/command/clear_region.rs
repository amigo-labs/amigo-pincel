//! `ClearRegion` command — clear the pixels inside a sprite-space rect on
//! one image cel to transparent, undoably.
//!
//! Backs the "Delete / Backspace clears the selection" UX: the wasm layer
//! passes the active selection rect and target `(layer, frame)`. On
//! `apply` the rect is intersected with the cel buffer, the covered pixels
//! are captured and set to transparent; `revert` writes them back.
//!
//! Tolerant by design: a missing cel or a rect that doesn't overlap the
//! cel is a successful no-op (nothing to clear), matching Aseprite's
//! behavior when deleting an empty area. Non-image / non-RGBA cels are
//! rejected (Phase 1 RGBA-first).

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;
use super::move_selection_content::{intersect_with_cel, read_pixel, write_pixel};

/// A pixel captured for `revert`: cel-local coords plus its prior color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriorPixel {
    local_x: u32,
    local_y: u32,
    prior: Rgba,
}

/// Clear the pixels inside `rect` (sprite-space) on `(layer, frame)` to
/// transparent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClearRegion {
    layer: LayerId,
    frame: FrameIndex,
    rect: Rect,
    /// `Some` after a successful `apply`; the pixels that were cleared,
    /// for `revert`. Empty when the rect didn't overlap the cel.
    cleared: Option<Vec<PriorPixel>>,
    /// Cel-local dirty bbox from the last apply, preserved across revert.
    dirty_bbox: Option<Rect>,
}

impl ClearRegion {
    pub fn new(layer: LayerId, frame: FrameIndex, rect: Rect) -> Self {
        Self {
            layer,
            frame,
            rect,
            cleared: None,
            dirty_bbox: None,
        }
    }

    /// Number of pixels cleared by the most recent `apply` (0 before
    /// `apply` or after `revert`).
    pub fn cleared_count(&self) -> usize {
        self.cleared.as_ref().map(|c| c.len()).unwrap_or(0)
    }
}

impl Command for ClearRegion {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        // Missing cel → nothing to clear (successful no-op).
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            self.cleared = Some(Vec::new());
            self.dirty_bbox = None;
            return Ok(());
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

        let mut cleared = Vec::new();
        if let Some((sx, sy, sw, sh)) =
            intersect_with_cel(self.rect, cel_pos, buffer.width, buffer.height)
        {
            for ly in sy..sy + sh {
                for lx in sx..sx + sw {
                    let prior = read_pixel(buffer, lx, ly);
                    // Skip already-transparent pixels so undo of a delete
                    // over mostly-empty space stays cheap.
                    if prior != Rgba::TRANSPARENT {
                        cleared.push(PriorPixel {
                            local_x: lx,
                            local_y: ly,
                            prior,
                        });
                        write_pixel(buffer, lx, ly, Rgba::TRANSPARENT);
                    }
                }
            }
            // Dirty bbox in sprite coords = the cleared sub-rect shifted
            // back by the cel position.
            self.dirty_bbox = Some(Rect::new(
                cel_pos.0 + sx as i32,
                cel_pos.1 + sy as i32,
                sw,
                sh,
            ));
        } else {
            self.dirty_bbox = None;
        }
        self.cleared = Some(cleared);
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(cleared) = self.cleared.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        for p in cleared {
            if p.local_x < buffer.width && p.local_y < buffer.height {
                write_pixel(buffer, p.local_x, p.local_y, p.prior);
            }
        }
    }

    fn dirty_region(&self) -> DirtyRegion {
        match self.dirty_bbox {
            Some(rect) => DirtyRegion::layer_rect(self.layer, self.frame, rect),
            None => DirtyRegion::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, Frame, Layer, PixelBuffer};

    const RED: Rgba = Rgba {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };

    fn doc_with_filled_cel() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut buffer = PixelBuffer::empty(4, 4, ColorMode::Rgba);
        for i in 0..(4 * 4) {
            let off = i * 4;
            buffer.data[off] = 255;
            buffer.data[off + 3] = 255; // opaque red
        }
        let mut cels = CelMap::new();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buffer));
        (sprite, cels)
    }

    #[test]
    fn clear_sets_region_transparent_and_revert_restores() {
        let (mut s, mut c) = doc_with_filled_cel();
        // Clear the top-left 2x2.
        let mut cmd = ClearRegion::new(LayerId::new(0), FrameIndex::new(0), Rect::new(0, 0, 2, 2));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(cmd.cleared_count(), 4);
        let cel = c.get(LayerId::new(0), FrameIndex::new(0)).unwrap();
        let CelData::Image(buf) = &cel.data else {
            panic!("image")
        };
        assert_eq!(read_pixel(buf, 0, 0), Rgba::TRANSPARENT);
        assert_eq!(read_pixel(buf, 1, 1), Rgba::TRANSPARENT);
        assert_eq!(read_pixel(buf, 2, 2), RED, "outside the rect untouched");
        cmd.revert(&mut s, &mut c);
        let cel = c.get(LayerId::new(0), FrameIndex::new(0)).unwrap();
        let CelData::Image(buf) = &cel.data else {
            panic!("image")
        };
        assert_eq!(read_pixel(buf, 0, 0), RED, "revert restores");
    }

    #[test]
    fn clear_with_no_cel_is_a_noop() {
        let sprite = Sprite::builder(4, 4)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("builds");
        let mut cels = CelMap::new();
        let mut cmd = ClearRegion::new(LayerId::new(0), FrameIndex::new(0), Rect::new(0, 0, 2, 2));
        cmd.apply(&mut sprite.clone(), &mut cels).expect("no-op ok");
        assert_eq!(cmd.cleared_count(), 0);
    }

    #[test]
    fn clear_off_cel_rect_clears_nothing() {
        let (mut s, mut c) = doc_with_filled_cel();
        let mut cmd =
            ClearRegion::new(LayerId::new(0), FrameIndex::new(0), Rect::new(10, 10, 2, 2));
        cmd.apply(&mut s, &mut c).expect("apply");
        assert_eq!(cmd.cleared_count(), 0);
    }
}
