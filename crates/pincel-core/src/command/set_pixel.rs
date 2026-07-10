//! `SetPixel` command — write RGBA pixels into an image cel.
//!
//! Pixel coordinates are sprite-global; the command translates them into the
//! cel's local buffer space via the cel's `position`. Indexed and grayscale
//! cels are rejected for now (Phase 1 RGBA-first; see `docs/specs/pincel.md`
//! §4.1).
//!
//! A `SetPixel` starts as a single-pixel write but [`Command::merge`]s with
//! the next `SetPixel` on the same `(layer, frame)`, so one press-drag-
//! release pencil / eraser stroke coalesces into a single undo entry instead
//! of flooding the history cap with one entry per pixel. The bus applies a
//! command *before* offering it for merging, so absorbed writes always carry
//! their recorded prior values. Strokes are delimited by
//! [`super::Bus::seal`].

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// One recorded pixel write within a [`SetPixel`] stroke.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PixelWrite {
    sprite_x: i32,
    sprite_y: i32,
    new_color: Rgba,
    /// `Some` after a successful `apply`; carries the prior pixel value used
    /// by `revert`.
    previous: Option<Rgba>,
}

/// Write one or more pixels into an image cel (one entry per stroke).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetPixel {
    layer: LayerId,
    frame: FrameIndex,
    /// In application order. Never empty — `new` seeds the first write.
    writes: Vec<PixelWrite>,
}

impl SetPixel {
    /// Build a new `SetPixel` targeting `(sprite_x, sprite_y)` on the cel at
    /// `(layer, frame)`.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        sprite_x: i32,
        sprite_y: i32,
        color: Rgba,
    ) -> Self {
        Self {
            layer,
            frame,
            writes: vec![PixelWrite {
                sprite_x,
                sprite_y,
                new_color: color,
                previous: None,
            }],
        }
    }

    /// Write one pixel, recording the prior value into `write.previous`.
    fn apply_write(
        cels: &mut CelMap,
        layer: LayerId,
        frame: FrameIndex,
        write: &mut PixelWrite,
    ) -> Result<(), CommandError> {
        let cel = cels
            .get_mut(layer, frame)
            .ok_or(CommandError::MissingCel { layer, frame })?;

        let CelData::Image(buffer) = &mut cel.data else {
            return Err(CommandError::NotAnImageCel { layer, frame });
        };

        if !matches!(buffer.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }

        let out_of_bounds = || CommandError::PixelOutOfBounds {
            x: write.sprite_x,
            y: write.sprite_y,
            width: buffer.width,
            height: buffer.height,
            position: cel.position,
        };
        let local =
            local_coords(write.sprite_x, write.sprite_y, cel.position).ok_or_else(out_of_bounds)?;
        if local.0 >= buffer.width || local.1 >= buffer.height {
            return Err(out_of_bounds());
        }

        let offset = ((local.1 * buffer.width + local.0) * 4) as usize;
        let prior = Rgba {
            r: buffer.data[offset],
            g: buffer.data[offset + 1],
            b: buffer.data[offset + 2],
            a: buffer.data[offset + 3],
        };
        buffer.data[offset] = write.new_color.r;
        buffer.data[offset + 1] = write.new_color.g;
        buffer.data[offset + 2] = write.new_color.b;
        buffer.data[offset + 3] = write.new_color.a;
        write.previous = Some(prior);
        Ok(())
    }

    /// Restore one pixel from `write.previous` (consumed). Skips writes that
    /// were never applied or whose cel has since vanished.
    fn revert_write(cels: &mut CelMap, layer: LayerId, frame: FrameIndex, write: &mut PixelWrite) {
        let Some(prior) = write.previous.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(layer, frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        let Some(local) = local_coords(write.sprite_x, write.sprite_y, cel.position) else {
            return;
        };
        if local.0 >= buffer.width || local.1 >= buffer.height {
            return;
        }
        let offset = ((local.1 * buffer.width + local.0) * 4) as usize;
        buffer.data[offset] = prior.r;
        buffer.data[offset + 1] = prior.g;
        buffer.data[offset + 2] = prior.b;
        buffer.data[offset + 3] = prior.a;
    }
}

