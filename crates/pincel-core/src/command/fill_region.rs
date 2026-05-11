//! `FillRegion` command — bucket-fill the contiguous region of pixels that
//! share a color with the seed pixel.
//!
//! Sprite-space seed coordinates are translated into cel-local space via the
//! cel's `position`. The fill is 4-connected (up / down / left / right
//! neighbors) and the tolerance is zero — only pixels whose RGBA matches
//! the seed exactly are replaced. Painting the seed color over itself
//! short-circuits to a no-op (records no modified pixels, so `revert`
//! does nothing). A seed that falls outside the cel buffer is also a
//! no-op. Indexed and grayscale cels are rejected (Phase 1 RGBA-first;
//! see `docs/specs/pincel.md` §4.1 / §5.2).
//!
//! Traversal uses a queue-based BFS with a visited bitmap so the worst-
//! case work is `O(width * height)` and each pixel is touched at most
//! once.

use std::collections::VecDeque;

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};

use super::Command;
use super::error::CommandError;

/// Flood-fill the 4-connected region of pixels matching the seed-pixel
/// color, replacing them with `new_color`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillRegion {
    layer: LayerId,
    frame: FrameIndex,
    sprite_x: i32,
    sprite_y: i32,
    new_color: Rgba,
    /// `Some` after a successful `apply`; carries the prior pixel value
    /// (shared across all filled pixels) and the cel-local coordinates of
    /// every pixel that was modified, used by `revert`.
    previous: Option<FilledRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FilledRegion {
    prior: Rgba,
    pixels: Vec<(u32, u32)>,
}

impl FillRegion {
    /// Build a new `FillRegion` seeded at sprite-space `(sprite_x, sprite_y)`
    /// on the cel at `(layer, frame)`.
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
            sprite_x,
            sprite_y,
            new_color: color,
            previous: None,
        }
    }

    /// Number of pixels written by the most recent successful `apply`.
    /// Returns `0` before `apply` runs or after `revert`.
    pub fn filled_count(&self) -> usize {
        self.previous.as_ref().map(|r| r.pixels.len()).unwrap_or(0)
    }
}

impl Command for FillRegion {
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

        let Some((seed_x, seed_y)) = local_coords(self.sprite_x, self.sprite_y, cel.position)
        else {
            self.previous = Some(FilledRegion {
                prior: Rgba::TRANSPARENT,
                pixels: Vec::new(),
            });
            return Ok(());
        };
        if seed_x >= buffer.width || seed_y >= buffer.height {
            self.previous = Some(FilledRegion {
                prior: Rgba::TRANSPARENT,
                pixels: Vec::new(),
            });
            return Ok(());
        }

        let width = buffer.width;
        let height = buffer.height;
        let seed_offset = ((seed_y * width + seed_x) * 4) as usize;
        let seed_color = Rgba {
            r: buffer.data[seed_offset],
            g: buffer.data[seed_offset + 1],
            b: buffer.data[seed_offset + 2],
            a: buffer.data[seed_offset + 3],
        };

        // Painting the seed color over itself would do O(region) work
        // for no visible change (the visited bitmap still bounds the
        // traversal, but every "write" would replace the pixel with its
        // own value). Short-circuit to a no-op.
        if seed_color == self.new_color {
            self.previous = Some(FilledRegion {
                prior: seed_color,
                pixels: Vec::new(),
            });
            return Ok(());
        }

        let total = (width as usize) * (height as usize);
        let mut visited = vec![false; total];
        let mut pixels = Vec::new();
        let mut queue: VecDeque<(u32, u32)> = VecDeque::new();

        let seed_idx = (seed_y as usize) * (width as usize) + (seed_x as usize);
        visited[seed_idx] = true;
        queue.push_back((seed_x, seed_y));

