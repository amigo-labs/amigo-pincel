//! The Smudge tool (spec §5.4).
//!
//! The brush carries a sample of what it picked up and lays it down again
//! as it moves, so colors drag along the stroke — paint.net's Smudge, and
//! the reason a finger-painted edge softens instead of shifting.
//!
//! The sample lives for one stroke. Each dab mixes the carried color into
//! the canvas by the dab's coverage, then takes the result back up again
//! at `1 - strength`: a low strength releases what it holds quickly, a
//! high one drags a color a long way.

use crate::color::Color;
use crate::command::SetPixels;
use crate::document::Document;
use crate::geometry::Point;
use crate::tools::{Brush, PenSamples};

/// The Smudge tool.
#[derive(Debug, Clone, Copy)]
pub struct Smudge {
    /// The brush tip; its color is ignored — the canvas supplies the paint.
    pub brush: Brush,
    /// How much of the carried sample survives each dab, 0.0–1.0. Higher
    /// drags colors further.
    pub strength: f32,
}

impl Smudge {
    /// Creates a smudge brush at `strength` (clamped to 0.0–1.0).
    pub fn new(brush: Brush, strength: f32) -> Self {
        Self {
            brush,
            strength: strength.clamp(0.0, 1.0),
        }
    }

    /// Smudges along `points`.
    pub fn stroke(
        &self,
        layer_index: usize,
        points: &[Point],
        doc: &Document,
    ) -> Option<SetPixels> {
        self.stroke_with_pen(layer_index, points, PenSamples::default(), doc)
    }

    /// [`Smudge::stroke`] with pen pressure and azimuth per point (spec §17.4).
    pub fn stroke_with_pen(
        &self,
        layer_index: usize,
        points: &[Point],
        pen: PenSamples,
        doc: &Document,
    ) -> Option<SetPixels> {
        // The carried sample is indexed by the pixel's offset from the dab's
        // centre, so it stays aligned with the tip as the stroke moves. A
        // slot is filled on first contact, which is why the very first dab
        // only picks up and changes nothing.
        let reach = self.brush.radius().ceil() as i32 + 1;
        let span = (reach * 2 + 1) as usize;
        let mut carried: Vec<Option<Color>> = vec![None; span * span];
        let keep = self.strength;

        let (region, after) =
            self.brush
                .rasterize(layer_index, points, pen, doc, self.brush.opacity, |p| {
                    let (ix, iy) = (p.dx + reach, p.dy + reach);
                    if ix < 0 || iy < 0 || ix >= span as i32 || iy >= span as i32 {
                        return p.dst;
                    }
                    let slot = &mut carried[iy as usize * span + ix as usize];
                    let Some(sample) = *slot else {
                        // First contact: pick the color up, lay nothing down.
                        *slot = Some(p.dst);
                        return p.dst;
                    };
                    // Strength governs both halves: how much of the carried
                    // color is laid down, and how slowly the sample refreshes
                    // from the canvas. At 0 nothing is laid down and the
                    // sample is replaced every dab — a no-op. At 1 the sample
                    // never refreshes and drags its first color forever.
                    let out = mix(p.dst, sample, p.eff * keep);
                    // Pick up what was there *before* this dab, not what the
                    // dab just wrote: those are the same color at full
                    // coverage, and the sample would never change again.
                    *slot = Some(mix(sample, p.dst, 1.0 - keep));
                    out
                })?;
        Some(SetPixels::new(layer_index, region, after).with_label("Smudge"))
    }
}

/// Straight-alpha linear mix: `t = 0` is `a`, `t = 1` is `b`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| -> u8 {
        (f32::from(x) + (f32::from(y) - f32::from(x)) * t)
            .clamp(0.0, 255.0)
            .round() as u8
    };
    Color::rgba(ch(a.r, b.r), ch(a.g, b.g), ch(a.b, b.b), ch(a.a, b.a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::document::ImageBuffer;

    fn doc_with(w: u32, h: u32, paint: impl Fn(&mut ImageBuffer)) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        paint(&mut doc.layers[0].pixels);
        doc
    }

    /// A black square on the left half of a white strip.
    fn half_and_half(w: u32, h: u32) -> Document {
        doc_with(w, h, |b| {
            for y in 0..h {
                for x in 0..w {
                    b.set_pixel(
                        x,
                        y,
                        if x < w / 2 {
                            Color::BLACK
                        } else {
                            Color::WHITE
                        },
                    );
                }
            }
        })
    }

    fn smudge(strength: f32) -> Smudge {
        Smudge::new(Brush::new(6, Color::BLACK, 1.0), strength)
    }

    #[test]
    fn smudge_drags_color_across_an_edge() {
        let mut doc = half_and_half(16, 8);
        let before = doc.layers[0].pixels.get_pixel(12, 4).unwrap();
        assert_eq!(before, Color::WHITE);
        let points: Vec<Point> = (2..14).map(|x| Point::new(x as f32, 4.0)).collect();
        let mut cmd = smudge(0.8).stroke(0, &points, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        // Black has been carried out into what was white.
        let after = doc.layers[0].pixels.get_pixel(12, 4).unwrap();
        assert!(after.r < 255, "nothing was dragged: {after:?}");
    }

    #[test]
    fn strength_zero_changes_nothing() {
        let mut doc = half_and_half(16, 8);
        let before = doc.layers[0].pixels.data().to_vec();
        let points: Vec<Point> = (2..14).map(|x| Point::new(x as f32, 4.0)).collect();
        let mut cmd = smudge(0.0).stroke(0, &points, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.data(), before.as_slice());
    }

    #[test]
    fn smudge_leaves_a_flat_color_alone() {
        let mut doc = doc_with(12, 6, |b| {
            for y in 0..6 {
                for x in 0..12 {
                    b.set_pixel(x, y, Color::rgba(70, 90, 110, 255));
                }
            }
        });
        let before = doc.layers[0].pixels.data().to_vec();
        let points: Vec<Point> = (2..10).map(|x| Point::new(x as f32, 3.0)).collect();
        let mut cmd = smudge(0.7).stroke(0, &points, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.data(), before.as_slice());
    }

    #[test]
    fn a_single_dab_only_picks_up() {
        let mut doc = half_and_half(16, 8);
        let before = doc.layers[0].pixels.data().to_vec();
        let mut cmd = smudge(0.8)
            .stroke(0, &[Point::new(8.0, 4.0)], &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.data(), before.as_slice());
    }

    #[test]
    fn higher_strength_drags_further() {
        let points: Vec<Point> = (2..14).map(|x| Point::new(x as f32, 4.0)).collect();
        let run = |s: f32| {
            let mut doc = half_and_half(16, 8);
            let mut cmd = smudge(s).stroke(0, &points, &doc).unwrap();
            cmd.apply(&mut doc).unwrap();
            doc.layers[0].pixels.get_pixel(12, 4).unwrap().r
        };
        assert!(run(0.9) < run(0.2), "0.9 should carry further than 0.2");
    }

    #[test]
    fn smudge_is_undoable() {
        let mut doc = half_and_half(16, 8);
        let before = doc.layers[0].pixels.data().to_vec();
        let points: Vec<Point> = (2..14).map(|x| Point::new(x as f32, 4.0)).collect();
        let mut cmd = smudge(0.8).stroke(0, &points, &doc).unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.data(), before.as_slice());
    }

    #[test]
    fn smudge_on_an_invalid_layer_returns_none() {
        let doc = half_and_half(16, 8);
        assert!(
            smudge(0.8)
                .stroke(9, &[Point::new(4.0, 4.0), Point::new(8.0, 4.0)], &doc)
                .is_none()
        );
    }
}
