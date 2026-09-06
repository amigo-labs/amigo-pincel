//! Hue / Saturation / Lightness (Fineliner spec §12.2), with an optional colorize mode.

use super::map_rgb;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Adjusts hue, saturation and lightness in HSL space.
///
/// - `hue` −180..+180 degrees (rotates hue; sets it in colorize mode).
/// - `saturation` −100..+100.
/// - `lightness` −100..+100 (blends toward black/white).
/// - `colorize` replaces every pixel's hue with a single hue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HueSaturation {
    /// Hue shift in degrees.
    pub hue: f32,
    /// Saturation, −100..+100.
    pub saturation: f32,
    /// Lightness, −100..+100.
    pub lightness: f32,
    /// Colorize mode.
    pub colorize: bool,
}

impl HueSaturation {
    /// Creates a hue/saturation/lightness adjustment.
    pub fn new(hue: f32, saturation: f32, lightness: f32) -> Self {
        Self {
            hue,
            saturation,
            lightness,
            colorize: false,
        }
    }

    /// Enables colorize mode.
    pub fn colorize(mut self) -> Self {
        self.colorize = true;
        self
    }
}

/// Applies a signed lightness slider (−1..1) to an HSL lightness.
fn shift_lightness(l: f32, amount: f32) -> f32 {
    if amount >= 0.0 {
        l + (1.0 - l) * amount
    } else {
        l * (1.0 + amount)
    }
}

impl Effect for HueSaturation {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let hue_shift = self.hue / 360.0;
        let sat = self.saturation / 100.0;
        let light = self.lightness / 100.0;
        map_rgb(src, |rgb| {
            let (h, s, l) = rgb_to_hsl(rgb);
            let (nh, ns) = if self.colorize {
                // Single hue; saturation slider maps −100..100 → 0..1.
                (hue_shift.rem_euclid(1.0), (sat + 1.0).clamp(0.0, 1.0) * 0.5)
            } else {
                (
                    (h + hue_shift).rem_euclid(1.0),
                    (s * (1.0 + sat)).clamp(0.0, 1.0),
                )
            };
            hsl_to_rgb(nh, ns, shift_lightness(l, light).clamp(0.0, 1.0))
        })
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

/// RGB (each `[0,1]`) → HSL (each `[0,1]`).
fn rgb_to_hsl([r, g, b]: [f32; 3]) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

/// HSL (each `[0,1]`) → RGB (each `[0,1]`).
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    if s <= 0.0 {
        return [l, l, l];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    [
        hue_to_channel(p, q, h + 1.0 / 3.0),
        hue_to_channel(p, q, h),
        hue_to_channel(p, q, h - 1.0 / 3.0),
    ]
}

fn hue_to_channel(p: f32, q: f32, t: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_on_gray_is_unchanged() {
        let src = EffectImage::from_rgba8(1, 1, vec![120, 120, 120, 255]).unwrap();
        assert_eq!(HueSaturation::new(0.0, 0.0, 0.0).apply(&src), src);
    }

    #[test]
    fn full_desaturation_makes_gray() {
        let src = EffectImage::from_rgba8(1, 1, vec![200, 80, 40, 255]).unwrap();
        let out = HueSaturation::new(0.0, -100.0, 0.0).apply(&src);
        let px = out.data();
        assert_eq!(px[0], px[1]);
        assert_eq!(px[1], px[2]);
    }

    #[test]
    fn hue_shift_changes_a_colored_pixel() {
        let src = EffectImage::from_rgba8(1, 1, vec![200, 80, 40, 255]).unwrap();
        let out = HueSaturation::new(120.0, 0.0, 0.0).apply(&src);
        assert_ne!(out.data(), src.data());
    }
}
