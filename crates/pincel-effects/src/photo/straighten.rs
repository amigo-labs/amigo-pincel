//! Straighten (docs/specs/effects-artistic-photo.md): rotates the whole
//! layer around its centre, optionally zoomed so no corner is uncovered.

use crate::blur::param;
use crate::distort::EdgeBehavior;
use crate::distort::edge::{Source, remap};
use crate::effect::Effect;
use crate::image::EffectImage;

/// What fills the corners a rotation uncovers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StraightenFill {
    /// Scale the rotated layer up just enough that it covers the layer.
    #[default]
    ZoomToFill,
    /// Keep the size; the uncovered corners become transparent.
    TransparentCorners,
}

/// Rotates the layer (colour and alpha, bilinear on premultiplied pixels)
/// by `angle` degrees around its centre, clockwise on screen for a positive
/// angle like the angle dial. Angle 0 leaves the layer unchanged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Straighten {
    /// Rotation in degrees, `-45..=45`.
    pub angle: f32,
    /// What fills the uncovered corners.
    pub fill: StraightenFill,
}

impl Straighten {
    /// Creates a rotation (a non-finite angle reads as 0, then clamped).
    pub fn new(angle: f32, fill: StraightenFill) -> Self {
        Self {
            angle: param(angle, -45.0, 45.0),
            fill,
        }
    }
}

impl Straighten {
    /// The source position output pixel `(x, y)` of a `w` x `h` layer reads
    /// (pixel-centre coordinates): the inverse rotation, then the inverse
    /// zoom.
    fn source(&self, w: f32, h: f32) -> impl Fn(f32, f32) -> (f32, f32) {
        let (sin, cos) = self.angle.to_radians().sin_cos();
        let scale = match self.fill {
            // The rotated w x h rectangle, scaled by s, covers the layer
            // when s >= cos|a| + max(w / h, h / w) · sin|a|.
            StraightenFill::ZoomToFill => cos + (w / h).max(h / w) * sin.abs(),
            StraightenFill::TransparentCorners => 1.0,
        };
        let (cx, cy) = ((w - 1.0) / 2.0, (h - 1.0) / 2.0);
        move |x, y| {
            let (u, v) = (x - cx, y - cy);
            (
                (u * cos + v * sin) / scale + cx,
                (v * cos - u * sin) / scale + cy,
            )
        }
    }
}

impl Effect for Straighten {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.angle == 0.0 {
            return src.clone();
        }
        let edge = match self.fill {
            StraightenFill::ZoomToFill => EdgeBehavior::Clamp,
            StraightenFill::TransparentCorners => EdgeBehavior::Transparent,
        };
        let source = self.source(src.width() as f32, src.height() as f32);
        remap(src, edge, |x, y| {
            let (sx, sy) = source(x, y);
            Source::At(sx, sy)
        })
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// How far the layer's outer corners map outside its pixel area, at
    /// most (negative: all inside).
    fn overshoot(s: &Straighten, w: f32, h: f32) -> f32 {
        let source = s.source(w, h);
        [
            (-0.5, -0.5),
            (w - 0.5, -0.5),
            (-0.5, h - 0.5),
            (w - 0.5, h - 0.5),
        ]
        .into_iter()
        .map(|(x, y)| {
            let (sx, sy) = source(x, y);
            let out_x = (-0.5 - sx).max(sx - (w - 0.5));
            let out_y = (-0.5 - sy).max(sy - (h - 0.5));
            out_x.max(out_y)
        })
        .fold(f32::MIN, f32::max)
    }

    #[test]
    fn source_zoom_to_fill_scales_just_enough_to_cover_the_layer() {
        for (w, h) in [(40.0, 30.0), (9.0, 31.0), (400.0, 20.0)] {
            for angle in [-45.0, -12.5, -0.1, 0.1, 3.0, 45.0] {
                let zoom = Straighten::new(angle, StraightenFill::ZoomToFill);
                let o = overshoot(&zoom, w, h);
                // Every corner inside, the outermost on the edge.
                assert!(o.abs() < 1e-3 * w.max(h), "{w} x {h} at {angle}: {o}");
                let corners = Straighten::new(angle, StraightenFill::TransparentCorners);
                assert!(overshoot(&corners, w, h) > 0.0, "{w} x {h} at {angle}");
            }
        }
    }
}
