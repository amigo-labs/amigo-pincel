//! `DrawDitherGradient` command — Pixel mode's Gradient tool
//! (`docs/specs/pincel.md` §5.2, `docs/specs/dithered-gradient.md`).
//!
//! A drag from `from` to `to` (sprite-space pixels, sampled at their
//! centres) gives every target pixel a ramp position `t` in `[0, 1]` — the
//! same linear / radial math Image mode's gradient uses
//! (`pincel_effects::gradient`). An ordered-dither threshold then picks
//! exactly the foreground or the background color per pixel, so the
//! gradient creates no new colors.
//!
//! The target is the active selection when there is one, else the whole
//! cel; either way only pixels inside both the cel and the canvas are
//! written, and the cel never grows. Indexed and grayscale cels are
//! rejected like [`FillRegion`](super::FillRegion) does.

use pincel_effects::gradient::GradientLine;

use crate::document::{CelData, CelMap, ColorMode, FrameIndex, LayerId, Rgba, Sprite};
use crate::geometry::Rect;

use super::Command;
use super::dirty::DirtyRegion;
use super::error::CommandError;

/// How the ramp runs away from the drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GradientKind {
    /// Along the drag: foreground up to the start, background past the end.
    #[default]
    Linear,
    /// Out from the start in every direction, background from the drag's
    /// length on.
    Radial,
}

impl GradientKind {
    /// Parse the wasm-boundary name (`linear`, `radial`).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "linear" => Some(Self::Linear),
            "radial" => Some(Self::Radial),
            _ => None,
        }
    }
}

/// The ordered-dither matrix that turns the ramp position into a color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DitherMatrix {
    /// No dithering: two hard bands split at `t = 0.5`.
    None,
    /// Bayer 2×2.
    Bayer2,
    /// Bayer 4×4.
    #[default]
    Bayer4,
    /// Bayer 8×8.
    Bayer8,
}

impl DitherMatrix {
    /// The matrix for a side length: `0` (or `1`) is [`DitherMatrix::None`],
    /// `2`, `4` and `8` the Bayer matrices; anything else is `None`.
    pub fn from_size(n: u32) -> Option<Self> {
        match n {
            0 | 1 => Some(Self::None),
            2 => Some(Self::Bayer2),
            4 => Some(Self::Bayer4),
            8 => Some(Self::Bayer8),
            _ => None,
        }
    }

    /// The matrix's side length (`1` for [`DitherMatrix::None`]).
    pub fn size(self) -> u32 {
        match self {
            Self::None => 1,
            Self::Bayer2 => 2,
            Self::Bayer4 => 4,
            Self::Bayer8 => 8,
        }
    }

    /// The threshold for sprite pixel `(x, y)`, in `(0, 1)`: a pixel whose
    /// ramp position is below it gets the foreground color.
    pub fn threshold(self, x: i32, y: i32) -> f32 {
        let n = self.size();
        if n == 1 {
            return 0.5;
        }
        let index = bayer_index(
            n,
            x.rem_euclid(n as i32) as u32,
            y.rem_euclid(n as i32) as u32,
        );
        (index as f32 + 0.5) / (n * n) as f32
    }
}

/// The Bayer matrix entry at `(x, y)` (both below `n`), `0..n²`: built
/// recursively from the 2×2 base `[[0, 2], [3, 1]]`.
fn bayer_index(n: u32, x: u32, y: u32) -> u32 {
    const BASE: [[u32; 2]; 2] = [[0, 2], [3, 1]];
    if n <= 1 {
        return 0;
    }
    let half = n / 2;
    4 * bayer_index(half, x % half, y % half) + BASE[(y / half) as usize][(x / half) as usize]
}

/// Fill the selection (or the whole cel) with a dithered foreground →
/// background ramp along a drag.
#[derive(Debug, Clone, PartialEq)]
pub struct DrawDitherGradient {
    layer: LayerId,
    frame: FrameIndex,
    from: (i32, i32),
    line: GradientLine,
    kind: GradientKind,
    matrix: DitherMatrix,
    foreground: Rgba,
    background: Rgba,
    /// `Some` after a successful `apply`: the cel-local rect that was
    /// written and its RGBA bytes from before, for `revert`.
    prior: Option<(Rect, Vec<u8>)>,
    /// Sprite-space bbox of the last apply, kept across revert.
    dirty_bbox: Option<Rect>,
}

