//! The Gradient tool (spec §5.4).
//!
//! A drag from `a` to `b` defines the ramp; every pixel takes a parameter
//! `t` in `[0, 1]` from its position relative to that line, which the seven
//! kinds compute differently. In Color mode the ramp is painted over the
//! layer; in Transparency mode it erases, turning the existing pixels from
//! opaque to clear along the same ramp — paint.net's two gradient channels.
//!
//! The result is cropped to the selection, so a gradient inside a marquee
//! stops at its edge and feathered edges mix (see [`masked_region`]).

use std::f32::consts::TAU;

use pincel_effects::gradient::GradientLine;

use crate::color::Color;
use crate::command::HistoryKind;
use crate::command::SetPixels;
use crate::document::Document;
use crate::geometry::Point;
use crate::selection::masked_region;
use crate::tools::Blend;

/// Which way the ramp runs away from the drag (spec §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GradientKind {
    /// Along the drag; everything past `b` keeps the end color.
    #[default]
    Linear,
    /// Along the drag and mirrored back through `a`.
    Reflected,
    /// Out from `a` in every direction, reaching the end color at `b`.
    Radial,
    /// Around `a`, one full turn, starting at the drag's direction.
    Conical,
    /// Out from `a` by Manhattan distance in the drag's frame.
    Diamond,
    /// Around and out from `a`: the ramp repeats once per drag length
    /// outward and once per turn, so its bands wind clockwise.
    SpiralCw,
    /// As [`GradientKind::SpiralCw`], winding counter-clockwise.
    SpiralCcw,
}

/// What the ramp changes (spec §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GradientChannel {
    /// Paints from the start color to the end color.
    #[default]
    Color,
    /// Keeps the pixels and fades their alpha from opaque to clear.
    Transparency,
}

/// The drag, pre-computed once per gradient instead of per pixel. The
/// linear / radial math is shared with Pixel mode (`pincel-effects`).
struct Ramp {
    line: GradientLine,
    base_angle: f32,
}

/// The Gradient tool.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gradient {
    pub kind: GradientKind,
    pub channel: GradientChannel,
    /// Color at `t = 0`.
    pub from: Color,
    /// Color at `t = 1`.
    pub to: Color,
    /// Whole-gradient opacity, 0.0–1.0.
    pub opacity: f32,
    /// Normal or Overwrite for the Color channel; the Transparency channel
    /// ignores it (docs/specs/image-edit-extras-tool-options.md).
    pub blend: Blend,
}

impl Default for Gradient {
    fn default() -> Self {
        Self {
            kind: GradientKind::default(),
            channel: GradientChannel::default(),
            from: Color::BLACK,
            to: Color::TRANSPARENT,
            opacity: 1.0,
            blend: crate::tools::Blend::Normal,
        }
    }
}

impl Gradient {
    /// Paints the ramp defined by the drag `a` → `b` into `layer_index`.
    ///
    /// Returns `None` for an invalid layer, a drag of zero length (there is
    /// no direction to ramp along), or an empty selection.
    pub fn draw(
        &self,
        layer_index: usize,
        a: Point,
        b: Point,
        doc: &Document,
    ) -> Option<SetPixels> {
        let src = &doc.layers.get(layer_index)?.pixels;
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let ramp = Ramp {
            line: GradientLine::new(dx, dy)?,
            base_angle: dy.atan2(dx),
        };

        let w = src.width();
        let h = src.height();
        let mut after = src.clone();
        for y in 0..h {
            for x in 0..w {
                // Sample at the pixel's centre, so a one-pixel drag is not
                // entirely on one side of the ramp.
                let px = x as f32 + 0.5 - a.x;
                let py = y as f32 + 0.5 - a.y;
                let t = self.parameter(px, py, &ramp);
                let Some(dst) = after.get_pixel(x, y) else {
                    continue;
                };
                after.set_pixel(x, y, self.sample(t, dst));
            }
        }

        let (region, cropped) = masked_region(src, &after, doc.selection.as_ref())?;
        Some(SetPixels::new(layer_index, region, cropped).with_kind(HistoryKind::Gradient))
    }

    /// `t` in `[0, 1]` for the pixel offset `(px, py)` from the drag's start.
    fn parameter(&self, px: f32, py: f32, r: &Ramp) -> f32 {
        match self.kind {
            GradientKind::Linear => r.line.linear(px, py),
            GradientKind::Reflected => r.line.projection(px, py).abs().clamp(0.0, 1.0),
            GradientKind::Radial => r.line.radial(px, py),
            GradientKind::Conical => {
                // `rem_euclid` wraps the turn into [0, TAU) so the seam sits
                // exactly along the drag.
                (py.atan2(px) - r.base_angle).rem_euclid(TAU) / TAU
            }
            GradientKind::Diamond => {
                // Manhattan distance in the drag's own frame: the axes are
                // the drag direction and its perpendicular.
                let len = r.line.len();
                let ux = r.line.dx() / len;
                let uy = r.line.dy() / len;
                let along = px * ux + py * uy;
                let across = -px * uy + py * ux;
                ((along.abs() + across.abs()) / len).clamp(0.0, 1.0)
            }
            GradientKind::SpiralCw => spiral(px, py, r, -1.0),
            GradientKind::SpiralCcw => spiral(px, py, r, 1.0),
        }
    }

