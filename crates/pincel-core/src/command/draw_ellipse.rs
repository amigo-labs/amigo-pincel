//! `DrawEllipse` command — rasterize an axis-aligned ellipse inscribed in
//! the bbox of two sprite-space corners, in outline or filled form.
//!
//! Endpoint order is irrelevant — the command normalizes to min / max
//! corners before rasterizing. `fill == false` walks the rim;
//! `fill == true` emits the full disk (rim + interior). Pixels outside
//! the target cel's pixel buffer are skipped silently per the natural
//! drawing-tool clipping semantics. Indexed and grayscale cels are
//! rejected (Phase 1 RGBA-first; see `docs/specs/pincel.md` §4.1 / §5.2).
//!
//! Rasterization uses the integer midpoint algorithm published by Alois
//! Zingl ("A Rasterizing Algorithm for Drawing Curves"). All internal
//! math is `i64` so any `i32` endpoint pair stays well-defined. To bound
//! iteration time (the perimeter walk is O(a + b)) and to keep
//! intermediate products within `i64`, ellipse bboxes larger than
//! `MAX_AXIS_SPAN` along either axis short-circuit to a no-op — those
//! are not realistic drags for pixel art, and a 2³² perimeter loop would
//! never terminate anyway. Degenerate `a == 0` / `b == 0` bboxes fall
//! back to a single-axis line (the Zingl algorithm assumes both axes
//! ≥ 1).

use std::collections::HashSet;

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, PixelBuffer, Rgba, Sprite};

use super::Command;
use super::error::CommandError;

/// Largest bbox dimension (max - min, in sprite pixels) along either
/// axis that this command will attempt to rasterize. Beyond this the
/// algorithm's products (`a*b*b`, etc.) approach `i64::MAX` and the
/// perimeter walk dominates the user's frame budget. 1M is several
/// orders of magnitude above any realistic pixel-art canvas.
const MAX_AXIS_SPAN: i64 = 1 << 20;

/// Write an axis-aligned ellipse inscribed in the bbox of sprite-space
/// `(x0, y0)` and `(x1, y1)`. Outline if `fill` is `false`, filled
/// otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawEllipse {
    layer: LayerId,
    frame: FrameIndex,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: bool,
    new_color: Rgba,
    /// `Some` after a successful `apply`; carries the prior pixel values
    /// used by `revert`. Stored in cel-local coordinates.
    previous: Option<Vec<PriorPixel>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PriorPixel {
    local_x: u32,
    local_y: u32,
    prior: Rgba,
}

impl DrawEllipse {
    /// Build a new `DrawEllipse` whose bbox spans the two sprite-space
    /// corners on the cel at `(layer, frame)`. Corners are passed as
    /// `(x, y)` tuples; endpoint order is irrelevant — the rasterizer
    /// normalizes to min / max before iterating.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        start: (i32, i32),
        end: (i32, i32),
        fill: bool,
        color: Rgba,
    ) -> Self {
        Self {
            layer,
            frame,
            x0: start.0,
            y0: start.1,
            x1: end.0,
            y1: end.1,
            fill,
            new_color: color,
            previous: None,
        }
    }
}

impl Command for DrawEllipse {
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

        let (min_x, min_y, max_x, max_y) = normalize(self.x0, self.y0, self.x1, self.y1);
        let cel_min_x = i64::from(cel.position.0);
        let cel_min_y = i64::from(cel.position.1);
        let cel_w = buffer.width;
        let cel_h = buffer.height;
        let cel_max_x = cel_min_x + i64::from(cel_w) - 1;
        let cel_max_y = cel_min_y + i64::from(cel_h) - 1;

        // Bbox entirely outside the cel — no pixels to write. Distinct
        // from the iteration-cap short-circuit below: the apply still
        // "succeeded" with zero prior pixels, so an undo is also a no-op.
        if i64::from(max_x) < cel_min_x
            || i64::from(min_x) > cel_max_x
            || i64::from(max_y) < cel_min_y
            || i64::from(min_y) > cel_max_y
        {
            self.previous = Some(Vec::new());
            return Ok(());
        }

