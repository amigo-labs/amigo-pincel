//! `DrawRectangle` command — rasterize an axis-aligned rectangle into an
//! image cel, in outline or filled form.
//!
//! Two sprite-space corners define the rectangle. Endpoint order is
//! irrelevant — the command normalizes to min / max corners before
//! rasterizing. `fill == false` writes the 1-pixel border; `fill == true`
//! writes every pixel in the interior (border included). Pixels outside
//! the target cel's pixel buffer are skipped silently per the natural
//! drawing-tool clipping semantics. Indexed and grayscale cels are
//! rejected (Phase 1 RGBA-first; see `docs/specs/pincel.md` §4.1 / §5.2).
//!
//! Iteration is clipped to the cel's pixel buffer up front so a user can
//! specify any `i32` endpoint pair (including arbitrarily large rects)
//! without allocating a prior-pixel record for every pixel in the bbox.

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};

use super::Command;
use super::error::CommandError;

/// Write an axis-aligned rectangle between sprite-space `(x0, y0)` and
/// `(x1, y1)`. Outline if `fill` is `false`, filled otherwise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawRectangle {
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

impl DrawRectangle {
    /// Build a new `DrawRectangle` between two sprite-space corners on the
    /// cel at `(layer, frame)`. Corners are passed as `(x, y)` tuples;
    /// endpoint order is irrelevant — the rasterizer normalizes to
    /// min / max before iterating.
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

impl Command for DrawRectangle {
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
        let cel_min_x = cel.position.0;
        let cel_min_y = cel.position.1;
        let cel_max_x = cel_min_x + buffer.width as i32 - 1;
        let cel_max_y = cel_min_y + buffer.height as i32 - 1;

        let mut prior: Vec<PriorPixel> = Vec::new();
        let mut write_pixel = |sx: i32, sy: i32, prior: &mut Vec<PriorPixel>| {
            if sx < cel_min_x || sx > cel_max_x || sy < cel_min_y || sy > cel_max_y {
                return;
            }
            let lx = (sx - cel_min_x) as u32;
            let ly = (sy - cel_min_y) as u32;
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
        };

        if self.fill {
            let lo_x = min_x.max(cel_min_x);
            let hi_x = max_x.min(cel_max_x);
            let lo_y = min_y.max(cel_min_y);
            let hi_y = max_y.min(cel_max_y);
            if lo_x <= hi_x && lo_y <= hi_y {
                for sy in lo_y..=hi_y {
                    for sx in lo_x..=hi_x {
                        write_pixel(sx, sy, &mut prior);
                    }
                }
            }
        } else {
            // Outline: walk the four edges, deduplicating corners by
            // restricting the side edges to the interior rows. Each
            // edge clips to the cel's bbox before iterating.
            for sx in min_x..=max_x {
                write_pixel(sx, min_y, &mut prior);
            }
            if max_y > min_y {
                for sx in min_x..=max_x {
                    write_pixel(sx, max_y, &mut prior);
                }
                if max_y > min_y + 1 {
                    for sy in (min_y + 1)..max_y {
                        write_pixel(min_x, sy, &mut prior);
                        if max_x > min_x {
                            write_pixel(max_x, sy, &mut prior);
                        }
                    }
                }
            }
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
/// resulting rectangle.
fn normalize(x0: i32, y0: i32, x1: i32, y1: i32) -> (i32, i32, i32, i32) {
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
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
    fn normalize_orders_endpoints_minmax() {
        assert_eq!(normalize(1, 2, 4, 6), (1, 2, 4, 6));
        assert_eq!(normalize(4, 6, 1, 2), (1, 2, 4, 6));
        assert_eq!(normalize(1, 6, 4, 2), (1, 2, 4, 6));
        assert_eq!(normalize(-3, -1, 0, 5), (-3, -1, 0, 5));
    }

    #[test]
    fn outline_writes_only_the_border() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (1, 1),
            (4, 4),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // Border pixels.
        for x in 1..=4 {
            assert_eq!(pixel(&cels, x, 1), Rgba::WHITE);
            assert_eq!(pixel(&cels, x, 4), Rgba::WHITE);
        }
        for y in 2..=3 {
            assert_eq!(pixel(&cels, 1, y), Rgba::WHITE);
            assert_eq!(pixel(&cels, 4, y), Rgba::WHITE);
        }
        // Interior stays transparent.
        for y in 2..=3 {
            for x in 2..=3 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
        // Outside the rect stays transparent.
        assert_eq!(pixel(&cels, 0, 0), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 5, 5), Rgba::TRANSPARENT);
    }

    #[test]
    fn fill_writes_every_pixel_in_the_bbox() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (1, 1),
            (3, 3),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 1..=3 {
            for x in 1..=3 {
                assert_eq!(pixel(&cels, x, y), Rgba::WHITE);
            }
        }
        assert_eq!(pixel(&cels, 0, 1), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 4, 3), Rgba::TRANSPARENT);
    }

