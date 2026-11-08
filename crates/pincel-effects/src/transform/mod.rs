//! Affine resampling and Layer ▸ Rotate / Zoom
//! (docs/specs/image-transform-handles-rotate-zoom.md).
//!
//! [`resample`] reads a source buffer of any size, transparent outside its
//! pixels, through an inverse [`Affine`] map into a target buffer of any
//! size, with [`Quality`] Nearest, Bilinear or Bicubic taps. [`RotateZoom`]
//! runs it with source = target = the layer; the on-canvas transform
//! handles (`image-transform-handles`) run it on a lifted patch.
//!
//! Coordinates are pixel-edge coordinates: `(0, 0)` is the top-left corner
//! of pixel `(0, 0)`, whose centre is `(0.5, 0.5)`. Nearest copies source
//! bytes exactly; Bilinear and Bicubic interpolate premultiplied pixels
//! (see [`crate::EffectImage::to_premultiplied_f32`]), and a sample that
//! falls exactly on a source pixel centre copies that pixel's bytes.

mod rotate_zoom;

pub use rotate_zoom::RotateZoom;

use crate::image::EffectImage;

/// Resampling quality of [`resample`]. Travels on the wire as its index
/// (`0` = Nearest, `1` = Bilinear, `2` = Bicubic); mirrors
/// `pincel_image_core::transform::Interpolation` without Supersampling.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Quality {
    /// The source pixel the sample position falls in: exact source colours.
    Nearest,
    /// A 2×2 weighted average of premultiplied pixels.
    #[default]
    Bilinear,
    /// A 4×4 Catmull-Rom filter of premultiplied pixels.
    Bicubic,
}

impl Quality {
    /// Reads a wire index: `0` Nearest, `1` Bilinear, anything above
    /// Bicubic.
    pub fn from_index(index: u32) -> Self {
        match index {
            0 => Self::Nearest,
            1 => Self::Bilinear,
            _ => Self::Bicubic,
        }
    }
}

/// An affine map of the plane in pixel-edge coordinates:
/// `(x, y) → (a·x + b·y + c, d·x + e·y + f)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    /// x factor of the new x.
    pub a: f64,
    /// y factor of the new x.
    pub b: f64,
    /// Offset of the new x.
    pub c: f64,
    /// x factor of the new y.
    pub d: f64,
    /// y factor of the new y.
    pub e: f64,
    /// Offset of the new y.
    pub f: f64,
}

impl Affine {
    /// The map that leaves every point where it is.
    pub const IDENTITY: Self = Self {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 0.0,
        e: 1.0,
        f: 0.0,
    };

    /// The forward map of Rotate / Zoom: turn by `angle` degrees about
    /// `center` (clockwise on screen, where y points down, for a positive
    /// angle), scale by `zoom` about `center`, then shift by `offset`.
    /// Multiples of 90° use exact sines and cosines.
    pub fn rotate_zoom(center: (f64, f64), angle: f64, zoom: f64, offset: (f64, f64)) -> Self {
        let (sin, cos) = sin_cos_degrees(angle);
        let (a, b, d, e) = (zoom * cos, -zoom * sin, zoom * sin, zoom * cos);
        let (cx, cy) = center;
        Self {
            a,
            b,
            c: cx + offset.0 - a * cx - b * cy,
            d,
            e,
            f: cy + offset.1 - d * cx - e * cy,
        }
    }

    /// The inverse map; `None` when the map is not invertible (it collapses
    /// the plane, or a factor is not finite).
    pub fn inverse(&self) -> Option<Self> {
        let det = self.a * self.e - self.b * self.d;
        if !det.is_finite() || det == 0.0 {
            return None;
        }
        let (a, b, d, e) = (self.e / det, -self.b / det, -self.d / det, self.a / det);
        let inv = Self {
            a,
            b,
            c: -(a * self.c + b * self.f),
            d,
            e,
            f: -(d * self.c + e * self.f),
        };
        [inv.a, inv.b, inv.c, inv.d, inv.e, inv.f]
            .iter()
            .all(|v| v.is_finite())
            .then_some(inv)
    }

    /// Maps the point `(x, y)`.
    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.b * y + self.c,
            self.d * x + self.e * y + self.f,
        )
    }
}

/// `(sin, cos)` of `degrees`, exact at multiples of 90°.
fn sin_cos_degrees(degrees: f64) -> (f64, f64) {
    let r = degrees.rem_euclid(360.0);
    match r {
        0.0 => (0.0, 1.0),
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        _ => r.to_radians().sin_cos(),
    }
}