        let a = i64::from(max_x) - i64::from(min_x);
        let b = i64::from(max_y) - i64::from(min_y);
        // Cap pathological drags so the perimeter walk terminates within
        // a frame budget and intermediate products stay in `i64`. The
        // ellipse is "essentially invisible" at the cel scale here;
        // dropping it is the closest analogue to "user dragged off the
        // canvas".
        if a > MAX_AXIS_SPAN || b > MAX_AXIS_SPAN {
            self.previous = Some(Vec::new());
            return Ok(());
        }

        let mut prior: Vec<PriorPixel> = Vec::new();
        // The midpoint algorithm plots the four quadrant corners every
        // iteration and the tail loop re-visits the axis tips, so the
        // same (lx, ly) often appears twice. Recording a "prior" on the
        // second visit would capture the just-written color and revert
        // would fail to restore the original pixel. `seen` deduplicates.
        let mut seen: HashSet<(u32, u32)> = HashSet::new();

        if self.fill {
            // Two passes: first collect the x extent of the rim for
            // each cel row the algorithm visits, then emit horizontal
            // spans inside those extents. Sized to the cel height so
            // out-of-cel rows are silently dropped without growing the
            // prior-pixel buffer.
            let mut row_min: Vec<i64> = vec![i64::MAX; cel_h as usize];
            let mut row_max: Vec<i64> = vec![i64::MIN; cel_h as usize];
            run_midpoint_ellipse(min_x, min_y, max_x, max_y, |sx, sy| {
                if sy < cel_min_y || sy > cel_max_y {
                    return;
                }
                let row = (sy - cel_min_y) as usize;
                if sx < row_min[row] {
                    row_min[row] = sx;
                }
                if sx > row_max[row] {
                    row_max[row] = sx;
                }
            });
            for row in 0..cel_h as usize {
                let lx = row_min[row];
                let rx = row_max[row];
                if lx > rx {
                    continue;
                }
                let lx_clip = lx.max(cel_min_x);
                let rx_clip = rx.min(cel_max_x);
                if lx_clip > rx_clip {
                    continue;
                }
                let sy = cel_min_y + row as i64;
                for sx in lx_clip..=rx_clip {
                    write_pixel(
                        buffer,
                        &mut prior,
                        &mut seen,
                        (cel_min_x, cel_min_y),
                        sx,
                        sy,
                        self.new_color,
                    );
                }
            }
        } else {
            run_midpoint_ellipse(min_x, min_y, max_x, max_y, |sx, sy| {
                if sx < cel_min_x || sx > cel_max_x || sy < cel_min_y || sy > cel_max_y {
                    return;
                }
                write_pixel(
                    buffer,
                    &mut prior,
                    &mut seen,
                    (cel_min_x, cel_min_y),
                    sx,
                    sy,
                    self.new_color,
                );
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

/// Sort the two endpoint corners so the returned tuple is
/// `(min_x, min_y, max_x, max_y)`. Endpoint order does not affect the
/// resulting ellipse.
fn normalize(x0: i32, y0: i32, x1: i32, y1: i32) -> (i32, i32, i32, i32) {
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}

fn write_pixel(
    buffer: &mut PixelBuffer,
    prior: &mut Vec<PriorPixel>,
    seen: &mut HashSet<(u32, u32)>,
    cel_origin: (i64, i64),
    sx: i64,
    sy: i64,
    color: Rgba,
) {
    let lx = (sx - cel_origin.0) as u32;
    let ly = (sy - cel_origin.1) as u32;
    if lx >= buffer.width || ly >= buffer.height {
        return;
    }
    if !seen.insert((lx, ly)) {
        return;
    }
    let offset = ((ly * buffer.width + lx) * 4) as usize;
    let before = Rgba {
        r: buffer.data[offset],
        g: buffer.data[offset + 1],
        b: buffer.data[offset + 2],
        a: buffer.data[offset + 3],
    };
    buffer.data[offset] = color.r;
    buffer.data[offset + 1] = color.g;
    buffer.data[offset + 2] = color.b;
    buffer.data[offset + 3] = color.a;
    prior.push(PriorPixel {
        local_x: lx,
        local_y: ly,
        prior: before,
    });
}

/// Rasterize the rim of the ellipse inscribed in the bbox
/// `[min_x, max_x] × [min_y, max_y]`. Calls `plot` with every (x, y)
/// pixel on the rim as `i64` sprite-space coordinates.
///
/// Degenerate bboxes (zero width and/or zero height) fall back to a
/// single-axis line — the published Zingl algorithm assumes both axes
/// ≥ 1 and otherwise leaves the tip pixels unplotted. Callers may pass
/// the bbox in any orientation; this helper requires it normalized.
fn run_midpoint_ellipse(
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    mut plot: impl FnMut(i64, i64),
) {
    let lo_x = i64::from(min_x);
    let hi_x = i64::from(max_x);
    let lo_y = i64::from(min_y);
    let hi_y = i64::from(max_y);
    let a = hi_x - lo_x;
    let b = hi_y - lo_y;

    if a == 0 && b == 0 {
        plot(lo_x, lo_y);
        return;
    }
    if a == 0 {
        for sy in lo_y..=hi_y {
            plot(lo_x, sy);
        }
        return;
    }
    if b == 0 {
        for sx in lo_x..=hi_x {
            plot(sx, lo_y);
        }
        return;
    }

    let b1 = b & 1;
    let mut x_lo = lo_x;
    let mut x_hi = hi_x;
    // Start at the rim's vertical extremes nearest the horizontal axis:
    // y_top sits just above the midline, y_bot just below. They expand
    // outward as the loop steps y. For odd `b` they coincide on the
    // exact midline (`b1 == 1`); for even `b` they straddle it.
    let mut y_top = lo_y + (b + 1) / 2;
    let mut y_bot = y_top - b1;

    let mut dx = 4 * (1 - a) * b * b;
    let mut dy = 4 * (b1 + 1) * a * a;
    let mut err = dx + dy + b1 * a * a;
    let step_y = 8 * a * a;
    let step_x = 8 * b * b;

    loop {
        plot(x_hi, y_top);
        plot(x_lo, y_top);
        plot(x_lo, y_bot);
        plot(x_hi, y_bot);
        let e2 = 2 * err;
        if e2 <= dy {
            y_top += 1;
            y_bot -= 1;
            dy += step_y;
            err += dy;
        }
        if e2 >= dx || 2 * err > dy {
            x_lo += 1;
            x_hi -= 1;
            dx += step_x;
            err += dx;
        }
        if x_lo > x_hi {
            break;
        }
    }

    // Flat ellipses (`a == 1`) leave the y tips unplotted because the
    // first loop exits while there are still rows between `y_top` and
    // the bbox edge. Walk the remaining rows one pixel wide. For
    // taller bboxes the predicate `y_top - y_bot < b` is already false
    // and this loop is a no-op.
    while y_top - y_bot < b {
        plot(x_lo - 1, y_top);
        plot(x_hi + 1, y_top);
        y_top += 1;
        plot(x_lo - 1, y_bot);
        plot(x_hi + 1, y_bot);
        y_bot -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, ColorMode, Frame, Layer, PixelBuffer};

    fn fixture(size: u32) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(size, size)
            .add_layer(Layer::image(LayerId::new(1), "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            PixelBuffer::empty(size, size, ColorMode::Rgba),
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

    fn is_set(cels: &CelMap, x: u32, y: u32) -> bool {
        pixel(cels, x, y) != Rgba::TRANSPARENT
    }

    #[test]
    fn normalize_orders_endpoints_minmax() {
        assert_eq!(normalize(1, 2, 4, 6), (1, 2, 4, 6));
        assert_eq!(normalize(4, 6, 1, 2), (1, 2, 4, 6));
        assert_eq!(normalize(-3, -1, 0, 5), (-3, -1, 0, 5));
    }

    #[test]
    fn single_pixel_bbox_writes_one_pixel() {
        let (mut sprite, mut cels) = fixture(8);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (3, 3),
            (3, 3),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(pixel(&cels, 3, 3), Rgba::WHITE);
        assert_eq!(pixel(&cels, 2, 3), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 4, 3), Rgba::TRANSPARENT);
    }

    #[test]
    fn zero_width_bbox_collapses_to_vertical_line() {
        let (mut sprite, mut cels) = fixture(8);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (4, 1),
            (4, 5),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 1..=5u32 {
            assert_eq!(pixel(&cels, 4, y), Rgba::WHITE);
        }
        // Nothing else is touched.
        assert_eq!(pixel(&cels, 3, 3), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 5, 3), Rgba::TRANSPARENT);
    }

    #[test]
    fn zero_height_bbox_collapses_to_horizontal_line() {
        let (mut sprite, mut cels) = fixture(8);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (1, 4),
            (6, 4),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for x in 1..=6u32 {
            assert_eq!(pixel(&cels, x, 4), Rgba::WHITE);
        }
        assert_eq!(pixel(&cels, 0, 4), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 7, 4), Rgba::TRANSPARENT);
    }

    #[test]
    fn outline_is_symmetric_about_the_center_of_the_bbox() {
        let (mut sprite, mut cels) = fixture(11);
        // Square bbox 0..=10 → an inscribed circle. The midpoint
        // algorithm is symmetric in both axes, so reflecting any rim
        // pixel about the center (5, 5) hits another rim pixel.
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (10, 10),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..11u32 {
            for x in 0..11u32 {
                let mirror_x = 10 - x;
                let mirror_y = 10 - y;
                assert_eq!(
                    is_set(&cels, x, y),
                    is_set(&cels, mirror_x, y),
                    "horizontal mirror at ({x}, {y})",
                );
                assert_eq!(
                    is_set(&cels, x, y),
                    is_set(&cels, x, mirror_y),
                    "vertical mirror at ({x}, {y})",
                );
            }
        }
    }

    #[test]
    fn outline_touches_the_four_axis_extremes() {
        let (mut sprite, mut cels) = fixture(11);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (10, 10),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // For an 11×11 bbox the rim must include the four axis-aligned
        // extremes — anything else and the user sees a broken rim.
        assert_eq!(pixel(&cels, 5, 0), Rgba::WHITE);
        assert_eq!(pixel(&cels, 5, 10), Rgba::WHITE);
        assert_eq!(pixel(&cels, 0, 5), Rgba::WHITE);
        assert_eq!(pixel(&cels, 10, 5), Rgba::WHITE);
        // The center is interior, not on the rim.
        assert_eq!(pixel(&cels, 5, 5), Rgba::TRANSPARENT);
    }

    #[test]
    fn fill_includes_the_center_and_the_rim() {
        let (mut sprite, mut cels) = fixture(11);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (10, 10),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // Center pixel filled.
        assert_eq!(pixel(&cels, 5, 5), Rgba::WHITE);
        // Rim extremes filled.
        assert_eq!(pixel(&cels, 5, 0), Rgba::WHITE);
        assert_eq!(pixel(&cels, 0, 5), Rgba::WHITE);
        // The four bbox corners lie outside an inscribed circle.
        assert_eq!(pixel(&cels, 0, 0), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 10, 10), Rgba::TRANSPARENT);
    }

    #[test]
    fn fill_rows_are_contiguous() {
        let (mut sprite, mut cels) = fixture(11);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (10, 10),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..11u32 {
            let mut filled = Vec::new();
            for x in 0..11u32 {
                if is_set(&cels, x, y) {
                    filled.push(x);
                }
            }
            if filled.is_empty() {
                continue;
            }
            let lo = *filled.first().unwrap();
            let hi = *filled.last().unwrap();
            for x in lo..=hi {
                assert!(is_set(&cels, x, y), "gap inside fill at ({x}, {y})");
            }
        }
    }

    #[test]
    fn reversed_endpoints_produce_the_same_ellipse() {
        let (mut sprite_a, mut cels_fwd) = fixture(11);
        let mut fwd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (1, 2),
            (9, 8),
            false,
            Rgba::WHITE,
        );
        fwd.apply(&mut sprite_a, &mut cels_fwd).expect("apply ok");

        let (mut sprite_b, mut cels_rev) = fixture(11);
        let mut rev = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (9, 8),
            (1, 2),
            false,
            Rgba::WHITE,
        );
        rev.apply(&mut sprite_b, &mut cels_rev).expect("apply ok");

        for y in 0..11u32 {
            for x in 0..11u32 {
                assert_eq!(pixel(&cels_fwd, x, y), pixel(&cels_rev, x, y));
            }
        }
    }

