//! Brightness / Contrast (Fineliner spec §12.1): −150..+150 each, Legacy or Enhanced.

use super::map_rgb;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Brightness and contrast adjustment.
///
/// `brightness` and `contrast` are both −150..+150 (0 = no change). In `Legacy`
/// mode contrast is a linear scale about mid-grey; in `Enhanced` mode it blends
/// toward an S-curve for gentler highlights/shadows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrightnessContrast {
    /// Brightness, −150..+150.
    pub brightness: f32,
    /// Contrast, −150..+150.
    pub contrast: f32,
    /// Enhanced (S-curve) contrast instead of Legacy (linear).
    pub enhanced: bool,
}

impl BrightnessContrast {
    /// Creates a Legacy (linear) brightness/contrast adjustment.
    pub fn new(brightness: f32, contrast: f32) -> Self {
        Self {
            brightness,
            contrast,
            enhanced: false,
        }
    }

    /// Switches to Enhanced (S-curve) contrast.
    pub fn enhanced(mut self) -> Self {
        self.enhanced = true;
        self
    }
}

impl Effect for BrightnessContrast {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let bright = self.brightness / 255.0;
        if self.enhanced {
            let s = self.contrast / 150.0;
            map_rgb(src, |rgb| {
                rgb.map(|v| {
                    let curve = v * v * (3.0 - 2.0 * v); // smoothstep S-curve
                    v + s * (curve - v) + bright
                })
            })
        } else {
            let factor = 1.0 + self.contrast / 150.0;
            map_rgb(src, |rgb| rgb.map(|v| (v - 0.5) * factor + 0.5 + bright))
        }
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(c: [u8; 4]) -> EffectImage {
        EffectImage::from_rgba8(1, 1, c.to_vec()).unwrap()
    }

    #[test]
    fn zero_is_identity_legacy() {
        let src = pixel([40, 120, 200, 255]);
        assert_eq!(BrightnessContrast::new(0.0, 0.0).apply(&src), src);
    }

    #[test]
    fn zero_is_identity_enhanced() {
        let src = pixel([40, 120, 200, 255]);
        assert_eq!(
            BrightnessContrast::new(0.0, 0.0).enhanced().apply(&src),
            src
        );
    }

    #[test]
    fn positive_brightness_lightens() {
        let src = pixel([100, 100, 100, 255]);
        let out = BrightnessContrast::new(50.0, 0.0).apply(&src);
        assert!(out.data()[0] > 100);
    }

    #[test]
    fn positive_contrast_pushes_away_from_mid() {
        let dark = BrightnessContrast::new(0.0, 100.0).apply(&pixel([80, 80, 80, 255]));
        let light = BrightnessContrast::new(0.0, 100.0).apply(&pixel([180, 180, 180, 255]));
        assert!(dark.data()[0] < 80, "below mid gets darker");
        assert!(light.data()[0] > 180, "above mid gets lighter");
    }
}