    /// The pixel `dst` becomes this once the ramp at `t` is applied.
    fn sample(&self, t: f32, dst: Color) -> Color {
        match self.channel {
            GradientChannel::Color => {
                let mix = |s: u8, e: u8| -> u8 {
                    let v = f32::from(s) + (f32::from(e) - f32::from(s)) * t;
                    v.clamp(0.0, 255.0).round() as u8
                };
                let color = Color::rgba(
                    mix(self.from.r, self.to.r),
                    mix(self.from.g, self.to.g),
                    mix(self.from.b, self.to.b),
                    255,
                );
                let alpha = (f32::from(self.from.a)
                    + (f32::from(self.to.a) - f32::from(self.from.a)) * t)
                    / 255.0;
                self.blend.paint(color, alpha * self.opacity, 1.0, dst)
            }
            GradientChannel::Transparency => {
                // t = 0 keeps the pixel, t = 1 clears it.
                let keep = 1.0 - t * self.opacity;
                let a = (f32::from(dst.a) * keep).round().clamp(0.0, 255.0) as u8;
                if a == 0 {
                    Color::TRANSPARENT
                } else {
                    Color::rgba(dst.r, dst.g, dst.b, a)
                }
            }
        }
    }
}

/// `frac(r / L + turn · θ / 360°)` with `turn` −1 for a clockwise spiral
/// and +1 for a counter-clockwise one (docs/specs/image-palette-gradient-spiral.md
/// AC-02). `θ` is the angle from the drag direction, clockwise on screen,
/// and 0 at the drag start itself.
fn spiral(px: f32, py: f32, r: &Ramp, turn: f32) -> f32 {
    let theta = if px == 0.0 && py == 0.0 {
        0.0
    } else {
        (py.atan2(px) - r.base_angle).rem_euclid(TAU) / TAU
    };
    let v = px.hypot(py) / r.line.len() + turn * theta;
    // `floor`, not truncation, so a negative `v` keeps winding; rounding can
    // land exactly on 1.0, which is the seam's start color.
    let f = v - v.floor();
    if f >= 1.0 { 0.0 } else { f }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Command;
    use crate::document::ImageBuffer;
    use crate::selection::SelectionMask;

    fn opaque_doc(w: u32, h: u32, c: Color) -> Document {
        let mut doc = Document::new(w, h).unwrap();
        let b: &mut ImageBuffer = &mut doc.layers[0].pixels;
        for y in 0..h {
            for x in 0..w {
                b.set_pixel(x, y, c);
            }
        }
        doc
    }

    fn black_to_white() -> Gradient {
        Gradient {
            from: Color::BLACK,
            to: Color::WHITE,
            ..Gradient::default()
        }
    }

    #[test]
    fn linear_runs_from_the_start_color_to_the_end_color() {
        let mut doc = opaque_doc(5, 1, Color::TRANSPARENT);
        let mut cmd = black_to_white()
            .draw(0, Point::new(0.0, 0.0), Point::new(5.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let left = doc.layers[0].pixels.get_pixel(0, 0).unwrap();
        let right = doc.layers[0].pixels.get_pixel(4, 0).unwrap();
        assert!(left.r < right.r, "left {left:?} right {right:?}");
        assert_eq!(left.a, 255);
    }

    #[test]
    fn linear_holds_the_end_color_past_the_drag() {
        let mut doc = opaque_doc(8, 1, Color::TRANSPARENT);
        let mut cmd = black_to_white()
            .draw(0, Point::new(0.0, 0.0), Point::new(4.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        let at_end = doc.layers[0].pixels.get_pixel(4, 0).unwrap();
        let past_end = doc.layers[0].pixels.get_pixel(7, 0).unwrap();
        assert_eq!(past_end, Color::WHITE);
        assert!(at_end.r <= past_end.r);
    }

    #[test]
    fn reflected_mirrors_through_the_start() {
        let mut doc = opaque_doc(9, 1, Color::TRANSPARENT);
        let mut cmd = Gradient {
            kind: GradientKind::Reflected,
            ..black_to_white()
        }
        .draw(0, Point::new(4.5, 0.0), Point::new(8.5, 0.0), &doc)
        .unwrap();
        cmd.apply(&mut doc).unwrap();
        let left = doc.layers[0].pixels.get_pixel(0, 0).unwrap();
        let right = doc.layers[0].pixels.get_pixel(8, 0).unwrap();
        assert_eq!(left.r, right.r);
    }

    #[test]
    fn radial_is_symmetric_around_the_start() {
        let mut doc = opaque_doc(5, 5, Color::TRANSPARENT);
        let mut cmd = Gradient {
            kind: GradientKind::Radial,
            ..black_to_white()
        }
        .draw(0, Point::new(2.5, 2.5), Point::new(4.5, 2.5), &doc)
        .unwrap();
        cmd.apply(&mut doc).unwrap();
        let px = doc.layers[0].pixels.get_pixel(0, 2).unwrap();
        let py = doc.layers[0].pixels.get_pixel(2, 0).unwrap();
        assert_eq!(px.r, py.r);
    }

    #[test]
    fn transparency_channel_keeps_the_color_and_clears_the_alpha() {
        let mut doc = opaque_doc(4, 1, Color::rgba(10, 20, 30, 255));
        let mut cmd = Gradient {
            channel: GradientChannel::Transparency,
            ..Gradient::default()
        }
        .draw(0, Point::new(0.0, 0.0), Point::new(4.0, 0.0), &doc)
        .unwrap();
        cmd.apply(&mut doc).unwrap();
        let near = doc.layers[0].pixels.get_pixel(0, 0).unwrap();
        let far = doc.layers[0].pixels.get_pixel(3, 0).unwrap();
        assert_eq!((near.r, near.g, near.b), (10, 20, 30));
        assert!(far.a < near.a, "near {near:?} far {far:?}");
    }

    #[test]
    fn a_selection_bounds_the_gradient() {
        let mut doc = opaque_doc(8, 1, Color::TRANSPARENT);
        let mut mask = SelectionMask::new_empty(8, 1);
        for x in 2..5 {
            mask.set(x, 0, 255);
        }
        doc.selection = Some(mask);
        let mut cmd = black_to_white()
            .draw(0, Point::new(0.0, 0.0), Point::new(8.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(0, 0),
            Some(Color::TRANSPARENT)
        );
        assert!(doc.layers[0].pixels.get_pixel(3, 0).unwrap().a > 0);
        assert_eq!(
            doc.layers[0].pixels.get_pixel(7, 0),
            Some(Color::TRANSPARENT)
        );
    }

    #[test]
    fn zero_length_drag_returns_none() {
        let doc = opaque_doc(4, 4, Color::TRANSPARENT);
        assert!(
            black_to_white()
                .draw(0, Point::new(1.0, 1.0), Point::new(1.0, 1.0), &doc)
                .is_none()
        );
    }

    #[test]
    fn invalid_layer_returns_none() {
        let doc = opaque_doc(4, 4, Color::TRANSPARENT);
        assert!(
            black_to_white()
                .draw(9, Point::new(0.0, 0.0), Point::new(4.0, 0.0), &doc)
                .is_none()
        );
    }

    /// FNV-1a over the layer's RGBA bytes after drawing `kind` from
    /// (1.25, 2.5) to (7.5, 5.75) over a 9×7 opaque grey layer.
    fn kind_digest(kind: GradientKind, channel: GradientChannel) -> u64 {
        let mut doc = opaque_doc(9, 7, Color::rgba(90, 120, 150, 255));
        let mut cmd = Gradient {
            kind,
            channel,
            from: Color::rgba(200, 30, 10, 255),
            to: Color::rgba(10, 60, 240, 128),
            opacity: 0.8,
            blend: crate::tools::Blend::Normal,
        }
        .draw(0, Point::new(1.25, 2.5), Point::new(7.5, 5.75), &doc)
        .unwrap();
        cmd.apply(&mut doc).unwrap();
        doc.layers[0]
            .pixels
            .data()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |h, &b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            })
    }

    /// Pins every kind's output to the values from before the linear /
    /// radial parameter math moved into `pincel-effects`
    /// (`docs/specs/dithered-gradient.md` AC-14).
    #[test]
    fn draw_every_kind_matches_the_pinned_output() {
        let digests = [
            GradientKind::Linear,
            GradientKind::Reflected,
            GradientKind::Radial,
            GradientKind::Conical,
            GradientKind::Diamond,
        ]
        .map(|k| {
            (
                kind_digest(k, GradientChannel::Color),
                kind_digest(k, GradientChannel::Transparency),
            )
        });
        assert_eq!(digests, PINNED_DIGESTS);
    }

    const PINNED_DIGESTS: [(u64, u64); 5] = [
        (6_338_608_836_344_469_668, 1_557_870_034_624_219_300),
        (2_370_596_846_134_834_344, 1_612_726_249_743_352_031),
        (14_321_446_003_203_966_033, 15_671_960_150_631_404_222),
        (2_078_452_278_065_046_774, 12_820_304_232_706_594_046),
        (12_999_643_837_064_416_437, 12_984_060_960_515_096_344),
    ];

    #[test]
    fn gradient_is_undoable() {
        let mut doc = opaque_doc(4, 1, Color::rgba(1, 2, 3, 255));
        let mut cmd = black_to_white()
            .draw(0, Point::new(0.0, 0.0), Point::new(4.0, 0.0), &doc)
            .unwrap();
        cmd.apply(&mut doc).unwrap();
        cmd.revert(&mut doc).unwrap();
        assert_eq!(
            doc.layers[0].pixels.get_pixel(0, 0),
            Some(Color::rgba(1, 2, 3, 255))
        );
    }
}
