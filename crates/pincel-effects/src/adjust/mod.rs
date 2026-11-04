//! Adjustments (Fineliner spec §12): per-pixel colour/tone transforms applied
//! destructively to a layer. Unlike the spatial effects, these have no
//! neighbourhood, so `scaled` returns the effect unchanged.
//!
//! All adjustments run in gamma (sRGB) space on straight alpha: RGB is mapped
//! per pixel and alpha is preserved (Fineliner spec §12). Exceptions from
//! docs/specs/adjustments-paintnet.md: [`Exposure`] scales in linear light,
//! [`InvertAlpha`] changes only alpha, and [`AutoLevel`] reads the luma
//! range of its target before mapping.

mod auto_level;
mod brightness_contrast;
mod color_balance;
mod curves;
mod exposure;
mod grayscale;
mod highlights_shadows;
mod hue_sat;
mod invert;
mod invert_alpha;
mod levels;
mod posterize;
mod sepia;
mod temperature_tint;
mod threshold;

pub use auto_level::AutoLevel;
pub use brightness_contrast::BrightnessContrast;
pub use color_balance::ColorBalance;
pub use curves::{CurveChannel, Curves};
pub use exposure::Exposure;
pub use grayscale::{Grayscale, GrayscaleMethod};
pub use highlights_shadows::HighlightsShadows;
pub use hue_sat::HueSaturation;
pub use invert::Invert;
pub use invert_alpha::InvertAlpha;
pub use levels::{Levels, LevelsChannel};
pub use posterize::Posterize;
pub use sepia::Sepia;
pub use temperature_tint::TemperatureTint;
pub use threshold::Threshold;

use crate::image::EffectImage;

/// Maps every pixel's RGB through `f` (operating in `[0,1]`), clamps, and
/// preserves alpha.
pub(crate) fn map_rgb(src: &EffectImage, f: impl Fn([f32; 3]) -> [f32; 3]) -> EffectImage {
    let mut out = src.data().to_vec();
    for px in out.as_chunks_mut::<4>().0 {
        let rgb = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
        ];
        let o = f(rgb);
        px[0] = to_u8(o[0]);
        px[1] = to_u8(o[1]);
        px[2] = to_u8(o[2]);
        // px[3] (alpha) is left as the copied source value.
    }
    EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
}

/// The 256-entry identity LUT.
pub(crate) fn identity_lut() -> [u8; 256] {
    let mut lut = [0u8; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        *v = i as u8;
    }
    lut
}

/// Rec.601 luma of an RGB triple in `[0,1]`.
pub(crate) fn luma(rgb: [f32; 3]) -> f32 {
    0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2]
}

/// Rec.709 luma of 0–255 channels, times 10 000: `2126 R + 7152 G + 722 B`.
/// Integer, so pixels with the same luma compare equal exactly
/// (docs/specs/adjustments-paintnet.md, "Luma").
pub(crate) fn luma_x10000(r: u8, g: u8, b: u8) -> u32 {
    2126 * u32::from(r) + 7152 * u32::from(g) + 722 * u32::from(b)
}

/// The sRGB transfer function, inverted: an sRGB sample in `[0,1]` to
/// linear light.
pub(crate) fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The sRGB transfer function: linear light in `[0,1]` to an sRGB sample.
pub(crate) fn linear_to_srgb(l: f64) -> f64 {
    if l <= 0.003_130_8 {
        l * 12.92
    } else {
        1.055 * l.powf(1.0 / 2.4) - 0.055
    }
}

/// Rounds an f32 to a clamped RGBA8 byte.
fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}
