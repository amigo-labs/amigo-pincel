//! The geometric warps of paint.net's Distort menu
//! (docs/specs/effects-distort.md): Bulge, Polar Inversion, Tile Reflection
//! and Twist. Each maps every output pixel to a source position and reads
//! it bilinearly through [`remap`].

use std::f32::consts::{PI, TAU};

use super::edge::{EdgeBehavior, Source, centre, remap};
use crate::blur::{angle, param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Half the shorter image side: the unit radius of Bulge, Polar Inversion
/// and Twist.
fn half_side(src: &EffectImage) -> f32 {
    src.width().min(src.height()) as f32 / 2.0
}

/// Magnifies (`amount > 0`) or pinches (`< 0`) the circle of radius half
/// the shorter image side around a centre point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bulge {
    /// Strength, `-200..=100`; 0 leaves the layer unchanged.
    pub amount: f32,
    /// Centre as a fraction of the width, `0..=1`.
    pub center_x: f32,
    /// Centre as a fraction of the height, `0..=1`.
    pub center_y: f32,
    /// What positions outside the layer read.
    pub edge: EdgeBehavior,
}

impl Bulge {
    /// Creates a bulge (non-finite numbers read as 0, then clamped).
    pub fn new(amount: f32, center_x: f32, center_y: f32, edge: EdgeBehavior) -> Self {
        Self {
            amount: param(amount, -200.0, 100.0),
            center_x: param(center_x, 0.0, 1.0),
            center_y: param(center_y, 0.0, 1.0),
            edge,
        }
    }
}

impl Effect for Bulge {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.amount == 0.0 {
            return src.clone();
        }
        let (cx, cy) = (
            centre(self.center_x, src.width()),
            centre(self.center_y, src.height()),
        );
        let radius = half_side(src);
        let k = self.amount / 100.0;
        remap(src, self.edge, |x, y| {
            let (u, v) = (x - cx, y - cy);
            let r = (u * u + v * v).sqrt() / radius;
            if r >= 1.0 {
                return Source::Keep;
            }
            // paint.net's bulge: scale the offset by 1 - k (1 - r)^2.
            let s = 1.0 - k * (1.0 - r) * (1.0 - r);
            Source::At(cx + u * s, cy + v * s)
        })
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

/// Maps each pixel through the circle inversion z ↦ amount / z̄ around a
/// centre point, z in units of half the shorter image side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarInversion {
    /// Inversion strength, `-4..=4`.
    pub amount: f32,
    /// Centre as a fraction of the width, `0..=1`.
    pub center_x: f32,
    /// Centre as a fraction of the height, `0..=1`.
    pub center_y: f32,
    /// What positions outside the layer read.
    pub edge: EdgeBehavior,
}

impl PolarInversion {
    /// Creates a polar inversion (non-finite numbers read as 0, then
    /// clamped).
    pub fn new(amount: f32, center_x: f32, center_y: f32, edge: EdgeBehavior) -> Self {
        Self {
            amount: param(amount, -4.0, 4.0),
            center_x: param(center_x, 0.0, 1.0),
            center_y: param(center_y, 0.0, 1.0),
            edge,
        }
    }
}

impl Effect for PolarInversion {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (cx, cy) = (
            centre(self.center_x, src.width()),
            centre(self.center_y, src.height()),
        );
        let radius = half_side(src);
        remap(src, self.edge, |x, y| {
            let (u, v) = ((x - cx) / radius, (y - cy) / radius);
            let d2 = u * u + v * v;
            if d2 == 0.0 {
                // The centre maps to infinity: outside the layer.
                return Source::At(f32::INFINITY, f32::INFINITY);
            }
            // amount / conj(z) = amount · z / |z|².
            let k = self.amount / d2;
            Source::At(cx + u * k * radius, cy + v * k * radius)
        })
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

/// Overlays a grid of rotated glass tiles, each refracting the image
/// beneath it with the given curvature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileReflection {
    /// Tile size in pixels, `2..=200`.
    pub tile_size: u32,
    /// Grid rotation in degrees, wrapped into `0..360`.
    pub angle: f32,
    /// Refraction of each tile, `-100..=100`.
    pub curvature: f32,
    /// What positions outside the layer read.
    pub edge: EdgeBehavior,
}

impl TileReflection {
    /// Creates a tile reflection (non-finite numbers read as 0; tile size
    /// rounded and clamped, the angle wrapped, the curvature clamped).
    pub fn new(tile_size: f32, angle_deg: f32, curvature: f32, edge: EdgeBehavior) -> Self {
        Self {
            tile_size: whole(tile_size, 2, 200),
            angle: angle(angle_deg),
            curvature: param(curvature, -100.0, 100.0),
            edge,
        }
    }
}

impl Effect for TileReflection {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (cx, cy) = (centre(0.5, src.width()), centre(0.5, src.height()));
        let scale = PI / self.tile_size as f32;
        let (sin, cos) = crate::math::sin_cos(self.angle.to_radians());
        let curvature = self.curvature;
        remap(src, self.edge, |x, y| {
            let (u, v) = (x - cx, y - cy);
            // Into the tile grid's frame, refract, and back (Pinta's
            // TileReflectionEffect).
            let s = cos * u + sin * v;
            let t = -sin * u + cos * v;
            let s = s + curvature * libm::tanf(s * scale);
            let t = t + curvature * libm::tanf(t * scale);
            Source::At(cx + cos * s - sin * t, cy + sin * s + cos * t)
        })
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            tile_size: ((self.tile_size as f32 * factor).round() as u32).max(2),
            ..*self
        }
    }
}

/// Rotates the pixels inside a circle around a centre point by an angle
/// that grows toward the centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Twist {
    /// Strength, `-100..=100`: 100 turns the centre once around; 0 leaves
    /// the layer unchanged.
    pub amount: f32,
    /// Circle radius in percent of half the shorter image side, `1..=200`.
    pub size: f32,
    /// Centre as a fraction of the width, `0..=1`.
    pub center_x: f32,
    /// Centre as a fraction of the height, `0..=1`.
    pub center_y: f32,
    /// What positions outside the layer read.
    pub edge: EdgeBehavior,
}

impl Twist {
    /// Creates a twist (non-finite numbers read as 0, then clamped).
    pub fn new(amount: f32, size: f32, center_x: f32, center_y: f32, edge: EdgeBehavior) -> Self {
        Self {
            amount: param(amount, -100.0, 100.0),
            size: param(size, 1.0, 200.0),
            center_x: param(center_x, 0.0, 1.0),
            center_y: param(center_y, 0.0, 1.0),
            edge,
        }
    }
}

impl Effect for Twist {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.amount == 0.0 {
            return src.clone();
        }
        let (cx, cy) = (
            centre(self.center_x, src.width()),
            centre(self.center_y, src.height()),
        );
        let radius = half_side(src) * self.size / 100.0;
        let turn = self.amount / 100.0 * TAU;
        remap(src, self.edge, |x, y| {
            let (u, v) = (x - cx, y - cy);
            let r = (u * u + v * v).sqrt();
            if r >= radius {
                return Source::Keep;
            }
            let t = 1.0 - r / radius;
            let (sin, cos) = crate::math::sin_cos(turn * t * t);
            Source::At(cx + cos * u - sin * v, cy + sin * u + cos * v)
        })
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}