impl Command for SetPixel {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        // Apply in order; on failure roll back the writes already applied
        // (in reverse) so a failed apply — reachable via `Bus::redo` on a
        // merged stroke — leaves the document unchanged.
        for i in 0..self.writes.len() {
            if let Err(e) = Self::apply_write(cels, self.layer, self.frame, &mut self.writes[i]) {
                for write in self.writes[..i].iter_mut().rev() {
                    Self::revert_write(cels, self.layer, self.frame, write);
                }
                return Err(e);
            }
        }
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        // Reverse order is load-bearing: a stroke that crosses the same
        // pixel twice must restore the value from before the *first* write.
        for write in self.writes.iter_mut().rev() {
            Self::revert_write(cels, self.layer, self.frame, write);
        }
    }

    fn merge(&mut self, next: &Self) -> bool {
        if self.layer != next.layer || self.frame != next.frame {
            return false;
        }
        self.writes.extend(next.writes.iter().cloned());
        true
    }

    fn dirty_region(&self) -> DirtyRegion {
        let mut it = self.writes.iter();
        let Some(first) = it.next() else {
            return DirtyRegion::None;
        };
        let (mut min_x, mut min_y) = (first.sprite_x, first.sprite_y);
        let (mut max_x, mut max_y) = (first.sprite_x, first.sprite_y);
        for w in it {
            min_x = min_x.min(w.sprite_x);
            min_y = min_y.min(w.sprite_y);
            max_x = max_x.max(w.sprite_x);
            max_y = max_y.max(w.sprite_y);
        }
        DirtyRegion::bbox(self.layer, self.frame, min_x, min_y, max_x, max_y)
    }
}

