//! Adjustments (Fineliner spec §12): per-pixel colour/tone transforms applied
//! destructively to a layer. Unlike the spatial effects, these have no
//! neighbourhood, so `scaled` returns the effect unchanged.
//!
//! All adjustments run in gamma (sRGB) space on straight alpha: RGB is mapped
//! per pixel and alpha is preserved (Fineliner spec §12).

mod brightness_contrast;
mod color_balance;
mod curves;
mod grayscale;
mod hue_sat;
mod invert;
mod levels;
mod posterize;
mod threshold;

pub use brightness_contrast::BrightnessContrast;
pub use color_balance::ColorBalance;
pub use curves::{CurveChannel, Curves};
pub use grayscale::{Grayscale, GrayscaleMethod};
pub use hue_sat::HueSaturation;
pub use invert::Invert;
pub use levels::{Levels, LevelsChannel};
pub use posterize::Posterize;
pub use threshold::Threshold;

use crate::image::EffectImage;

/// Maps every pixel's RGB through `f` (operating in `[0,1]`), clamps, and
/// preserves alpha.
pub(crate) fn map_rgb(src: &EffectImage, f: impl Fn([f32; 3]) -> [f32; 3]) -> EffectImage {
    let mut out = src.data().to_vec();
    for px in out.chunks_exact_mut(4) {
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

/// Rounds an f32 to a clamped RGBA8 byte.
fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}
