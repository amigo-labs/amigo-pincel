//! Color Balance (Fineliner spec §12.5): per-tone-range channel shifts, optionally
//! preserving luminosity.

use super::{luma, map_rgb};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Color balance. Each range holds `[cyan↔red, magenta↔green, yellow↔blue]`
/// sliders in −100..+100 (positive = toward red/green/blue).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorBalance {
    /// Shadow-range shifts.
    pub shadows: [f32; 3],
    /// Midtone-range shifts.
    pub midtones: [f32; 3],
    /// Highlight-range shifts.
    pub highlights: [f32; 3],
    /// Re-apply the original luminance after shifting.
    pub preserve_luminosity: bool,
}

impl ColorBalance {
    /// Creates a color-balance adjustment.
    pub fn new(shadows: [f32; 3], midtones: [f32; 3], highlights: [f32; 3]) -> Self {
        Self {
            shadows,
            midtones,
            highlights,
            preserve_luminosity: false,
        }
    }

    /// Preserves luminosity after balancing.
    pub fn preserve_luminosity(mut self) -> Self {
        self.preserve_luminosity = true;
        self
    }
}

/// Per-channel shift strength: ±100 → up to ±0.5 in that tone range.
const STRENGTH: f32 = 0.5;

impl Effect for ColorBalance {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        map_rgb(src, |rgb| {
            let l = luma(rgb);
            // Tone weights: shadows peak at 0, midtones at 0.5, highlights at 1.
            let w_shadow = (1.0 - 2.0 * l).max(0.0);
            let w_high = (2.0 * l - 1.0).max(0.0);
            let w_mid = 1.0 - (2.0 * l - 1.0).abs();
            let mut out = [0.0f32; 3];
            for c in 0..3 {
                let shift = (self.shadows[c] * w_shadow
                    + self.midtones[c] * w_mid
                    + self.highlights[c] * w_high)
                    / 100.0
                    * STRENGTH;
                out[c] = (rgb[c] + shift).clamp(0.0, 1.0);
            }
            if self.preserve_luminosity {
                let delta = l - luma(out);
                for c in &mut out {
                    *c = (*c + delta).clamp(0.0, 1.0);
                }
            }
            out
        })
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_balance_is_identity() {
        let src = EffectImage::from_rgba8(2, 1, vec![40, 120, 200, 255, 10, 10, 10, 255]).unwrap();
        assert_eq!(
            ColorBalance::new([0.0; 3], [0.0; 3], [0.0; 3]).apply(&src),
            src
        );
    }

    #[test]
    fn shadow_red_reddens_dark_pixels() {
        // A dark grey (shadow range) pushed toward red gains red.
        let src = EffectImage::from_rgba8(1, 1, vec![30, 30, 30, 255]).unwrap();
        let out = ColorBalance::new([100.0, 0.0, 0.0], [0.0; 3], [0.0; 3]).apply(&src);
        assert!(out.data()[0] > 30, "red raised: {}", out.data()[0]);
    }

    #[test]
    fn highlight_shift_leaves_shadows_mostly_alone() {
        // A near-black pixel has ~zero highlight weight, so a highlight-only
        // shift barely moves it.
        let src = EffectImage::from_rgba8(1, 1, vec![0, 0, 0, 255]).unwrap();
        let out = ColorBalance::new([0.0; 3], [0.0; 3], [100.0, 0.0, 0.0]).apply(&src);
        assert_eq!(out.data()[0], 0);
    }
}
