//! `MoveSelectionContent` command — translate the pixels inside the
//! sprite's active marquee selection by a `(dx, dy)` offset.
//!
//! Sprite-space semantics: the command snapshots the current
//! [`Sprite::selection`] at construction; on `apply` it intersects the
//! selection with the target cel's pixel buffer, copies the pixels
//! inside the intersection to the translated location, clears the
//! source pixels to transparent, and updates `Sprite::selection` to
//! the translated rect. Pixels whose destination falls outside the
//! cel buffer are dropped — Phase 1 does not auto-grow the cel (Spec
//! §5.2 Move).
//!
//! Source / destination overlap is handled correctly: the apply path
//! captures source and destination pixels before any writes, so the
//! command stores enough state to reverse the move regardless of how
//! the rectangles overlap.
//!
//! Indexed and grayscale cels are rejected (Phase 1 RGBA-first; see
//! `docs/specs/pincel.md` §4.1 / §5.2).

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::error::CommandError;

/// Move the pixels inside the sprite's active selection by
/// `(delta_x, delta_y)` (sprite-space). The selection rect tracks
/// along with the pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveSelectionContent {
    layer: LayerId,
    frame: FrameIndex,
    delta_x: i32,
    delta_y: i32,
    /// `Some` after a successful `apply`; carries every datum required
    /// to undo the move. `None` before `apply` or after `revert`.
    state: Option<AppliedState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AppliedState {
    /// The `Sprite::selection` value before `apply`, restored verbatim.
    prior_selection: Option<Rect>,
    /// `(local_x, local_y, prior pixel)` for every source pixel that
    /// was cleared. Replayed after `overwritten_dest` so overlapping
    /// pixels end up at their original color.
    cleared_source: Vec<PriorPixel>,
    /// `(local_x, local_y, prior pixel)` for every destination pixel
    /// that was overwritten. Deduplicated by `(x, y)` so each pixel
    /// is restored once.
    overwritten_dest: Vec<PriorPixel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriorPixel {
    local_x: u32,
    local_y: u32,
    prior: Rgba,
}

impl MoveSelectionContent {
    /// Build a new `MoveSelectionContent`. The selection is read off
    /// the sprite at `apply` time; the constructor only takes the
    /// target `(layer, frame)` and the translation delta.
    pub fn new(layer: LayerId, frame: FrameIndex, delta_x: i32, delta_y: i32) -> Self {
        Self {
            layer,
            frame,
            delta_x,
            delta_y,
            state: None,
        }
    }

    /// Number of source pixels that were cleared by the most recent
    /// `apply`. Returns `0` before `apply` runs or after `revert`.
    pub fn moved_count(&self) -> usize {
        self.state
            .as_ref()
            .map(|s| s.cleared_source.len())
            .unwrap_or(0)
    }
}

impl Command for MoveSelectionContent {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        let selection = doc.selection.ok_or(CommandError::NoSelection)?;
        if selection.is_empty() {
            return Err(CommandError::NoSelection);
        }

        let cel = cels
            .get_mut(self.layer, self.frame)
            .ok_or(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            })?;

        let CelData::Image(buffer) = &mut cel.data else {
            return Err(CommandError::NotAnImageCel {
                layer: self.layer,
                frame: self.frame,
            });
        };

        if !matches!(buffer.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }

        let cel_pos = cel.position;
        let cel_w = buffer.width;
        let cel_h = buffer.height;

        // Source rect in cel-local coords, clipped to the cel buffer.
        // The selection is in sprite coords; cel-local = sprite -
        // cel.position. We clip both to `[0, cel_w)` × `[0, cel_h)`.
        let src_local = intersect_with_cel(selection, cel_pos, cel_w, cel_h);

        let mut cleared_source: Vec<PriorPixel> = Vec::new();
        // (dst_lx, dst_ly, pixel from source). Built first so we can
        // capture the original destination pixels before any writes.
        let mut moved: Vec<(u32, u32, Rgba)> = Vec::new();

        if let Some((sx, sy, sw, sh)) = src_local {
            for ly in sy..sy + sh {
                for lx in sx..sx + sw {
                    let prior = read_pixel(buffer, lx, ly);
                    cleared_source.push(PriorPixel {
                        local_x: lx,
                        local_y: ly,
                        prior,
                    });
                    // Destination cel-local coords. `delta` is sprite-
                    // space but cel-local is a pure shift, so adding
                    // `delta` directly is correct. `i64` keeps the
                    // arithmetic from overflowing on extreme deltas.
                    let dx_i = i64::from(lx) + i64::from(self.delta_x);
                    let dy_i = i64::from(ly) + i64::from(self.delta_y);
                    if dx_i < 0 || dy_i < 0 || dx_i >= i64::from(cel_w) || dy_i >= i64::from(cel_h)
                    {
                        continue;
                    }
                    moved.push((dx_i as u32, dy_i as u32, prior));
                }
            }
        }

        // Capture pre-apply destination pixels, deduping by (x, y) so
        // overlap with the source doesn't double-record (which would
        // make `revert` write the destination twice).
        let mut overwritten_dest: Vec<PriorPixel> = Vec::with_capacity(moved.len());
        let mut seen = vec![false; (cel_w as usize) * (cel_h as usize)];
        for &(dx, dy, _) in &moved {
            let idx = (dy as usize) * (cel_w as usize) + (dx as usize);
            if !seen[idx] {
                seen[idx] = true;
                overwritten_dest.push(PriorPixel {
                    local_x: dx,
                    local_y: dy,
                    prior: read_pixel(buffer, dx, dy),
                });
            }
        }

        // Mutate the buffer: clear the source rect first, then write
        // the moved pixels at their destinations. Pixels in src ∩ dst
        // are cleared then immediately overwritten with the source
        // value, so they end up at the source color.
        for p in &cleared_source {
            write_pixel(buffer, p.local_x, p.local_y, Rgba::TRANSPARENT);
        }
        for &(dx, dy, color) in &moved {
            write_pixel(buffer, dx, dy, color);
        }

        // Translate the active selection. The new rect mirrors the
        // sprite-space selection moved by `(delta_x, delta_y)`; the
        // model preserves rects that extend off-canvas (consumers
        // clip), and an empty translated rect would clear via
        // `set_selection`, but the original-selection emptiness check
        // above already ruled that out.
        let prior_selection = doc.selection;
        let translated = Rect::new(
            selection.x.saturating_add(self.delta_x),
            selection.y.saturating_add(self.delta_y),
            selection.width,
            selection.height,
        );
        doc.set_selection(translated);

        self.state = Some(AppliedState {
            prior_selection,
            cleared_source,
            overwritten_dest,
        });
        Ok(())
    }

    fn revert(&mut self, doc: &mut Sprite, cels: &mut CelMap) {
        let Some(state) = self.state.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        // Restore destination first so any overlap with source is
        // then re-overwritten by the source restore — the final state
        // matches the pre-apply pixels exactly.
        for p in state.overwritten_dest {
            if p.local_x < buffer.width && p.local_y < buffer.height {
                write_pixel(buffer, p.local_x, p.local_y, p.prior);
            }
        }
        for p in state.cleared_source {
            if p.local_x < buffer.width && p.local_y < buffer.height {
                write_pixel(buffer, p.local_x, p.local_y, p.prior);
            }
        }
        doc.selection = state.prior_selection;
    }
}

