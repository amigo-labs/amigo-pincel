//! Julia Fractal and Mandelbrot Fractal (docs/specs/effects-render.md):
//! escape-time renderings, opaque everywhere, with plain (integer)
//! colouring so points inside the set stay exactly black.

use crate::blur::{angle, param, whole};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Iterations before a point counts as inside the set.
const MAX_ITER: u32 = 256;

/// The fixed Julia constant c = −0.4 + 0.6i.
const JULIA_C: (f64, f64) = (-0.4, 0.6);

/// RGB in `[0, 1]` of hue `deg` at full saturation and value.
fn hue(deg: f32) -> [f32; 3] {
    let h = deg.rem_euclid(360.0) / 60.0;
    let f = h - h.floor();
    match h as u32 {
        0 => [1.0, f, 0.0],
        1 => [1.0 - f, 1.0, 0.0],
        2 => [0.0, 1.0, f],
        3 => [0.0, 1.0 - f, 1.0],
        4 => [f, 0.0, 1.0],
        _ => [1.0, 0.0, 1.0 - f],
    }
}

/// The escape-time colour of a point that escaped after `n` iterations
/// (`None`: never, black).
fn color(n: Option<u32>, factor: f32) -> [f32; 3] {
    n.map_or([0.0; 3], |n| hue(n as f32 * factor * 7.0))
}

/// Iterates z ↦ z² + c from `z`; the iteration at which |z| exceeds 2, or
/// `None` within [`MAX_ITER`].
fn escape((mut zr, mut zi): (f64, f64), (cr, ci): (f64, f64)) -> Option<u32> {
    for n in 1..=MAX_ITER {
        let r = zr * zr - zi * zi + cr;
        zi = 2.0 * zr * zi + ci;
        zr = r;
        if zr * zr + zi * zi > 4.0 {
            return Some(n);
        }
    }
    None
}

/// `true` for a point of the Mandelbrot set's main cardioid or period-2
/// bulb, which never escapes (a shortcut, same result as iterating).
fn in_main_bulbs(cr: f64, ci: f64) -> bool {
    let x = cr - 0.25;
    let q = x * x + ci * ci;
    q * (q + x) <= 0.25 * ci * ci || (cr + 1.0).powi(2) + ci * ci <= 1.0 / 16.0
}

/// The view a fractal renders: centre, half-height and rotation.
#[derive(Clone, Copy)]
struct View {
    center: (f64, f64),
    half_height: f64,
    angle_deg: f32,
}

/// Renders `w` × `h` opaque pixels, averaging `quality`² samples each;
/// `point` gives the escape colour of a complex point.
fn render(
    w: u32,
    h: u32,
    view: View,
    quality: u32,
    point: impl Fn(f64, f64) -> [f32; 3],
) -> EffectImage {
    let unit = 2.0 * view.half_height / f64::from(h.max(1));
    let (sin, cos) = f64::from(view.angle_deg).to_radians().sin_cos();
    let (hw, hh) = (f64::from(w) / 2.0, f64::from(h) / 2.0);
    let q = quality.max(1);
    let samples = (q * q) as f32;
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            for sy in 0..q {
                for sx in 0..q {
                    let px = f64::from(x) + (f64::from(sx) + 0.5) / f64::from(q);
                    let py = f64::from(y) + (f64::from(sy) + 0.5) / f64::from(q);
                    // Image y grows down, the imaginary axis up.
                    let (dx, dy) = ((px - hw) * unit, (hh - py) * unit);
                    let re = view.center.0 + dx * cos - dy * sin;
                    let im = view.center.1 + dx * sin + dy * cos;
                    let c = point(re, im);
                    for ch in 0..3 {
                        acc[ch] += c[ch];
                    }
                }
            }
            for v in acc {
                out.push((v / samples * 255.0).round().clamp(0.0, 255.0) as u8);
            }
            out.push(255);
        }
    }
    EffectImage::from_rgba8(w, h, out).expect("w * h pixels")
}