        while let Some((x, y)) = queue.pop_front() {
            let offset = ((y * width + x) * 4) as usize;
            let here = Rgba {
                r: buffer.data[offset],
                g: buffer.data[offset + 1],
                b: buffer.data[offset + 2],
                a: buffer.data[offset + 3],
            };
            if here != seed_color {
                continue;
            }
            buffer.data[offset] = self.new_color.r;
            buffer.data[offset + 1] = self.new_color.g;
            buffer.data[offset + 2] = self.new_color.b;
            buffer.data[offset + 3] = self.new_color.a;
            pixels.push((x, y));

            // 4-connected neighbors. Manually unrolled to skip the
            // `i32`-arith costs of a Δ array.
            if x > 0 {
                let n_idx = (y as usize) * (width as usize) + (x as usize - 1);
                if !visited[n_idx] {
                    visited[n_idx] = true;
                    queue.push_back((x - 1, y));
                }
            }
            if x + 1 < width {
                let n_idx = (y as usize) * (width as usize) + (x as usize + 1);
                if !visited[n_idx] {
                    visited[n_idx] = true;
                    queue.push_back((x + 1, y));
                }
            }
            if y > 0 {
                let n_idx = (y as usize - 1) * (width as usize) + (x as usize);
                if !visited[n_idx] {
                    visited[n_idx] = true;
                    queue.push_back((x, y - 1));
                }
            }
            if y + 1 < height {
                let n_idx = (y as usize + 1) * (width as usize) + (x as usize);
                if !visited[n_idx] {
                    visited[n_idx] = true;
                    queue.push_back((x, y + 1));
                }
            }
        }

        self.previous = Some(FilledRegion {
            prior: seed_color,
            pixels,
        });
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some(region) = self.previous.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        for (lx, ly) in region.pixels {
            if lx >= buffer.width || ly >= buffer.height {
                continue;
            }
            let offset = ((ly * buffer.width + lx) * 4) as usize;
            buffer.data[offset] = region.prior.r;
            buffer.data[offset + 1] = region.prior.g;
            buffer.data[offset + 2] = region.prior.b;
            buffer.data[offset + 3] = region.prior.a;
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
        let off = ((y * buf.width + x) * 4) as usize;
        Rgba {
            r: buf.data[off],
            g: buf.data[off + 1],
            b: buf.data[off + 2],
            a: buf.data[off + 3],
        }
    }

    fn write_pixel(cels: &mut CelMap, x: u32, y: u32, color: Rgba) {
        let cel = cels
            .get_mut(LayerId::new(1), FrameIndex::new(0))
            .expect("cel exists");
        let CelData::Image(buf) = &mut cel.data else {
            panic!("expected image cel");
        };
        let off = ((y * buf.width + x) * 4) as usize;
        buf.data[off] = color.r;
        buf.data[off + 1] = color.g;
        buf.data[off + 2] = color.b;
        buf.data[off + 3] = color.a;
    }

