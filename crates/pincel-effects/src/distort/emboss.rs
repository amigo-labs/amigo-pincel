//! Emboss (Fineliner spec §11.3): angle, elevation, relief.

use super::gradient;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Emboss: lights the image as a height field (luminance = height) from a
/// direction, producing a grey relief. Alpha is preserved.
///
/// - `angle` is the light azimuth in degrees (0–360).
/// - `elevation` is the light height in degrees (0–90); it sets the flat-area
///   grey.
/// - `relief` (1–10) steepens the surface, deepening highlights and shadows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emboss {
    /// Light azimuth in degrees.
    pub angle: f32,
    /// Light elevation in degrees.
    pub elevation: f32,
    /// Surface steepness, 1–10.
    pub relief: f32,
}

impl Emboss {
    /// Creates an emboss effect.
    pub fn new(angle: f32, elevation: f32, relief: f32) -> Self {
        Self {
            angle,
            elevation,
            relief,
        }
    }
}

impl Effect for Emboss {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        // Light direction (unit vector).
        let az = self.angle.to_radians();
        let el = self.elevation.to_radians();
        let (lx, ly, lz) = (az.cos() * el.cos(), az.sin() * el.cos(), el.sin());
        let mut out = vec![0u8; data.len()];
        for y in 0..h {
            for x in 0..w {
                let (gx, gy) = gradient(data, w, h, x as i32, y as i32, 2.0);
                // Surface normal from the gradient; relief steepens it.
                let (nx, ny, nz) = (-gx * self.relief, -gy * self.relief, 1.0);
                let len = (nx * nx + ny * ny + nz * nz).sqrt();
                let intensity = ((nx * lx + ny * ly + nz * lz) / len).clamp(0.0, 1.0);
                let g = (intensity * 255.0).round() as u8;
                let o = (y * w + x) * 4;
                out[o] = g;
                out[o + 1] = g;
                out[o + 2] = g;
                out[o + 3] = data[o + 3];
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        // Emboss is a 3×3 neighbourhood effect; independent of image scale.
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray_columns(cols: &[u8], height: u32) -> EffectImage {
        let w = cols.len() as u32;
        let mut data = Vec::new();
        for _ in 0..height {
            for &v in cols {
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
        EffectImage::from_rgba8(w, height, data).unwrap()
    }

    #[test]
    fn emboss_of_solid_is_flat_gray() {
        // A flat field has no gradient: every pixel becomes the same grey
        // (= sin(elevation)) and alpha is preserved.
        let src = gray_columns(&[100, 100, 100, 100], 4);
        let out = Emboss::new(135.0, 45.0, 5.0).apply(&src);
        let first = &out.data()[0..4];
        for px in out.data().as_chunks::<4>().0 {
            assert_eq!(px[0], px[1]);
            assert_eq!(px[1], px[2]);
            assert_eq!(px[3], 255);
            assert_eq!(px[0], first[0]);
        }
    }

    #[test]
    fn emboss_highlights_and_shadows_an_edge() {
        // A vertical step lit from the left/right produces both a bright and a
        // dark pixel around the seam, unlike the flat baseline.
        let src = gray_columns(&[60, 60, 200, 200], 3);
        let out = Emboss::new(0.0, 30.0, 6.0).apply(&src);
        let grays: Vec<u8> = out.data().as_chunks::<4>().0.iter().map(|p| p[0]).collect();
        let max = *grays.iter().max().unwrap();
        let min = *grays.iter().min().unwrap();
        assert!(max > min, "edge produced contrast ({} vs {})", max, min);
    }
}
