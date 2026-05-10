//! `DrawLine` command — rasterize a 1-pixel-wide line into an image cel.
//!
//! Sprite-space endpoints are translated into cel-local coordinates via the
//! cel's `position`. Pixels that fall outside the cel buffer are skipped
//! silently — drawing tools naturally clip to the target. Indexed and
//! grayscale cels are rejected (Phase 1 RGBA-first; see
//! `docs/specs/pincel.md` §4.1 / §5.2).
//!
//! The traversal uses the integer Bresenham algorithm so the rasterized
//! pixel set is symmetric and reproducible across host and wasm targets.

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};

use super::Command;
use super::error::CommandError;

/// Write a 1-pixel-wide line between sprite-space `(x0, y0)` and `(x1, y1)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawLine {
    layer: LayerId,
    frame: FrameIndex,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    new_color: Rgba,
    /// `Some` after a successful `apply`; carries the prior pixel values used
    /// by `revert`. Stored in cel-local coordinates.
    previous: Option<Vec<PriorPixel>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriorPixel {
    local_x: u32,
    local_y: u32,
    prior: Rgba,
}

impl DrawLine {
    /// Build a new `DrawLine` between two sprite-space points on the cel at
    /// `(layer, frame)`.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: Rgba,
    ) -> Self {
        Self {
            layer,
            frame,
            x0,
            y0,
            x1,
            y1,
            new_color: color,
            previous: None,
        }
    }
}

impl Command for DrawLine {
    fn apply(&mut self, _doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
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

        let mut prior = Vec::new();
        for (sx, sy) in bresenham(self.x0, self.y0, self.x1, self.y1) {
            let Some((lx, ly)) = local_coords(sx, sy, cel.position) else {
                continue;
            };
            if lx >= buffer.width || ly >= buffer.height {
                continue;
            }
            let offset = ((ly * buffer.width + lx) * 4) as usize;
            let before = Rgba {
                r: buffer.data[offset],
                g: buffer.data[offset + 1],
                b: buffer.data[offset + 2],
                a: buffer.data[offset + 3],
            };
            buffer.data[offset] = self.new_color.r;
            buffer.data[offset + 1] = self.new_color.g;
            buffer.data[offset + 2] = self.new_color.b;
            buffer.data[offset + 3] = self.new_color.a;
            prior.push(PriorPixel {
                local_x: lx,
                local_y: ly,
                prior: before,
            });
        }
        self.previous = Some(prior);
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(prior) = self.previous.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        for entry in prior {
            if entry.local_x >= buffer.width || entry.local_y >= buffer.height {
                continue;
            }
            let offset = ((entry.local_y * buffer.width + entry.local_x) * 4) as usize;
            buffer.data[offset] = entry.prior.r;
            buffer.data[offset + 1] = entry.prior.g;
            buffer.data[offset + 2] = entry.prior.b;
            buffer.data[offset + 3] = entry.prior.a;
        }
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

/// Bresenham's line algorithm as a lazy iterator. Yields the integer pixel
/// coordinates the rasterized line covers, inclusive of both endpoints.
/// Equal endpoints yield a single pixel.
///
/// All internal math is done in `i64` so the iterator is well-defined for
/// any `i32` endpoint pair, including the `i32::MIN` / `i32::MAX` extremes
/// where the naive `(x1 - x0).abs()` would overflow. The emitted
/// coordinates stay within the closed interval `[min(x0, x1), max(x0, x1)]`
/// (and similarly for `y`), so each step casts back to `i32` safely.
#[must_use]
pub(crate) struct BresenhamIter {
    x: i64,
    y: i64,
    x1: i64,
    y1: i64,
    sx: i64,
    sy: i64,
    dx: i64,
    dy: i64,
    err: i64,
    done: bool,
}

impl Iterator for BresenhamIter {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let out = (self.x as i32, self.y as i32);
        if self.x == self.x1 && self.y == self.y1 {
            self.done = true;
        } else {
            let e2 = 2 * self.err;
            if e2 >= self.dy {
                self.err += self.dy;
                self.x += self.sx;
            }
            if e2 <= self.dx {
                self.err += self.dx;
                self.y += self.sy;
            }
        }
        Some(out)
    }
}

fn bresenham(x0: i32, y0: i32, x1: i32, y1: i32) -> BresenhamIter {
    let x0 = i64::from(x0);
    let y0 = i64::from(y0);
    let x1 = i64::from(x1);
    let y1 = i64::from(y1);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let err = dx + dy;
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    BresenhamIter {
        x: x0,
        y: y0,
        x1,
        y1,
        sx,
        sy,
        dx,
        dy,
        err,
        done: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, ColorMode, Frame, Layer, PixelBuffer};

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
        let off = ((y * buf.width + x) * 4) as usize;
        Rgba {
            r: buf.data[off],
            g: buf.data[off + 1],
            b: buf.data[off + 2],
            a: buf.data[off + 3],
        }
    }

    #[test]
    fn bresenham_horizontal_line_covers_each_x() {
        let pts: Vec<_> = bresenham(0, 3, 4, 3).collect();
        assert_eq!(pts, vec![(0, 3), (1, 3), (2, 3), (3, 3), (4, 3)]);
    }