    #[test]
    fn fill_on_blank_cel_paints_every_pixel() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 3, 3, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), RED);
            }
        }
        assert_eq!(cmd.filled_count(), 64);
    }

    #[test]
    fn fill_stops_at_a_color_boundary() {
        // Paint a vertical line down column 4; flood-filling from (0, 0)
        // should only touch x = 0..=3, leaving the line and the right half
        // untouched.
        let (mut sprite, mut cels) = fixture();
        for y in 0..8u32 {
            write_pixel(&mut cels, 4, y, BLUE);
        }
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 0, 0, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..8u32 {
            for x in 0..=3u32 {
                assert_eq!(pixel(&cels, x, y), RED);
            }
            assert_eq!(pixel(&cels, 4, y), BLUE);
            for x in 5..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
        assert_eq!(cmd.filled_count(), 32);
    }

    #[test]
    fn fill_is_four_connected_not_eight() {
        // A diagonal blue line should *not* leak through. The fill seeded
        // from (0, 0) covers the top-left triangle only; the bottom-right
        // triangle stays transparent.
        let (mut sprite, mut cels) = fixture();
        for i in 0..8u32 {
            write_pixel(&mut cels, i, i, BLUE);
        }
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 0, 1, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // Top-left triangle (above diagonal) is red.
        for y in 1..8u32 {
            for x in 0..y {
                assert_eq!(pixel(&cels, x, y), RED);
            }
        }
        // The diagonal stays blue.
        for i in 0..8u32 {
            assert_eq!(pixel(&cels, i, i), BLUE);
        }
        // Bottom-right triangle (below diagonal) is untouched.
        for y in 0..7u32 {
            for x in (y + 1)..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn fill_with_same_color_is_a_noop() {
        let (mut sprite, mut cels) = fixture();
        // Pre-fill with RED, then fill again with RED — no pixels should
        // be recorded as modified.
        for y in 0..8u32 {
            for x in 0..8u32 {
                write_pixel(&mut cels, x, y, RED);
            }
        }
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 0, 0, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(cmd.filled_count(), 0);
        // Revert is also a no-op; pixels stay red.
        cmd.revert(&mut sprite, &mut cels);
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), RED);
            }
        }
    }

    #[test]
    fn revert_restores_every_filled_pixel() {
        let (mut sprite, mut cels) = fixture();
        // Pre-paint a single pixel BLUE; everything else is transparent.
        write_pixel(&mut cels, 3, 3, BLUE);
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 0, 0, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        cmd.revert(&mut sprite, &mut cels);
        // Back to the pre-apply state: BLUE at (3, 3), transparent elsewhere.
        for y in 0..8u32 {
            for x in 0..8u32 {
                let expected = if (x, y) == (3, 3) {
                    BLUE
                } else {
                    Rgba::TRANSPARENT
                };
                assert_eq!(pixel(&cels, x, y), expected);
            }
        }
    }

    #[test]
    fn seed_outside_cel_is_a_noop() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 100, 100, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
        assert_eq!(cmd.filled_count(), 0);
    }

    #[test]
    fn seed_with_negative_local_coord_is_a_noop() {
        let (mut sprite, mut cels) = fixture();
        // Offset the cel so sprite (0, 0) → local (-2, -2).
        cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            .unwrap()
            .position = (2, 2);
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 0, 0, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        for y in 0..8u32 {
            for x in 0..8u32 {
                assert_eq!(pixel(&cels, x, y), Rgba::TRANSPARENT);
            }
        }
    }

    #[test]
    fn seed_uses_cel_local_coords_for_offset_cels() {
        let (mut sprite, mut cels) = fixture();
        cels.get_mut(LayerId::new(1), FrameIndex::new(0))
            .unwrap()
            .position = (4, 4);
        // Sprite (5, 5) → local (1, 1); fills the whole 8×8 buffer.
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 5, 5, RED);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        assert_eq!(cmd.filled_count(), 64);
    }

    #[test]
    fn missing_cel_yields_error() {
        let (mut sprite, mut cels) = fixture();
        let mut cmd = FillRegion::new(LayerId::new(99), FrameIndex::new(0), 0, 0, RED);
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
    fn fill_into_enclosed_region_does_not_leak() {
        // Build a 6×6 RED frame inside an 8×8 transparent cel, then fill
        // from inside (4, 4): only the interior 4×4 region should turn
        // BLUE, the RED frame stays RED, and the outside transparent rim
        // is untouched.
        let (mut sprite, mut cels) = fixture();
        for x in 1..=6u32 {
            write_pixel(&mut cels, x, 1, RED);
            write_pixel(&mut cels, x, 6, RED);
        }
        for y in 2..=5u32 {
            write_pixel(&mut cels, 1, y, RED);
            write_pixel(&mut cels, 6, y, RED);
        }
        let mut cmd = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 4, 4, BLUE);
        cmd.apply(&mut sprite, &mut cels).expect("apply ok");
        // Outer rim: transparent.
        for x in 0..8u32 {
            assert_eq!(pixel(&cels, x, 0), Rgba::TRANSPARENT);
            assert_eq!(pixel(&cels, x, 7), Rgba::TRANSPARENT);
        }
        for y in 0..8u32 {
            assert_eq!(pixel(&cels, 0, y), Rgba::TRANSPARENT);
            assert_eq!(pixel(&cels, 7, y), Rgba::TRANSPARENT);
        }
        // RED frame: still RED.
        for x in 1..=6u32 {
            assert_eq!(pixel(&cels, x, 1), RED);
            assert_eq!(pixel(&cels, x, 6), RED);
        }
        for y in 2..=5u32 {
            assert_eq!(pixel(&cels, 1, y), RED);
            assert_eq!(pixel(&cels, 6, y), RED);
        }
        // Interior 4×4: filled BLUE.
        for y in 2..=5u32 {
            for x in 2..=5u32 {
                assert_eq!(pixel(&cels, x, y), BLUE);
            }
        }
        assert_eq!(cmd.filled_count(), 16);
    }

    #[test]
    fn does_not_merge_with_another_fill_region() {
        let mut a = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 0, 0, RED);
        let b = FillRegion::new(LayerId::new(1), FrameIndex::new(0), 1, 1, BLUE);
        assert!(!a.merge(&b));
    }
}