    #[test]
    fn revert_restores_outline_pixels() {
        let (mut sprite, mut cels) = fixture(11);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (10, 10),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        for y in 0..11u32 {
            for x in 0..11u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn revert_restores_fill_pixels() {
        let (mut sprite, mut cels) = fixture(11);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (10, 10),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        for y in 0..11u32 {
            for x in 0..11u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn outline_clipped_to_cel_bounds() {
        let (mut sprite, mut cels) = fixture(8);
        // Bbox extends one pixel past the 8×8 cel on each side, so
        // the rim's cardinal extremes lie outside the cel but the
        // diagonal-ish rim pixels still land inside. Out-of-cel rim
        // segments are silently dropped.
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (-1, -1),
            (8, 8),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        let mut any = false;
        for y in 0..8u32 {
            for x in 0..8u32 {
                if is_set(&cels, x, y) {
                    any = true;
                }
            }
        }
        assert!(any, "expected some rim pixels inside the cel");
    }

    #[test]
    fn bbox_entirely_outside_cel_is_noop() {
        let (mut sprite, mut cels) = fixture(8);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (100, 100),
            (110, 110),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn huge_bbox_short_circuits_without_drawing() {
        // i32::MIN/MAX corners would otherwise drive a perimeter walk
        // of ~2³² iterations and overflow `a*b*b` in the algorithm's
        // products. The command short-circuits to a no-op.
        let (mut sprite, mut cels) = fixture(8);
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn fill_with_offset_cel_uses_local_coords() {
        let (mut sprite, mut cels) = fixture(8);
        cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            .unwrap()
            .position = (2, 2);
        // Sprite-space bbox (3..=8) overlaps cel-local (1..=6). The
        // ellipse rim center sits at sprite (5.5, 5.5) → local (3.5,
        // 3.5).
        let mut cmd = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (3, 3),
            (8, 8),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // The center pixel of the cel-local mapping lands inside the
        // fill. (3, 3) in local coords corresponds to sprite (5, 5).
        assert_eq!(pixel(&cels, 3, 3), Rgba::WHITE);
        // Pixels outside the sprite-space bbox stay transparent.
        assert_eq!(pixel(&cels, 0, 0), Rgba::TRANSPARENT);
    }

    #[test]
    fn missing_cel_yields_error() {
        let (mut sprite, mut cels) = fixture(8);
        let mut cmd = DrawEllipse::new(
            LayerId::new(99),
            FrameIndex::new(0),
            (0, 0),
            (1, 1),
            true,
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
    fn does_not_merge_with_another_draw_ellipse() {
        let mut a = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (4, 4),
            false,
            Rgba::WHITE,
        );
        let b = DrawEllipse::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (5, 5),
            (9, 9),
            false,
            Rgba::WHITE,
        );
        assert!(!a.merge(&b));
    }
}