/// Intersect a sprite-space rect with the cel buffer's bounds and
/// return the resulting cel-local `(x, y, width, height)`. Returns
/// `None` when the intersection is empty.
fn intersect_with_cel(
    rect: Rect,
    cel_pos: (i32, i32),
    cel_w: u32,
    cel_h: u32,
) -> Option<(u32, u32, u32, u32)> {
    let cel_min_x = i64::from(cel_pos.0);
    let cel_min_y = i64::from(cel_pos.1);
    let cel_max_x = cel_min_x + i64::from(cel_w);
    let cel_max_y = cel_min_y + i64::from(cel_h);

    let r_min_x = i64::from(rect.x);
    let r_min_y = i64::from(rect.y);
    let r_max_x = r_min_x + i64::from(rect.width);
    let r_max_y = r_min_y + i64::from(rect.height);

    let lo_x = r_min_x.max(cel_min_x);
    let lo_y = r_min_y.max(cel_min_y);
    let hi_x = r_max_x.min(cel_max_x);
    let hi_y = r_max_y.min(cel_max_y);
    if lo_x >= hi_x || lo_y >= hi_y {
        return None;
    }
    let local_x = (lo_x - cel_min_x) as u32;
    let local_y = (lo_y - cel_min_y) as u32;
    let local_w = (hi_x - lo_x) as u32;
    let local_h = (hi_y - lo_y) as u32;
    Some((local_x, local_y, local_w, local_h))
}