/// Resamples `src` into a `width` × `height` buffer: target pixel `(x, y)`
/// reads the source at `inverse` applied to its centre `(x + 0.5, y + 0.5)`
/// with `quality`'s taps; taps outside the source read as fully
/// transparent, so a target pixel no source pixel reaches is transparent.
pub fn resample(
    src: &EffectImage,
    width: u32,
    height: u32,
    inverse: &Affine,
    quality: Quality,
) -> EffectImage {
    let sampler = Sampler {
        src,
        premultiplied: match quality {
            Quality::Nearest => Vec::new(),
            Quality::Bilinear | Quality::Bicubic => src.to_premultiplied_f32(),
        },
        quality,
    };
    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        for x in 0..width {
            let (sx, sy) = inverse.apply(f64::from(x) + 0.5, f64::from(y) + 0.5);
            out.extend_from_slice(&sampler.at(sx, sy));
        }
    }
    EffectImage::from_rgba8(width, height, out).expect("width · height pixels")
}

/// Pixel reads of one [`resample`] run.
struct Sampler<'a> {
    src: &'a EffectImage,
    /// The source premultiplied (empty for Nearest).
    premultiplied: Vec<f32>,
    quality: Quality,
}

/// How close a sample position must be to a pixel centre to read that
/// pixel's exact bytes.
const EXACT: f64 = 1e-9;

impl Sampler<'_> {
    /// The source pixel `(x, y)`'s bytes; transparent outside.
    fn bytes(&self, x: i64, y: i64) -> [u8; 4] {
        let (w, h) = (i64::from(self.src.width()), i64::from(self.src.height()));
        if x < 0 || y < 0 || x >= w || y >= h {
            return [0; 4];
        }
        let i = ((y * w + x) * 4) as usize;
        let d = self.src.data();
        [d[i], d[i + 1], d[i + 2], d[i + 3]]
    }

    /// The source pixel `(x, y)` premultiplied; transparent outside.
    fn premul(&self, x: i64, y: i64) -> [f32; 4] {
        let (w, h) = (i64::from(self.src.width()), i64::from(self.src.height()));
        if x < 0 || y < 0 || x >= w || y >= h {
            return [0.0; 4];
        }
        let i = ((y * w + x) * 4) as usize;
        let p = &self.premultiplied;
        [p[i], p[i + 1], p[i + 2], p[i + 3]]
    }

    /// The colour at pixel-edge position `(x, y)`.
    fn at(&self, x: f64, y: f64) -> [u8; 4] {
        if !x.is_finite() || !y.is_finite() {
            return [0; 4];
        }
        if self.quality == Quality::Nearest {
            return self.bytes(x.floor() as i64, y.floor() as i64);
        }
        // Pixel-centre coordinates: source pixel i sits at i.
        let (u, v) = (x - 0.5, y - 0.5);
        let (iu, iv) = (u.round(), v.round());
        if (u - iu).abs() < EXACT && (v - iv).abs() < EXACT {
            return self.bytes(iu as i64, iv as i64);
        }
        let (fu, fv) = (u.floor(), v.floor());
        let (tu, tv) = (u - fu, v - fv);
        let (x0, y0) = (fu as i64, fv as i64);
        let (wx, wy, from) = match self.quality {
            Quality::Bicubic => (catmull_rom(tu), catmull_rom(tv), -1),
            _ => ([1.0 - tu, tu, 0.0, 0.0], [1.0 - tv, tv, 0.0, 0.0], 0),
        };
        let mut acc = [0.0f64; 4];
        for (j, wyj) in wy.iter().enumerate().filter(|(_, w)| **w != 0.0) {
            for (i, wxi) in wx.iter().enumerate().filter(|(_, w)| **w != 0.0) {
                let p = self.premul(x0 + from + i as i64, y0 + from + j as i64);
                let k = wxi * wyj;
                for (a, c) in acc.iter_mut().zip(p) {
                    *a += k * f64::from(c);
                }
            }
        }
        straight(acc)
    }
}

/// Catmull-Rom weights of the taps at -1, 0, 1, 2 for offset `t` in `0..1`.
fn catmull_rom(t: f64) -> [f64; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [
        (-t3 + 2.0 * t2 - t) / 2.0,
        (3.0 * t3 - 5.0 * t2 + 2.0) / 2.0,
        (-3.0 * t3 + 4.0 * t2 + t) / 2.0,
        (t3 - t2) / 2.0,
    ]
}

/// A premultiplied sample as straight RGBA8; alpha clamps to `0..=1` and
/// each colour channel to `0..=alpha` (a Catmull-Rom overshoot).
fn straight(p: [f64; 4]) -> [u8; 4] {
    let a = p[3].clamp(0.0, 1.0);
    let byte = |v: f64| (v * 255.0).round() as u8;
    if a == 0.0 {
        return [0; 4];
    }
    let c = |v: f64| byte((v.clamp(0.0, a) / a).clamp(0.0, 1.0));
    [c(p[0]), c(p[1]), c(p[2]), byte(a)]
}

#[cfg(test)]
mod tests;
