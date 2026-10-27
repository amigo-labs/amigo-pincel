//! The Recolor tool (spec §5.4).
//!
//! A brush that only writes where it finds the color it is replacing, so a
//! stroke across a sprite's outline can repaint the outline without
//! touching what it encloses. paint.net's Recolor, with the tolerance the
//! bucket already has.

use std::collections::HashMap;

use crate::color::Color;
use crate::command::SetPixels;
use crate::document::Document;
use crate::geometry::Point;
use crate::tools::{Brush, PenSamples, src_over};

/// What each pixel looked like before the drag touched it.
///
/// Eligibility has to be judged against that, not against the pixel as the
/// stroke has left it: a soft dab edge shifts a pixel part of the way, and
/// at zero tolerance it would then no longer match `from` when the tip's
/// centre arrives. One drag arrives as many commands (merged into one undo
/// step by `stroke_id`), so the caller keeps this across them.
#[derive(Debug, Default, Clone)]
pub struct RecolorOrigins {
    seen: HashMap<(u32, u32), Color>,
}

impl RecolorOrigins {
    /// Forget everything; a fresh drag starts over.
    pub fn clear(&mut self) {
        self.seen.clear();
    }

    /// The colour this pixel had on first contact, recording `now` if this
    /// is the first time the drag has reached it.
    fn origin(&mut self, x: u32, y: u32, now: Color) -> Color {
        *self.seen.entry((x, y)).or_insert(now)
    }
}

/// The Recolor tool — paints `to` only over pixels close to `from`.
#[derive(Debug, Clone, Copy)]
pub struct Recolor {
    /// The brush tip; its color is ignored in favour of `to`.
    pub brush: Brush,
    /// The color being replaced (the background color in the UI).
    pub from: Color,
    /// The color painted in its place (the foreground color).
    pub to: Color,
    /// How far a pixel may differ from `from` and still count, 0–255.
    pub tolerance: u8,
}

impl Recolor {
    /// Creates a recolor brush replacing `from` with `to`.
    pub fn new(brush: Brush, from: Color, to: Color, tolerance: u8) -> Self {
        Self {
            brush,
            from,
            to,
            tolerance,
        }
    }

    /// Recolors along `points`, as [`Pencil::stroke`](super::Pencil::stroke).
    ///
    /// For a stroke delivered in one piece; a drag split across several
    /// calls passes its own [`RecolorOrigins`] to [`Recolor::stroke_with_pen`].
    pub fn stroke(
        &self,
        layer_index: usize,
        points: &[Point],
        doc: &Document,
    ) -> Option<SetPixels> {
        self.stroke_with_pen(
            layer_index,
            points,
            PenSamples::default(),
            doc,
            &mut RecolorOrigins::default(),
        )
    }