impl DrawDitherGradient {
    /// A gradient on the cel at `(layer, frame)` dragged from sprite pixel
    /// `from` to sprite pixel `to`: `colors.0` (foreground) at the start,
    /// `colors.1` (background) at the end. `None` when `from == to` — a
    /// zero-length drag has no direction and changes nothing.
    pub fn new(
        layer: LayerId,
        frame: FrameIndex,
        from: (i32, i32),
        to: (i32, i32),
        kind: GradientKind,
        matrix: DitherMatrix,
        colors: (Rgba, Rgba),
    ) -> Option<Self> {
        let dx = to.0 as f32 - from.0 as f32;
        let dy = to.1 as f32 - from.1 as f32;
        Some(Self {
            layer,
            frame,
            from,
            line: GradientLine::new(dx, dy)?,
            kind,
            matrix,
            foreground: colors.0,
            background: colors.1,
            prior: None,
            dirty_bbox: None,
        })
    }

    /// The color sprite pixel `(x, y)` gets.
    fn color_at(&self, x: i32, y: i32) -> Rgba {
        // Both the start and the pixel sit at pixel centres, so the offset
        // is whole pixels.
        let px = (i64::from(x) - i64::from(self.from.0)) as f32;
        let py = (i64::from(y) - i64::from(self.from.1)) as f32;
        let t = match self.kind {
            GradientKind::Linear => self.line.linear(px, py),
            GradientKind::Radial => self.line.radial(px, py),
        };
        if t < self.matrix.threshold(x, y) {
            self.foreground
        } else {
            self.background
        }
    }
}

impl Command for DrawDitherGradient {
    fn apply(&mut self, doc: &mut Sprite, cels: &mut CelMap) -> Result<(), CommandError> {
        if !matches!(doc.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }
        let cel = cels
            .get_mut(self.layer, self.frame)
            .ok_or(CommandError::MissingCel {
                layer: self.layer,
                frame: self.frame,
            })?;
        let (cx, cy) = cel.position;
        let CelData::Image(buffer) = &mut cel.data else {
            return Err(CommandError::NotAnImageCel {
                layer: self.layer,
                frame: self.frame,
            });
        };
        if !matches!(buffer.color_mode, ColorMode::Rgba) {
            return Err(CommandError::UnsupportedColorMode);
        }

        // The cel's pixels that are on the canvas, cel-local, clipped
        // further to the selection's bounds when there is one.
        let mut x0 = (-i64::from(cx)).max(0);
        let mut y0 = (-i64::from(cy)).max(0);
        let mut x1 = i64::from(buffer.width).min(i64::from(doc.width) - i64::from(cx));
        let mut y1 = i64::from(buffer.height).min(i64::from(doc.height) - i64::from(cy));
        let selected = doc.has_selection();
        if let Some(sel) = doc.selection.filter(|_| selected) {
            x0 = x0.max(i64::from(sel.x) - i64::from(cx));
            y0 = y0.max(i64::from(sel.y) - i64::from(cy));
            x1 = x1.min(i64::from(sel.x) + i64::from(sel.width) - i64::from(cx));
            y1 = y1.min(i64::from(sel.y) + i64::from(sel.height) - i64::from(cy));
        }
        if x0 >= x1 || y0 >= y1 {
            self.prior = Some((Rect::new(0, 0, 0, 0), Vec::new()));
            self.dirty_bbox = None;
            return Ok(());
        }
        // In range: 0 ≤ x0 < x1 ≤ buffer.width, likewise for y.
        let (x0, y0, x1, y1) = (x0 as u32, y0 as u32, x1 as u32, y1 as u32);
        let rect = Rect::new(x0 as i32, y0 as i32, x1 - x0, y1 - y0);
        let width = buffer.width as usize;
        let mut before = Vec::with_capacity((rect.width * rect.height * 4) as usize);
        for ly in y0..y1 {
            let row = (ly as usize * width + x0 as usize) * 4;
            before.extend_from_slice(&buffer.data[row..row + (x1 - x0) as usize * 4]);
            for lx in x0..x1 {
                let sx = cx + lx as i32;
                let sy = cy + ly as i32;
                if selected && !doc.selection_contains(sx, sy) {
                    continue;
                }
                let c = self.color_at(sx, sy);
                let at = (ly as usize * width + lx as usize) * 4;
                buffer.data[at..at + 4].copy_from_slice(&[c.r, c.g, c.b, c.a]);
            }
        }
        self.dirty_bbox = Some(Rect::new(cx + rect.x, cy + rect.y, rect.width, rect.height));
        self.prior = Some((rect, before));
        Ok(())
    }

