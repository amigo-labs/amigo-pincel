//! Box blur (Fineliner spec §11.1): separable uniform kernel, width × height (odd).

use super::convolve_axis;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Box blur averaging a `width` × `height` neighbourhood.
///
/// Both dimensions are forced odd (rounded up) so the kernel stays centred; a
/// 1 × 1 box is the identity. Valid UI range is 1–500 px per axis (Fineliner spec §11.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoxBlur {
    /// Horizontal box width in pixels.
    pub width: u32,
    /// Vertical box height in pixels.
    pub height: u32,
}

impl BoxBlur {
    /// Creates a box blur of the given width and height in pixels.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Builds a normalised uniform kernel, forcing `len` odd and ≥ 1.
fn box_kernel(len: u32) -> Vec<f32> {
    let len = (len.max(1) | 1) as usize;
    vec![1.0 / len as f32; len]
}

impl Effect for BoxBlur {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        if self.width <= 1 && self.height <= 1 {
            return src.clone();
        }
        let (w, h) = (src.width() as usize, src.height() as usize);
        let buf = src.to_premultiplied_f32();
        let horizontal = convolve_axis(&buf, w, h, &box_kernel(self.width), true);
        let blurred = convolve_axis(&horizontal, w, h, &box_kernel(self.height), false);
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &blurred)
    }

    fn scaled(&self, factor: f32) -> Self {
        let scale = |v: u32| ((v as f32 * factor).round() as u32).max(1);
        Self {
            width: scale(self.width),
            height: scale(self.height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> EffectImage {
        let data: Vec<u8> = std::iter::repeat_n(c, (w * h) as usize).flatten().collect();
        EffectImage::from_rgba8(w, h, data).unwrap()
    }

    #[test]
    fn box_blur_1x1_returns_identity() {
        let src = solid(4, 4, [10, 20, 30, 255]);
        assert_eq!(BoxBlur::new(1, 1).apply(&src), src);
    }

    #[test]
    fn box_blur_of_solid_color_is_unchanged() {
        let src = solid(6, 6, [77, 88, 99, 255]);
        assert_eq!(BoxBlur::new(5, 5).apply(&src), src);
    }

    #[test]
    fn box_blur_averages_a_two_tone_row() {
        // Left half black, right half white; a 3-wide box averages the seam.
        let mut data = Vec::new();
        for _ in 0..3 {
            data.extend_from_slice(&[0, 0, 0, 255]);
        }
        for _ in 0..3 {
            data.extend_from_slice(&[255, 255, 255, 255]);
        }
        let src = EffectImage::from_rgba8(6, 1, data).unwrap();
        let out = BoxBlur::new(3, 1).apply(&src);
        // The two pixels straddling the seam become mid-grey (~1/3 or 2/3).
        let r = |x: usize| out.data()[x * 4];
        assert_eq!(r(0), 0);
        assert!(r(2) > 0 && r(2) < 128);
        assert!(r(3) > 128 && r(3) < 255);
        assert_eq!(r(5), 255);
    }

    #[test]
    fn scaled_rounds_each_axis() {
        let s = BoxBlur::new(10, 4).scaled(0.5);
        assert_eq!((s.width, s.height), (5, 2));
    }
}