fn local_coords(sprite_x: i32, sprite_y: i32, position: (i32, i32)) -> Option<(u32, u32)> {
    let lx = sprite_x.checked_sub(position.0)?;
    let ly = sprite_y.checked_sub(position.1)?;
    if lx < 0 || ly < 0 {
        return None;
    }
    Some((lx as u32, ly as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, ColorMode, PixelBuffer};
    use crate::geometry::Rect;

    fn fixture() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 8)
            .add_layer(crate::document::Layer::image(LayerId::new(1), "bg"))
            .add_frame(crate::document::Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBuffer::empty(8, 8, ColorMode::Rgba),
        ));
        (sprite, cels)
    }

    fn pixel(cels: &CelMap, x: u32, y: u32) -> Rgba {
        let cel = cels
            .get(LayerId::new(1), FrameIndex::new(0))
            .expect("cel exists");
        let CelData::Image(buf) = &cel.data else {
            panic!("expected image cel");
        };
        let off = ((y * buf.width + x) * 4) as usize;
        Rgba {
            r: buf.data[off],
            g: buf.data[off + 1],
            b: buf.data[off + 2],
            a: buf.data[off + 3],
        }
    }

    fn px(x: i32, y: i32, color: Rgba) -> SetPixel {
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), x, y, color)
    }

    #[test]
    fn apply_writes_pixel_and_records_previous() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = px(2, 3, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply succeeds");
        assert_eq!(pixel(&cels, 2, 3), Rgba::WHITE);
        assert_eq!(cmd.writes[0].previous, Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn revert_restores_previous_pixel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = px(4, 4, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply succeeds");
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(pixel(&cels, 4, 4), Rgba::TRANSPARENT);
    }

    #[test]
    fn apply_then_revert_then_apply_round_trip() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = px(0, 0, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply 1");
        cmd.revert(&mut sprite, &mut cels);
        cmd.apply(&mut sprite, &mut cels).expect("apply 2");
        assert_eq!(pixel(&cels, 0, 0), Rgba::WHITE);
    }

    #[test]
    fn missing_cel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = SetPixel::new(LayerId::new(99), FrameIndex::new(0), 0, 0, Rgba::WHITE);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert_eq!(
            err,
            CommandError::MissingCel {
                layer: LayerId::new(99),
                frame: FrameIndex::new(0),
            }
        );
    }

    #[test]
    fn out_of_bounds_pixel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = px(100, 100, Rgba::WHITE);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert!(matches!(err, CommandError::PixelOutOfBounds { .. }));
    }

    #[test]
    fn coords_outside_cel_position_are_rejected() {
        let (mut sprite, mut cels) = fixture();
        // Place cel offset to (4, 4); painting (3, 3) on sprite is outside the cel.
        cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            .unwrap()
            .position = (4, 4);
        let mut cmd = px(3, 3, Rgba::WHITE);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert!(matches!(err, CommandError::PixelOutOfBounds { .. }));
    }

    #[test]
    fn merge_same_layer_frame_absorbs_pixels() {
        let (mut sprite, mut cels) = fixture();
        let mut a = px(0, 0, Rgba::WHITE);
        a.apply(&mut sprite, &mut cels).expect("apply a");
        let mut b = px(1, 0, Rgba::WHITE);
        b.apply(&mut sprite, &mut cels).expect("apply b");
        assert!(a.merge(&b));
        assert_eq!(a.writes.len(), 2);
    }

    #[test]
    fn merge_rejects_different_layer_or_frame() {
        let mut a = px(0, 0, Rgba::WHITE);
        let other_layer = SetPixel::new(LayerId::new(2), FrameIndex::new(0), 1, 0, Rgba::WHITE);
        assert!(!a.merge(&other_layer));
        let other_frame = SetPixel::new(LayerId::new(1), FrameIndex::new(1), 1, 0, Rgba::WHITE);
        assert!(!a.merge(&other_frame));
    }

    #[test]
    fn revert_of_merged_stroke_restores_all_pixels() {
        let (mut sprite, mut cels) = fixture();
        let mut stroke = px(0, 0, Rgba::WHITE);
        stroke.apply(&mut sprite, &mut cels).expect("apply 0");
        for x in 1..4 {
            let mut next = px(x, 0, Rgba::WHITE);
            next.apply(&mut sprite, &mut cels).expect("apply next");
            assert!(stroke.merge(&next));
        }
        stroke.revert(&mut sprite, &mut cels);
        for x in 0..4 {
            assert_eq!(pixel(&cels, x, 0), Rgba::TRANSPARENT, "x={x}");
        }
    }

    #[test]
    fn revert_restores_twice_painted_pixel_to_original() {
        let (mut sprite, mut cels) = fixture();
        // Same pixel painted white then red within one stroke; revert must
        // restore the pre-stroke transparent, not the intermediate white.
        let mut stroke = px(1, 1, Rgba::WHITE);
        stroke.apply(&mut sprite, &mut cels).expect("apply white");
        let red = Rgba::new(255, 0, 0, 255);
        let mut second = px(1, 1, red);
        second.apply(&mut sprite, &mut cels).expect("apply red");
        assert!(stroke.merge(&second));
        assert_eq!(pixel(&cels, 1, 1), red);
        stroke.revert(&mut sprite, &mut cels);
        assert_eq!(pixel(&cels, 1, 1), Rgba::TRANSPARENT);
    }

    #[test]
    fn apply_of_merged_command_supports_redo() {
        let (mut sprite, mut cels) = fixture();
        let mut stroke = px(0, 0, Rgba::WHITE);
        stroke.apply(&mut sprite, &mut cels).expect("apply 0");
        let mut next = px(3, 2, Rgba::WHITE);
        next.apply(&mut sprite, &mut cels).expect("apply next");
        assert!(stroke.merge(&next));
        stroke.revert(&mut sprite, &mut cels);
        stroke.apply(&mut sprite, &mut cels).expect("re-apply");
        assert_eq!(pixel(&cels, 0, 0), Rgba::WHITE);
        assert_eq!(pixel(&cels, 3, 2), Rgba::WHITE);
    }

    #[test]
    fn failed_apply_rolls_back_earlier_writes() {
        let (mut sprite, mut cels) = fixture();
        let mut stroke = px(0, 0, Rgba::WHITE);
        stroke.apply(&mut sprite, &mut cels).expect("apply 0");
        stroke.revert(&mut sprite, &mut cels);
        // Sneak an out-of-bounds write into the batch, then re-apply.
        stroke.writes.push(PixelWrite {
            sprite_x: 100,
            sprite_y: 100,
            new_color: Rgba::WHITE,
            previous: None,
        });
        let err = stroke.apply(&mut sprite, &mut cels).unwrap_err();
        assert!(matches!(err, CommandError::PixelOutOfBounds { .. }));
        assert_eq!(
            pixel(&cels, 0, 0),
            Rgba::TRANSPARENT,
            "first write rolled back"
        );
    }

    #[test]
    fn dirty_region_of_merged_stroke_is_bbox_union() {
        let (mut sprite, mut cels) = fixture();
        let mut stroke = px(2, 3, Rgba::WHITE);
        stroke.apply(&mut sprite, &mut cels).expect("apply 0");
        let mut next = px(5, 1, Rgba::WHITE);
        next.apply(&mut sprite, &mut cels).expect("apply next");
        assert!(stroke.merge(&next));
        assert_eq!(
            stroke.dirty_region(),
            DirtyRegion::layer_rect(LayerId::new(1), FrameIndex::new(0), Rect::new(2, 1, 4, 3),)
        );
    }
}
