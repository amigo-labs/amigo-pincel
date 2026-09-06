//! Gaussian blur (Fineliner spec §11.1): separable Gaussian kernel, σ = radius / 3.

use super::convolve_axis;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Gaussian blur with a radius in pixels.
///
/// The kernel standard deviation is `radius / 3`, so a `radius` of 0 (σ = 0) is
/// the identity. Valid UI range is 0.1–250 px (Fineliner spec §11.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaussianBlur {
    /// Blur radius in pixels.
    pub radius: f32,
}

impl GaussianBlur {
    /// Creates a Gaussian blur of the given radius in pixels.
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }
}

/// Builds a normalised 1-D Gaussian kernel spanning ±3σ.
fn gaussian_kernel(sigma: f32) -> Vec<f32> {
    let radius = (sigma * 3.0).ceil().max(1.0) as i32;
    let two_sigma_sq = 2.0 * sigma * sigma;
    let mut kernel: Vec<f32> = (-radius..=radius)
        .map(|i| (-(i * i) as f32 / two_sigma_sq).exp())
        .collect();
    let sum: f32 = kernel.iter().sum();
    for w in &mut kernel {
        *w /= sum;
    }
    kernel
}

impl Effect for GaussianBlur {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let sigma = self.radius / 3.0;
        if sigma <= 0.0 {
            return src.clone();
        }
        let (w, h) = (src.width() as usize, src.height() as usize);
        let kernel = gaussian_kernel(sigma);
        let buf = src.to_premultiplied_f32();
        let horizontal = convolve_axis(&buf, w, h, &kernel, true);
        let blurred = convolve_axis(&horizontal, w, h, &kernel, false);
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &blurred)
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            radius: self.radius * factor,
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
    fn gaussian_blur_with_sigma_zero_returns_identity() {
        let src = solid(5, 5, [12, 34, 56, 255]);
        // radius 0 → σ 0 → identity.
        assert_eq!(GaussianBlur::new(0.0).apply(&src), src);
    }

    #[test]
    fn gaussian_blur_of_solid_color_is_unchanged() {
        let src = solid(8, 8, [200, 100, 50, 255]);
        assert_eq!(GaussianBlur::new(6.0).apply(&src), src);
    }

    #[test]
    fn gaussian_blur_spreads_a_single_pixel() {
        // One opaque white pixel at the centre of a transparent field.
        let mut data = vec![0u8; 5 * 5 * 4];
        let center = (2 * 5 + 2) * 4;
        data[center..center + 4].copy_from_slice(&[255, 255, 255, 255]);
        let src = EffectImage::from_rgba8(5, 5, data).unwrap();
        let out = GaussianBlur::new(3.0).apply(&src);
        // Neighbour picks up alpha; centre loses some.
        let alpha_at = |x: usize, y: usize| out.data()[(y * 5 + x) * 4 + 3];
        assert!(alpha_at(2, 2) < 255);
        assert!(alpha_at(1, 2) > 0);
        assert!(alpha_at(3, 2) > 0);
    }

    #[test]
    fn scaled_multiplies_radius() {
        assert_eq!(GaussianBlur::new(30.0).scaled(0.5).radius, 15.0);
    }
}