    fn revert(&mut self, _doc: &mut Sprite, cels: &mut CelMap) {
        let Some((rect, before)) = self.prior.take() else {
            return;
        };
        let Some(cel) = cels.get_mut(self.layer, self.frame) else {
            return;
        };
        let CelData::Image(buffer) = &mut cel.data else {
            return;
        };
        let row_len = rect.width as usize * 4;
        if row_len == 0 {
            return;
        }
        for (i, src) in before.chunks(row_len).enumerate() {
            let ly = rect.y as usize + i;
            let at = (ly * buffer.width as usize + rect.x as usize) * 4;
            if let Some(dst) = buffer.data.get_mut(at..at + src.len()) {
                dst.copy_from_slice(src);
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
    use crate::command::{AnyCommand, Bus};
    use crate::document::{Cel, Frame, Layer, PixelBuffer};
    use crate::selection::SelectionMask;

    const FG: Rgba = Rgba {
        r: 20,
        g: 30,
        b: 40,
        a: 255,
    };
    const BG: Rgba = Rgba {
        r: 200,
        g: 210,
        b: 220,
        a: 255,
    };
    const OTHER: Rgba = Rgba {
        r: 1,
        g: 99,
        b: 7,
        a: 128,
    };

    const L: LayerId = LayerId(1);
    const F: FrameIndex = FrameIndex(0);

    /// A `w × h` sprite whose one cel is `cel_w × cel_h` at `pos`, filled
    /// with `OTHER`.
    fn fixture_with_cel(
        w: u32,
        h: u32,
        pos: (i32, i32),
        cel_w: u32,
        cel_h: u32,
    ) -> (Sprite, CelMap) {
        let sprite = Sprite::builder(w, h)
            .add_layer(Layer::image(L, "bg"))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite builds");
        let mut buffer = PixelBuffer::empty(cel_w, cel_h, ColorMode::Rgba);
        for px in buffer.data.as_chunks_mut::<4>().0 {
            *px = [OTHER.r, OTHER.g, OTHER.b, OTHER.a];
        }
        let mut cel = Cel::image(L, F, buffer);
        cel.position = pos;
        let mut cels = CelMap::new();
        cels.insert(cel);
        (sprite, cels)
    }

    fn fixture(w: u32, h: u32) -> (Sprite, CelMap) {
        fixture_with_cel(w, h, (0, 0), w, h)
    }

    /// Sprite pixel `(x, y)` of the fixture's cel.
    fn px(sprite: &Sprite, cels: &CelMap, x: i32, y: i32) -> Rgba {
        let cel = cels.get(L, F).expect("cel");
        let CelData::Image(buf) = &cel.data else {
            panic!("image cel")
        };
        let _ = sprite;
        let lx = (x - cel.position.0) as usize;
        let ly = (y - cel.position.1) as usize;
        let at = (ly * buf.width as usize + lx) * 4;
        let d = &buf.data[at..at + 4];
        Rgba {
            r: d[0],
            g: d[1],
            b: d[2],
            a: d[3],
        }
    }

    fn cel_bytes(cels: &CelMap) -> Vec<u8> {
        let CelData::Image(buf) = &cels.get(L, F).expect("cel").data else {
            panic!("image cel")
        };
        buf.data.clone()
    }

    fn draw(
        sprite: &mut Sprite,
        cels: &mut CelMap,
        from: (i32, i32),
        to: (i32, i32),
        kind: GradientKind,
        matrix: DitherMatrix,
    ) {
        let mut cmd =
            DrawDitherGradient::new(L, F, from, to, kind, matrix, (FG, BG)).expect("non-zero drag");
        cmd.apply(sprite, cels).expect("apply");
    }

    const MATRICES: [DitherMatrix; 4] = [
        DitherMatrix::None,
        DitherMatrix::Bayer2,
        DitherMatrix::Bayer4,
        DitherMatrix::Bayer8,
    ];

    #[test]
    fn linear_before_start_is_foreground_and_past_end_is_background() {
        for matrix in MATRICES {
            let (mut s, mut c) = fixture(20, 8);
            draw(
                &mut s,
                &mut c,
                (5, 2),
                (13, 6),
                GradientKind::Linear,
                matrix,
            );
            let (dx, dy) = (8.0f32, 4.0f32);
            for y in 0..8 {
                for x in 0..20 {
                    let proj = ((x - 5) as f32 * dx + (y - 2) as f32 * dy) / (dx * dx + dy * dy);
                    if proj <= 0.0 {
                        assert_eq!(px(&s, &c, x, y), FG, "{matrix:?} ({x}, {y})");
                    }
                    if proj >= 1.0 {
                        assert_eq!(px(&s, &c, x, y), BG, "{matrix:?} ({x}, {y})");
                    }
                }
            }
        }
    }

    #[test]
    fn linear_sixteen_wide_row_starts_foreground_and_ends_background() {
        let (mut s, mut c) = fixture(16, 1);
        draw(
            &mut s,
            &mut c,
            (0, 0),
            (15, 0),
            GradientKind::Linear,
            DitherMatrix::Bayer4,
        );
        assert_eq!(px(&s, &c, 0, 0), FG);
        assert_eq!(px(&s, &c, 15, 0), BG);
    }

    #[test]
    fn radial_start_is_foreground_and_beyond_the_radius_is_background() {
        for matrix in MATRICES {
            let (mut s, mut c) = fixture(17, 17);
            draw(
                &mut s,
                &mut c,
                (8, 8),
                (11, 12),
                GradientKind::Radial,
                matrix,
            );
            assert_eq!(px(&s, &c, 8, 8), FG, "{matrix:?}");
            for y in 0..17 {
                for x in 0..17 {
                    let d2 = (x - 8) * (x - 8) + (y - 8) * (y - 8);
                    if d2 >= 25 {
                        assert_eq!(px(&s, &c, x, y), BG, "{matrix:?} ({x}, {y})");
                    }
                }
            }
        }
    }

    #[test]
    fn every_written_pixel_is_exactly_foreground_or_background() {
        for kind in [GradientKind::Linear, GradientKind::Radial] {
            for matrix in MATRICES {
                let (mut s, mut c) = fixture(13, 9);
                draw(&mut s, &mut c, (-3, 1), (11, 7), kind, matrix);
                for y in 0..9 {
                    for x in 0..13 {
                        let p = px(&s, &c, x, y);
                        assert!(p == FG || p == BG, "{kind:?} {matrix:?} ({x}, {y}) {p:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn equal_colors_paint_every_target_pixel_that_color() {
        let (mut s, mut c) = fixture(6, 6);
        let mut cmd = DrawDitherGradient::new(
            L,
            F,
            (0, 0),
            (5, 5),
            GradientKind::Linear,
            DitherMatrix::Bayer2,
            (FG, FG),
        )
        .unwrap();
        cmd.apply(&mut s, &mut c).unwrap();
        assert!(
            cel_bytes(&c)
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| *p == [20, 30, 40, 255])
        );
    }

    #[test]
    fn bayer_matrices_are_the_standard_ones() {
        let b2: Vec<u32> = (0..4).map(|i| bayer_index(2, i % 2, i / 2)).collect();
        assert_eq!(b2, [0, 2, 3, 1]);
        let b4: Vec<u32> = (0..16).map(|i| bayer_index(4, i % 4, i / 4)).collect();
        assert_eq!(b4, [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5]);
        let mut b8: Vec<u32> = (0..64).map(|i| bayer_index(8, i % 8, i / 8)).collect();
        assert_eq!(&b8[..8], [0, 32, 8, 40, 2, 34, 10, 42]);
        b8.sort_unstable();
        assert_eq!(b8, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn bayer_threshold_decides_every_pixel() {
        let b2 = [[0u32, 2], [3, 1]];
        let b4 = [
            [0u32, 8, 2, 10],
            [12, 4, 14, 6],
            [3, 11, 1, 9],
            [15, 7, 13, 5],
        ];
        for matrix in [
            DitherMatrix::Bayer2,
            DitherMatrix::Bayer4,
            DitherMatrix::Bayer8,
        ] {
            let n = matrix.size() as i32;
            let (mut s, mut c) = fixture(24, 9);
            draw(
                &mut s,
                &mut c,
                (2, 0),
                (21, 0),
                GradientKind::Linear,
                matrix,
            );
            for y in 0..9 {
                for x in 0..24 {
                    let t = ((x - 2) as f32 / 19.0).clamp(0.0, 1.0);
                    let (mx, my) = ((x % n) as usize, (y % n) as usize);
                    let index = match n {
                        2 => b2[my][mx],
                        4 => b4[my][mx],
                        _ => bayer_index(8, mx as u32, my as u32),
                    };
                    let m = (index as f32 + 0.5) / (n * n) as f32;
                    assert!(m > 0.0 && m < 1.0);
                    let want = if t < m { FG } else { BG };
                    assert_eq!(px(&s, &c, x, y), want, "{matrix:?} ({x}, {y}) t={t} m={m}");
                }
            }
        }
    }

    #[test]
    fn bayer_block_foreground_counts_never_increase_along_a_horizontal_ramp() {
        // docs/specs/dithered-gradient.md AC-05: per block of n columns
        // starting at a multiple of n, for ramps of several lengths, starts
        // and heights (a height not a multiple of n included).
        for matrix in [
            DitherMatrix::Bayer2,
            DitherMatrix::Bayer4,
            DitherMatrix::Bayer8,
        ] {
            let n = matrix.size() as i32;
            for (w, h, x0, x1) in [(32, 8, 0, 31), (40, 5, 3, 37), (64, 16, 8, 50)] {
                let (mut s, mut c) = fixture(w, h);
                draw(
                    &mut s,
                    &mut c,
                    (x0, 0),
                    (x1, 0),
                    GradientKind::Linear,
                    matrix,
                );
                let counts: Vec<usize> = (0..w as i32 / n)
                    .map(|b| {
                        (b * n..(b + 1) * n)
                            .flat_map(|x| (0..h as i32).map(move |y| (x, y)))
                            .filter(|&(x, y)| px(&s, &c, x, y) == FG)
                            .count()
                    })
                    .collect();
                assert!(
                    counts.windows(2).all(|p| p[1] <= p[0]),
                    "{matrix:?} {w}x{h} {x0}..{x1}: {counts:?}"
                );
                assert!(counts[0] > *counts.last().expect("at least one block"));
            }
        }
    }

    #[test]
    fn bayer8_on_a_small_sprite_uses_the_matching_part_of_the_matrix() {
        let (mut s, mut c) = fixture(4, 4);
        draw(
            &mut s,
            &mut c,
            (0, 0),
            (3, 0),
            GradientKind::Linear,
            DitherMatrix::Bayer8,
        );
        for y in 0..4 {
            for x in 0..4 {
                let t = x as f32 / 3.0;
                let m = (bayer_index(8, x as u32, y as u32) as f32 + 0.5) / 64.0;
                assert_eq!(px(&s, &c, x, y), if t < m { FG } else { BG });
            }
        }
    }

    #[test]
    fn none_matrix_splits_into_two_hard_bands_at_half() {
        let (mut s, mut c) = fixture(11, 3);
        draw(
            &mut s,
            &mut c,
            (0, 0),
            (10, 0),
            GradientKind::Linear,
            DitherMatrix::None,
        );
        for y in 0..3 {
            for x in 0..11 {
                let want = if (x as f32 / 10.0) < 0.5 { FG } else { BG };
                assert_eq!(px(&s, &c, x, y), want, "({x}, {y})");
            }
        }
    }

    #[test]
    fn gradient_respects_selection_mask() {
        // A shaped (ellipse) selection the drag line does not touch.
        let (mut s, mut c) = fixture(12, 12);
        s.set_selection_mask(SelectionMask::ellipse(12, 12, Rect::new(5, 5, 6, 6)));
        assert!(s.selection_mask.is_some());
        draw(
            &mut s,
            &mut c,
            (0, 0),
            (11, 0),
            GradientKind::Linear,
            DitherMatrix::Bayer4,
        );
        for y in 0..12 {
            for x in 0..12 {
                let p = px(&s, &c, x, y);
                if s.selection_contains(x, y) {
                    assert!(p == FG || p == BG, "inside ({x}, {y})");
                } else {
                    assert_eq!(p, OTHER, "outside ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn gradient_respects_selection_rect() {
        let (mut s, mut c) = fixture(10, 4);
        s.set_selection(Rect::new(3, 1, 4, 2));
        draw(
            &mut s,
            &mut c,
            (0, 0),
            (9, 0),
            GradientKind::Linear,
            DitherMatrix::None,
        );
        for y in 0..4 {
            for x in 0..10 {
                let inside = (3..7).contains(&x) && (1..3).contains(&y);
                let p = px(&s, &c, x, y);
                if inside {
                    assert_eq!(p, if (x as f32 / 9.0) < 0.5 { FG } else { BG });
                } else {
                    assert_eq!(p, OTHER, "outside ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn gradient_revert_restores_cel_and_redo_is_identical() {
        let (mut s, mut c) = fixture(9, 7);
        s.set_selection(Rect::new(1, 1, 6, 5));
        let before = cel_bytes(&c);
        let mut bus = Bus::new();
        let cmd = DrawDitherGradient::new(
            L,
            F,
            (1, 6),
            (7, 0),
            GradientKind::Radial,
            DitherMatrix::Bayer2,
            (FG, BG),
        )
        .unwrap();
        bus.execute(AnyCommand::from(cmd), &mut s, &mut c).unwrap();
        assert_eq!(bus.undo_depth(), 1);
        let after = cel_bytes(&c);
        assert_ne!(after, before);
        assert!(bus.undo(&mut s, &mut c));
        assert_eq!(cel_bytes(&c), before);
        assert!(bus.redo(&mut s, &mut c).unwrap());
        assert_eq!(cel_bytes(&c), after);
    }

    #[test]
    fn new_zero_length_drag_returns_none() {
        assert!(
            DrawDitherGradient::new(
                L,
                F,
                (3, 4),
                (3, 4),
                GradientKind::Linear,
                DitherMatrix::Bayer4,
                (FG, BG),
            )
            .is_none()
        );
    }

    #[test]
    fn indexed_or_grayscale_sprite_is_refused_without_an_undo_entry() {
        for mode in [
            ColorMode::Indexed {
                transparent_index: 0,
            },
            ColorMode::Grayscale,
        ] {
            let (mut s, mut c) = fixture(4, 4);
            s.color_mode = mode;
            let before = cel_bytes(&c);
            let mut bus = Bus::new();
            let cmd = DrawDitherGradient::new(
                L,
                F,
                (0, 0),
                (3, 0),
                GradientKind::Linear,
                DitherMatrix::Bayer4,
                (FG, BG),
            )
            .unwrap();
            let err = bus.execute(cmd.into(), &mut s, &mut c).unwrap_err();
            assert!(matches!(err, CommandError::UnsupportedColorMode), "{err:?}");
            assert_eq!(bus.undo_depth(), 0);
            assert_eq!(cel_bytes(&c), before);
        }
    }

    #[test]
    fn without_selection_every_cel_pixel_on_the_canvas_is_written_and_the_cel_keeps_its_size() {
        // A trimmed cel that sticks out past the canvas's right edge.
        let (mut s, mut c) = fixture_with_cel(8, 8, (5, 2), 6, 3);
        draw(
            &mut s,
            &mut c,
            (0, 0),
            (7, 0),
            GradientKind::Linear,
            DitherMatrix::None,
        );
        let cel = c.get(L, F).unwrap();
        assert_eq!(cel.position, (5, 2));
        let CelData::Image(buf) = &cel.data else {
            panic!()
        };
        assert_eq!((buf.width, buf.height), (6, 3));
        for ly in 0..3i32 {
            for lx in 0..6i32 {
                let (x, y) = (5 + lx, 2 + ly);
                let p = px(&s, &c, x, y);
                if x < 8 {
                    assert_eq!(
                        p,
                        if (x as f32 / 7.0) < 0.5 { FG } else { BG },
                        "({x}, {y})"
                    );
                } else {
                    assert_eq!(p, OTHER, "off-canvas ({x}, {y})");
                }
            }
        }
    }
}