    #[test]
    fn reversed_endpoints_produce_the_same_rectangle() {
        let (mut sprite, mut cels_fwd) = fixture();
        let mut fwd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (1, 2),
            (4, 5),
            false,
            Rgba::WHITE,
        );
        fwd.apply(&mut sprite, &mut cels_fwd).expect("apply ok");

        let (mut sprite2, mut cels_rev) = fixture();
        let mut rev = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (4, 5),
            (1, 2),
            false,
            Rgba::WHITE,
        );
        rev.apply(&mut sprite2, &mut cels_rev).expect("apply ok");

        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels_fwd, x, y), pixel(&cels_rev, x, y));
            }
        }
    }

    #[test]
    fn single_pixel_rectangle_outline_writes_one_pixel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (3, 3),
            (3, 3),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(pixel(&cels, 3, 3), Rgba::WHITE);
        // Neighbors stay transparent.
        assert_eq!(pixel(&cels, 2, 3), Rgba::TRANSPARENT);
        assert_eq!(pixel(&cels, 4, 3), Rgba::TRANSPARENT);
    }

    #[test]
    fn one_dimensional_outline_degenerates_to_a_line() {
        let (mut sprite, mut cels) = fixture();
        // Same `y` for both corners — outline should collapse to a single
        // horizontal run without writing the "bottom" edge twice.
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 2),
            (5, 2),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for x in 0..=5 {
            assert_eq!(pixel(&cels, x, 2), Rgba::WHITE);
        }
        // No other rows should be touched.
        for y in 0..8u32 {
            if y == 2 {
                continue;
            }
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn revert_restores_outline_pixels() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (7, 7),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn revert_restores_fill_pixels() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (1, 1),
            (6, 6),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn outline_clips_to_cel_bounds() {
        let (mut sprite, mut cels) = fixture();
        // Rect extends past the 8×8 cel on all sides. Only the in-bounds
        // border segments should be written.
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (-2, -2),
            (10, 10),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // The border lies entirely outside the cel, so nothing should be
        // written.
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn outline_partial_overlap_writes_only_in_bounds_edge_pixels() {
        let (mut sprite, mut cels) = fixture();
        // Rect bottom edge crosses the cel; top / left / right are
        // outside. Only the bottom row inside the cel should be touched.
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (-3, -3),
            (10, 4),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for x in 0..8u32 {
            assert_eq!(pixel(&cels, x, 4), Rgba::WHITE);
        }
        for y in 0..8u32 {
            if y == 4 {
                continue;
            }
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn fill_clips_to_cel_bounds() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (-2, -2),
            (3, 3),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..=3u32 {
            for x in 0..=3u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::WHITE);
            }
        }
        assert_eq!(pixel(&cels, 4, 4), Rgba::TRANSPARENT);
    }

    #[test]
    fn fill_with_offset_cel_uses_local_coords() {
        let (mut sprite, mut cels) = fixture();
        cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            .unwrap()
            .position = (2, 2);
        // Sprite-space (3..=4, 3..=4) → local (1..=2, 1..=2). Pixels at
        // sprite (1, 1) (= local (-1, -1)) are silently skipped.
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (1, 1),
            (4, 4),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for ly in 0..=2u32 {
            for lx in 0..=2u32 {
                assert_eq!(pixel(&cels, lx, ly), Rgba::WHITE);
            }
        }
        // Local (3, 3) is past sprite-space (4, 4) — outside the rect.
        assert_eq!(pixel(&cels, 3, 3), Rgba::TRANSPARENT);
    }

    #[test]
    fn missing_cel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
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
    fn does_not_merge_with_another_draw_rectangle() {
        let mut a = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (0, 0),
            (1, 1),
            false,
            Rgba::WHITE,
        );
        let b = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (2, 2),
            (3, 3),
            false,
            Rgba::WHITE,
        );
        assert!(!a.merge(&b));
    }

    #[test]
    fn outline_with_far_endpoints_does_not_allocate_per_unclipped_pixel() {
        // i32::MIN/MAX corners must clip to the cel before iterating —
        // otherwise the prior-pixel vec would grow to 2^32 entries and
        // exhaust memory.
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
            false,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // The outline corners are way outside the cel, so nothing lands.
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn fill_with_far_endpoints_clips_to_cel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = DrawRectangle::new(
            LayerId::new(1),
            FrameIndex::new(0),
            (i32::MIN, i32::MIN),
            (i32::MAX, i32::MAX),
            true,
            Rgba::WHITE,
        );
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // Every cel pixel was inside the rect, so every pixel is now opaque.
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::WHITE);
            }
        }
    }
}