fn read_pixel(buf: &crate::document::PixelBuffer, x: u32, y: u32) -> Rgba {
    let off = ((y * buf.width + x) * 4) as usize;
    Rgba {
        r: buf.data[off],
        g: buf.data[off + 1],
        b: buf.data[off + 2],
        a: buf.data[off + 3],
    }
}

fn write_pixel(buf: &mut crate::document::PixelBuffer, x: u32, y: u32, c: Rgba) {
    let off = ((y * buf.width + x) * 4) as usize;
    buf.data[off] = c.r;
    buf.data[off + 1] = c.g;
    buf.data[off + 2] = c.b;
    buf.data[off + 3] = c.a;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, ColorMode, Frame, Layer, PixelBuffer};

    const RED: Rgba = Rgba {
        r: 255,
        g: 0,
        b: 0,
        a: 255,
    };
    const BLUE: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 255,
        a: 255,
    };

    fn fixture() -> (Sprite, CelMap) {
        let sprite = Sprite::builder(8, 8)
            .add_layer(Layer::image(LayerId::new(1), "bg"))
            .add_frame(Frame::new(100))
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
        read_pixel(buf, x, y)
    }

    fn write(cels: &mut CelMap, x: u32, y: u32, color: Rgba) {
        let cel = cels
            .get_mut(LayerId::new(1), FrameIndex::new(0))
            .expect("cel exists");
        let CelData::Image(buf) = &mut cel.data else {
            panic!("expected image cel");
        };
        write_pixel(buf, x, y, color);
    }

    #[test]
    fn move_translates_pixels_and_selection() {
        let (mut sprite, mut cels) = fixture();
        // Paint a 2×2 RED square at (1, 1).
        write(&mut cels, 1, 1, RED);
        write(&mut cels, 2, 1, RED);
        write(&mut cels, 1, 2, RED);
        write(&mut cels, 2, 2, RED);
        sprite.set_selection(Rect::new(1, 1, 2, 2));
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 3, 2);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // Source cleared.
        assert_eq!(pixel(&cels, 1, 1), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 2, 1), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 1, 2), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 2, 2), Rgba::TRANSPARENT);
        // Destination painted.
        assert_eq!(pixel(&cels, 4, 3), RED);
        assert_eq!(pixel(&cels, 5, 3), RED);
        assert_eq!(pixel(&cels, 4, 4), RED);
        assert_eq!(pixel(&cels, 5, 4), RED);
        // Selection translated.
        assert_eq!(sprite.selection, Some(Rect::new(4, 3, 2, 2)));
        assert_eq!(cmd.moved_count(), 4);
    }

    #[test]
    fn move_with_overlapping_source_and_destination() {
        let (mut sprite, mut cels) = fixture();
        // 3×1 stripe at row 4: RED RED BLUE at (1, 4) (2, 4) (3, 4).
        write(&mut cels, 1, 4, RED);
        write(&mut cels, 2, 4, RED);
        write(&mut cels, 3, 4, BLUE);
        // Select (1, 4)..(2, 4), move +1 to the right; destination
        // overlaps with the source (column 2).
        sprite.set_selection(Rect::new(1, 4, 2, 1));
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 1, 0);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // (1, 4) cleared (source-only).
        assert_eq!(pixel(&cels, 1, 4), Rgba::TRANSPARENT);
        // (2, 4) overlap: cleared then overwritten with RED from src(1,4).
        assert_eq!(pixel(&cels, 2, 4), RED);
        // (3, 4) destination-only: overwritten with RED from src(2,4).
        assert_eq!(pixel(&cels, 3, 4), RED);
    }

    #[test]
    fn revert_restores_source_destination_and_selection() {
        let (mut sprite, mut cels) = fixture();
        // Paint mixed src + dst: RED at (1,1)(2,1), BLUE at (4,1)(5,1).
        write(&mut cels, 1, 1, RED);
        write(&mut cels, 2, 1, RED);
        write(&mut cels, 4, 1, BLUE);
        write(&mut cels, 5, 1, BLUE);
        sprite.set_selection(Rect::new(1, 1, 2, 1));
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 3, 0);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        // Source restored.
        assert_eq!(pixel(&cels, 1, 1), RED);
        assert_eq!(pixel(&cels, 2, 1), RED);
        // Destination restored.
        assert_eq!(pixel(&cels, 4, 1), BLUE);
        assert_eq!(pixel(&cels, 5, 1), BLUE);
        // Selection restored.
        assert_eq!(sprite.selection, Some(Rect::new(1, 1, 2, 1)));
    }

    #[test]
    fn revert_with_overlapping_rects_restores_originals() {
        let (mut sprite, mut cels) = fixture();
        write(&mut cels, 1, 4, RED);
        write(&mut cels, 2, 4, RED);
        write(&mut cels, 3, 4, BLUE);
        sprite.set_selection(Rect::new(1, 4, 2, 1));
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 1, 0);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(pixel(&cels, 1, 4), RED);
        assert_eq!(pixel(&cels, 2, 4), RED);
        assert_eq!(pixel(&cels, 3, 4), BLUE);
        assert_eq!(sprite.selection, Some(Rect::new(1, 4, 2, 1)));
    }

    #[test]
    fn move_with_no_selection_errors() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 1, 0);
        let err = cmd.apply(&mut sprite, &mut cels).unwrap_err();
        assert_eq!(err, CommandError::NoSelection);
    }

    #[test]
    fn destination_outside_cel_drops_pixels() {
        let (mut sprite, mut cels) = fixture();
        // Paint a RED pixel at (0, 0) and select it; move (-1, 0) so
        // the destination is off-cel. The source is still cleared,
        // but the destination pixel is silently dropped (no auto-grow
        // in Phase 1).
        write(&mut cels, 0, 0, RED);
        sprite.set_selection(Rect::new(0, 0, 1, 1));
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), -1, 0);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(pixel(&cels, 0, 0), Rgba::TRANSPARENT);
        // Revert restores the source even though the destination was
        // off-cel and never recorded an overwritten pixel.
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(pixel(&cels, 0, 0), RED);
    }

    #[test]
    fn selection_outside_cel_apply_is_a_noop_but_translates_selection() {
        let (mut sprite, mut cels) = fixture();
        sprite.set_selection(Rect::new(100, 100, 2, 2));
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 1, 1);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(cmd.moved_count(), 0);
        // Selection still translates even when no pixels move — the
        // marquee rides along with the (empty) drag.
        assert_eq!(sprite.selection, Some(Rect::new(101, 101, 2, 2)));
    }

    #[test]
    fn zero_delta_apply_then_revert_is_identity() {
        let (mut sprite, mut cels) = fixture();
        write(&mut cels, 2, 2, RED);
        sprite.set_selection(Rect::new(2, 2, 1, 1));
        let mut cmd = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 0, 0);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(pixel(&cels, 2, 2), RED);
        cmd.revert(&mut sprite, &mut cels);
        assert_eq!(pixel(&cels, 2, 2), RED);
        assert_eq!(sprite.selection, Some(Rect::new(2, 2, 1, 1)));
    }

    #[test]
    fn missing_cel_errors() {
        let (mut sprite, mut cels) = fixture();
        sprite.set_selection(Rect::new(0, 0, 2, 2));
        let mut cmd = MoveSelectionContent::new(LayerId::new(99), FrameIndex::new(0), 1, 0);
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
    fn does_not_merge_with_another_move() {
        let mut a = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 1, 0);
        let b = MoveSelectionContent::new(LayerId::new(1), FrameIndex::new(0), 1, 0);
        assert!(!a.merge(&b));
    }
}
