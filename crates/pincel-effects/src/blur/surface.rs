//! Surface Blur (docs/specs/effects-blurs.md): smooths flat areas, keeps
//! edges.

use super::whole;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Averages each pixel with the neighbours of its `(2·radius + 1)²` square
/// whose largest premultiplied RGBA channel difference from it is at most
/// `threshold` % of 255, so edges stay sharp while flat areas smooth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceBlur {
    /// Neighbourhood radius in pixels, `1..=100`.
    pub radius: u32,
    /// Largest channel difference that still mixes, in % of 255, `1..=100`.
    pub threshold: u32,
}

impl SurfaceBlur {
    /// Creates a surface blur (non-finite numbers read as 0, then clamped and
    /// rounded: radius and threshold to `1..=100`).
    pub fn new(radius: f32, threshold: f32) -> Self {
        Self {
            radius: whole(radius, 1, 100),
            threshold: whole(threshold, 1, 100),
        }
    }
}

impl Effect for SurfaceBlur {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as i64, src.height() as i64);
        if w == 0 || h == 0 {
            return src.clone();
        }
        let buf = src.to_premultiplied_f32();
        let px = buf.as_chunks::<4>().0;
        // Compared on the 0–255 scale; the epsilon absorbs f32 noise so a
        // difference of exactly the threshold still mixes.
        let limit = self.threshold as f32 * 2.55 + 1e-3;
        let r = i64::from(self.radius);
        let mut out = vec![0.0f32; buf.len()];
        for y in 0..h {
            for x in 0..w {
                let me = &px[(y * w + x) as usize];
                let mut acc = [0.0f32; 4];
                let mut count = 0u32;
                for dy in -r..=r {
                    let sy = (y + dy).clamp(0, h - 1);
                    for dx in -r..=r {
                        let sx = (x + dx).clamp(0, w - 1);
                        let q = &px[(sy * w + sx) as usize];
                        let near = (0..4).all(|c| (q[c] - me[c]).abs() * 255.0 <= limit);
                        if near {
                            for c in 0..4 {
                                acc[c] += q[c];
                            }
                            count += 1;
                        }
                    }
                }
                // The pixel itself always counts, so count >= 1.
                let o = ((y * w + x) * 4) as usize;
                for c in 0..4 {
                    out[o + c] = acc[c] / count as f32;
                }
            }
        }
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &out)
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: ((self.radius as f32 * factor).round() as u32).max(1),
            threshold: self.threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_blur_mixes_only_within_the_threshold() {
        // 40 apart = 15.7 % of 255: mixes at 20 %, not at 10 %. The clamped
        // 3x3 window of pixel 0 holds it six times and pixel 1 three times.
        let data = [[100u8, 100, 100, 255], [140, 140, 140, 255]].concat();
        let src = EffectImage::from_rgba8(2, 1, data).expect("2x1");
        assert_eq!(SurfaceBlur::new(1.0, 10.0).apply(&src), src);
        assert_eq!(SurfaceBlur::new(1.0, 20.0).apply(&src).data()[0], 113);
    }
}
