//! Fixed one-step sharpen (Fineliner spec §11.2): a single 3×3 convolution, no parameters.

use crate::effect::Effect;
use crate::image::EffectImage;
use crate::kernel::convolve_3x3;

/// A parameterless sharpen: the classic 3×3 kernel
/// `[0 −1 0; −1 5 −1; 0 −1 0]` (weights sum to 1, so flat regions are
/// unchanged). Spec §11.2 defines it as Unsharp Mask with a fixed preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sharpen;

#[rustfmt::skip]
const SHARPEN_KERNEL: [f32; 9] = [
    0.0, -1.0,  0.0,
   -1.0,  5.0, -1.0,
    0.0, -1.0,  0.0,
];

impl Effect for Sharpen {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let buf = src.to_premultiplied_f32();
        let sharpened = convolve_3x3(&buf, w, h, &SHARPEN_KERNEL);
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &sharpened)
    }

    fn scaled(&self, _factor: f32) -> Self {
        // A fixed 3×3 kernel is scale-independent.
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
    fn sharpen_of_solid_color_is_unchanged() {
        let src = gray_columns(&[128, 128, 128, 128], 4);
        assert_eq!(Sharpen.apply(&src), src);
    }

    #[test]
    fn sharpen_overshoots_at_an_edge() {
        // A 100→150 step: the light side near the seam overshoots up, the dark
        // side undershoots down.
        let src = gray_columns(&[100, 100, 150, 150], 3);
        let out = Sharpen.apply(&src);
        let r = |x: usize, y: usize| out.data()[(y * 4 + x) * 4] as i32;
        assert!(r(2, 1) > 150, "light side overshoots (got {})", r(2, 1));
        assert!(r(1, 1) < 100, "dark side undershoots (got {})", r(1, 1));
    }
}