/// Julia Fractal: the Julia set of z ↦ z² + (−0.4 + 0.6i) over the view
/// centred on 0 with half-height `2 / zoom`, rotated by `angle`; a point
/// escaping after n of 256 iterations takes hue `(n · factor · 7) mod 360`,
/// a point that never escapes is black. `quality`² samples per pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JuliaFractal {
    /// Colour cycling speed, `1..=10`.
    pub factor: f32,
    /// Magnification, `1..=50`.
    pub zoom: f32,
    /// View rotation in degrees, wrapped into `0..360`.
    pub angle: f32,
    /// Samples per pixel axis, `1..=5`.
    pub quality: u32,
}

impl JuliaFractal {
    /// Creates a Julia fractal (non-finite numbers read as 0, then clamped;
    /// the angle wrapped, the quality rounded).
    pub fn new(factor: f32, zoom: f32, angle_deg: f32, quality: f32) -> Self {
        Self {
            factor: param(factor, 1.0, 10.0),
            zoom: param(zoom, 1.0, 50.0),
            angle: angle(angle_deg),
            quality: whole(quality, 1, 5),
        }
    }
}

impl Effect for JuliaFractal {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let view = View {
            center: (0.0, 0.0),
            half_height: 2.0 / f64::from(self.zoom),
            angle_deg: self.angle,
        };
        render(src.width(), src.height(), view, self.quality, |re, im| {
            color(escape((re, im), JULIA_C), self.factor)
        })
    }

    /// The view is relative to the image size: nothing to scale.
    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

/// Mandelbrot Fractal: the Mandelbrot set over the view centred on
/// −0.5 + 0i with half-height `15 / zoom`, coloured as [`JuliaFractal`];
/// `invert` inverts every result pixel's colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MandelbrotFractal {
    /// Colour cycling speed, `1..=10`.
    pub factor: f32,
    /// Magnification, `1..=100`.
    pub zoom: f32,
    /// View rotation in degrees, wrapped into `0..360`.
    pub angle: f32,
    /// Samples per pixel axis, `1..=5`.
    pub quality: u32,
    /// Invert the colours of the result.
    pub invert: bool,
}

impl MandelbrotFractal {
    /// Creates a Mandelbrot fractal (non-finite numbers read as 0, then
    /// clamped; the angle wrapped, the quality rounded).
    pub fn new(factor: f32, zoom: f32, angle_deg: f32, quality: f32, invert: bool) -> Self {
        Self {
            factor: param(factor, 1.0, 10.0),
            zoom: param(zoom, 1.0, 100.0),
            angle: angle(angle_deg),
            quality: whole(quality, 1, 5),
            invert,
        }
    }
}

impl Effect for MandelbrotFractal {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let view = View {
            center: (-0.5, 0.0),
            half_height: 15.0 / f64::from(self.zoom),
            angle_deg: self.angle,
        };
        let out = render(src.width(), src.height(), view, self.quality, |re, im| {
            if in_main_bulbs(re, im) {
                return [0.0; 3];
            }
            color(escape((0.0, 0.0), (re, im)), self.factor)
        });
        if !self.invert {
            return out;
        }
        let mut data = out.into_raw();
        for p in data.as_chunks_mut::<4>().0 {
            for ch in &mut p[..3] {
                *ch = 255 - *ch;
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), data).expect("same dimensions")
    }

    /// The view is relative to the image size: nothing to scale.
    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_primaries() {
        assert_eq!(hue(0.0), [1.0, 0.0, 0.0]);
        assert_eq!(hue(120.0), [0.0, 1.0, 0.0]);
        assert_eq!(hue(240.0), [0.0, 0.0, 1.0]);
        assert_eq!(hue(360.0), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn escape_inside_never_escapes_and_far_points_escape_at_once() {
        assert_eq!(escape((0.0, 0.0), (-0.5, 0.0)), None);
        assert_eq!(escape((0.0, 0.0), (2.5, 0.0)), Some(1));
    }

    #[test]
    fn main_bulbs_shortcut_agrees_with_iterating() {
        for i in 0..200 {
            for j in 0..100 {
                let (cr, ci) = (-2.0 + f64::from(i) * 0.0137, f64::from(j) * 0.0113);
                if in_main_bulbs(cr, ci) {
                    assert_eq!(escape((0.0, 0.0), (cr, ci)), None, "({cr}, {ci})");
                }
            }
        }
    }
}
