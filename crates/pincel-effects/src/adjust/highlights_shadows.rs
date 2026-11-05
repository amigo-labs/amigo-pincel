//! Highlights / Shadows (docs/specs/adjustments-paintnet.md): lifts or
//! darkens the dark and the bright tones separately.

use super::luma_x10000;
use crate::effect::Effect;
use crate::image::EffectImage;

/// With `s = shadows / 100`, `h = highlights / 100`, `l` the pixel's luma
/// in `0..=1` and each channel `c` in `0..=1`: `c += s · 0.5 · (1 − l)² ·
/// (1 − c)` for `s > 0` or `· c` for `s < 0`, then `c −= h · 0.5 · l² · c`
/// for `h > 0` or `· (1 − c)` for `h < 0`, clamped. Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighlightsShadows {
    /// `-100..=100`; positive lifts the shadows.
    pub shadows: f32,
    /// `-100..=100`; positive darkens the highlights.
    pub highlights: f32,
}

/// Non-finite reads as 0, then clamped to `-100..=100`.
fn percent(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-100.0, 100.0)
    } else {
        0.0
    }
}

impl HighlightsShadows {
    /// Creates the adjustment (non-finite reads as 0, then each value is
    /// clamped to `-100..=100`).
    pub fn new(shadows: f32, highlights: f32) -> Self {
        Self {
            shadows: percent(shadows),
            highlights: percent(highlights),
        }
    }
}

impl Effect for HighlightsShadows {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let s = f64::from(self.shadows) / 100.0;
        let h = f64::from(self.highlights) / 100.0;
        let mut out = src.data().to_vec();
        for p in out.as_chunks_mut::<4>().0 {
            let l = f64::from(luma_x10000(p[0], p[1], p[2])) / (10_000.0 * 255.0);
            let lift = s * 0.5 * (1.0 - l).powi(2);
            let cut = h * 0.5 * l * l;
            for ch in &mut p[..3] {
                let mut c = f64::from(*ch) / 255.0;
                c += if s > 0.0 { lift * (1.0 - c) } else { lift * c };
                c -= if h > 0.0 { cut * c } else { cut * (1.0 - c) };
                *ch = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gray(v: u8) -> EffectImage {
        EffectImage::from_rgba8(1, 1, vec![v, v, v, 255]).unwrap()
    }

    #[test]
    fn negative_shadows_darken_dark_tones() {
        let out = HighlightsShadows::new(-100.0, 0.0).apply(&gray(60));
        assert!(out.data()[0] < 60);
    }

    #[test]
    fn negative_highlights_brighten_bright_tones() {
        let out = HighlightsShadows::new(0.0, -100.0).apply(&gray(200));
        assert!(out.data()[0] > 200);
    }
}