    /// [`Recolor::stroke`] with pen input, and the drag's own record of what
    /// each pixel looked like before it (spec §17.4).
    pub fn stroke_with_pen(
        &self,
        layer_index: usize,
        points: &[Point],
        pen: PenSamples,
        doc: &Document,
        origins: &mut RecolorOrigins,
    ) -> Option<SetPixels> {
        let to = self.to;
        let from = self.from;
        let tol = self.tolerance;
        // The replacement's own alpha scales the stroke, as the pencil's does.
        let strength = self.brush.opacity * (f32::from(to.a) / 255.0);
        let (region, after) =
            self.brush
                .rasterize(layer_index, points, pen, doc, strength, |p| {
                    if origins.origin(p.x, p.y, p.dst).within_tolerance(from, tol) {
                        src_over(to, p.eff, p.dst)
                    } else {
                        p.dst
                    }
                })?;
        Some(SetPixels::new(layer_index, region, after).with_label("Recolor"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::document::ImageBuffer;
    use crate::tools::BrushShape;

    // A size-1 tip covers less than a whole pixel, so the tests use a
    // 4 px brush and assert "changed / unchanged" rather than exact values:
    // the dab's edge coverage is the brush's business, not recolor's.
    fn doc_with(w: u32, h: u32, paint: impl Fn(&mut ImageBuffer)) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        paint(&mut doc.layers[0].pixels);
        doc
    }

    fn across(w: u32) -> Vec<Point> {
        (0..w).map(|x| Point::new(x as f32, 0.0)).collect()
    }

    fn tool(from: Color, to: Color, tolerance: u8) -> Recolor {
        Recolor::new(Brush::new(4, to, 1.0), from, to, tolerance)
    }

    #[test]
    fn recolor_replaces_only_the_matching_pixels() {
        let mut doc = doc_with(3, 1, |b| {
            b.set_pixel(0, 0, Color::WHITE);
            b.set_pixel(1, 0, Color::BLACK);
            b.set_pixel(2, 0, Color::WHITE);
        });
        let mut cmd = tool(Color::WHITE, Color::rgba(255, 0, 0, 255), 0)
            .stroke(0, &across(3), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        // The white pixels moved toward red …
        let left = doc.layers[0].pixels.get_pixel(0, 0).unwrap();
        assert!(left.g < 255, "left stayed white: {left:?}");
        // … the black one, which never matched, is untouched.
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(Color::BLACK));
    }

    #[test]
    fn recolor_leaves_everything_alone_without_a_match() {
        let mut doc = doc_with(2, 1, |b| {
            b.set_pixel(0, 0, Color::rgba(10, 10, 10, 255));
            b.set_pixel(1, 0, Color::rgba(10, 10, 10, 255));
        });
        let before = doc.layers[0].pixels.data().to_vec();
        let mut cmd = tool(Color::WHITE, Color::BLACK, 0)
            .stroke(0, &across(2), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.data(), before.as_slice());
    }

    #[test]
    fn tolerance_widens_what_counts_as_a_match() {
        let near_white = Color::rgba(250, 250, 250, 255);
        let strict = {
            let mut doc = doc_with(3, 1, |b| b.set_pixel(1, 0, near_white));
            let mut cmd = tool(Color::WHITE, Color::BLACK, 0)
                .stroke(0, &across(3), &doc)
                .unwrap();
            cmd.apply(&mut doc).unwrap();
            doc.layers[0].pixels.get_pixel(1, 0).unwrap()
        };
        let loose = {
            let mut doc = doc_with(3, 1, |b| b.set_pixel(1, 0, near_white));
            let mut cmd = tool(Color::WHITE, Color::BLACK, 16)
                .stroke(0, &across(3), &doc)
                .unwrap();
            cmd.apply(&mut doc).unwrap();
            doc.layers[0].pixels.get_pixel(1, 0).unwrap()
        };
        assert_eq!(strict, near_white, "0 tolerance should not match");
        assert!(
            loose.r < near_white.r,
            "16 tolerance should match: {loose:?}"
        );
    }

    /// A soft tip at zero tolerance: the dab edge shifts a pixel part of the
    /// way, and judging eligibility from the stroke buffer would then rule it
    /// out for the dabs that follow.
    fn soft_tool(from: Color, to: Color) -> Recolor {
        Recolor::new(
            Brush::new(6, to, 1.0)
                .with_shape(BrushShape::SoftRound)
                .with_hardness(0.3),
            from,
            to,
            0,
        )
    }

    fn white_strip(w: u32, h: u32) -> Document {
        doc_with(w, h, |b| {
            for y in 0..h {
                for x in 0..w {
                    b.set_pixel(x, y, Color::WHITE);
                }
            }
        })
    }

    fn run_split(tool: &Recolor, origins: Option<&mut RecolorOrigins>) -> Vec<u8> {
        let pts: Vec<Point> = (2..14).map(|x| Point::new(x as f32, 4.0)).collect();
        let mut doc = white_strip(16, 8);
        let mut own = RecolorOrigins::default();
        let shared = origins.is_some();
        let origins = origins.unwrap_or(&mut own);
        for part in [&pts[..7], &pts[6..]] {
            if !shared {
                origins.clear();
            }
            let mut cmd = tool
                .stroke_with_pen(0, part, PenSamples::default(), &doc, origins)
                .unwrap();
            cmd.apply(&mut doc).unwrap();
        }
        doc.layers[0].pixels.data().to_vec()
    }

    #[test]
    fn a_split_stroke_matches_the_unsplit_one() {
        let tool = soft_tool(Color::WHITE, Color::BLACK);
        let pts: Vec<Point> = (2..14).map(|x| Point::new(x as f32, 4.0)).collect();
        let whole = {
            let mut doc = white_strip(16, 8);
            let mut cmd = tool.stroke(0, &pts, &doc).unwrap();
            cmd.apply(&mut doc).unwrap();
            doc.layers[0].pixels.data().to_vec()
        };
        let mut origins = RecolorOrigins::default();
        assert_eq!(whole, run_split(&tool, Some(&mut origins)));
    }

    #[test]
    fn a_split_stroke_that_forgets_its_origins_differs() {
        // The counter-test: judging the second half against the half-recolored
        // pixels leaves them stranded at zero tolerance.
        let tool = soft_tool(Color::WHITE, Color::BLACK);
        let mut origins = RecolorOrigins::default();
        assert_ne!(run_split(&tool, Some(&mut origins)), run_split(&tool, None));
    }

    #[test]
    fn recolor_is_undoable() {
        let mut doc = doc_with(3, 1, |b| b.set_pixel(1, 0, Color::WHITE));
        let mut cmd = tool(Color::WHITE, Color::BLACK, 0)
            .stroke(0, &across(3), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(doc.layers[0].pixels.get_pixel(1, 0), Some(Color::WHITE));
    }

    #[test]
    fn recolor_on_an_invalid_layer_returns_none() {
        let doc = doc_with(3, 1, |b| b.set_pixel(1, 0, Color::WHITE));
        assert!(
            tool(Color::WHITE, Color::BLACK, 0)
                .stroke(9, &across(3), &doc)
                .is_none()
        );
    }
}