    #[test]
    fn bresenham_vertical_line_covers_each_y() {
        let pts: Vec<_> = bresenham(2, 1, 2, 5).collect();
        assert_eq!(pts, vec![(2, 1), (2, 2), (2, 3), (2, 4), (2, 5)]);
    }

    #[test]
    fn bresenham_perfect_diagonal_covers_each_step() {
        let pts: Vec<_> = bresenham(0, 0, 3, 3).collect();
        assert_eq!(pts, vec![(0, 0), (1, 1), (2, 2), (3, 3)]);
    }

    #[test]
    fn bresenham_reverse_diagonal_is_symmetric() {
        let pts: Vec<_> = bresenham(3, 3, 0, 0).collect();
        assert_eq!(pts, vec![(3, 3), (2, 2), (1, 1), (0, 0)]);
    }

    #[test]
    fn bresenham_same_endpoint_yields_single_pixel() {
        let pts: Vec<_> = bresenham(2, 2, 2, 2).collect();
        assert_eq!(pts, vec![(2, 2)]);
    }

    #[test]
    fn bresenham_handles_full_i32_range_without_overflow() {
        // `(x1 - x0).abs()` on `i32` would overflow at these endpoints
        // (panic in debug, wrap in release). The i64-internal iterator
        // stays well-defined and advances by ±1 each step.
        let mut iter = bresenham(i32::MIN, 0, i32::MAX, 0);
        assert_eq!(iter.next(), Some((i32::MIN, 0)));
        assert_eq!(iter.next(), Some((i32::MIN + 1, 0)));
        assert_eq!(iter.next(), Some((i32::MIN + 2, 0)));
    }

    #[test]
    fn bresenham_handles_i32_min_endpoint() {
        // `(x1 - x0).abs()` at `i32::MIN` panics on i32 because
        // `i32::MIN.abs()` is undefined. Promotion to i64 sidesteps it.
        let pts: Vec<_> = bresenham(i32::MIN, 0, i32::MIN, 0).collect();
        assert_eq!(pts, vec![(i32::MIN, 0)]);
    }

    #[test]
    fn apply_writes_horizontal_pixels() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawLine::new(LayerId::new(1), FrameIndex::new(0), 1, 2, 4, 2, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for x in 1..=4 {
            assert_eq!(pixel(&cels, x, 2), Rgba::WHITE);
        }
        assert_eq!(pixel(&cels, 0, 2), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 5, 2), Rgba::TRANSPARENT);
    }

    #[test]
    fn apply_writes_diagonal_pixels() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawLine::new(LayerId::new(1), FrameIndex::new(0), 0, 0, 3, 3, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for i in 0..=3 {
            assert_eq!(pixel(&cels, i, i), Rgba::WHITE);
        }
    }

    #[test]
    fn apply_single_pixel_line_writes_one_pixel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawLine::new(LayerId::new(1), FrameIndex::new(0), 5, 5, 5, 5, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(pixel(&cels, 5, 5), Rgba::WHITE);
    }

    #[test]
    fn revert_restores_each_pixel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawLine::new(LayerId::new(1), FrameIndex::new(0), 0, 0, 7, 7, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        for i in 0..8u32 {
            assert_eq!(pixel(&cels, i, i), Rgba::TRANSPARENT);
        }
    }

    #[test]
    fn apply_skips_pixels_outside_cel_silently() {
        let (mut sprite, mut cels) = fixture();
        // Line extends past the right edge of the 8×8 cel; the in-bounds
        // pixels still land, the rest are dropped without an error.
        let mut cmd = DrawLine::new(
            LayerId::new(1),
            FrameIndex::new(0),
            6,
            4,
            12,
            4,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(pixel(&cels, 6, 4), Rgba::WHITE);
        assert_eq!(pixel(&cels, 7, 4), Rgba::WHITE);
    }

    #[test]
    fn apply_with_offset_cel_uses_local_coords() {
        let (mut sprite, mut cels) = fixture();
        cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            .unwrap()
            .position = (2, 2);
        // Sprite-space (3, 3) → local (1, 1). (1, 1) → local (-1, -1) is dropped.
        let mut cmd = DrawLine::new(LayerId::new(1), FrameIndex::new(0), 1, 3, 3, 3, Rgba::WHITE);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(pixel(&cels, 0, 1), Rgba::WHITE);
        assert_eq!(pixel(&cels, 1, 1), Rgba::WHITE);
    }

    #[test]
    fn missing_cel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawLine::new(
            LayerId::new(99),
            FrameIndex::new(0),
            0,
            0,
            1,
            1,
            Rgba::WHITE,
        );
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
    fn does_not_merge_with_another_draw_line() {
        let mut a = DrawLine::new(LayerId::new(1), FrameIndex::new(0), 0, 0, 1, 1, Rgba::WHITE);
        let b = DrawLine::new(LayerId::new(1), FrameIndex::new(0), 1, 1, 2, 2, Rgba::WHITE);
        assert!(!a.merge(&b));
    }
}
