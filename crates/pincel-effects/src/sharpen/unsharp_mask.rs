//! Unsharp Mask (Fineliner spec §11.2): amount, radius, threshold.

use crate::blur::GaussianBlur;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Unsharp Mask: adds a scaled high-pass (original − Gaussian-blurred) back onto
/// the image, but only where the local luminance change exceeds `threshold`.
///
/// - `amount` is a percentage (100 = 1.0×; spec range 1–500%).
/// - `radius` is the Gaussian low-pass radius in pixels (0.1–250).
/// - `threshold` is 0–255; higher values leave flat/low-contrast areas alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnsharpMask {
    /// Sharpening strength as a percentage (100 = 1.0×).
    pub amount: f32,
    /// Gaussian low-pass radius in pixels.
    pub radius: f32,
    /// Minimum luminance change (0–255) before a pixel is sharpened.
    pub threshold: f32,
}

impl UnsharpMask {
    /// Creates an Unsharp Mask.
    pub fn new(amount: f32, radius: f32, threshold: f32) -> Self {
        Self {
            amount,
            radius,
            threshold,
        }
    }
}

/// Rec.601 luma of a premultiplied RGB triple.
fn luma(px: &[f32]) -> f32 {
    0.299 * px[0] + 0.587 * px[1] + 0.114 * px[2]
}

impl Effect for UnsharpMask {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let amount = self.amount / 100.0;
        if amount <= 0.0 {
            return src.clone();
        }
        let src_pm = src.to_premultiplied_f32();
        let low_pm = GaussianBlur::new(self.radius)
            .apply(src)
            .to_premultiplied_f32();
        let threshold = self.threshold / 255.0;
        let mut out = src_pm.clone();
        for (o, low) in out
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(low_pm.as_chunks::<4>().0)
        {
            // Gate on the luminance difference so flat areas / noise stay put.
            if (luma(o) - luma(low)).abs() < threshold {
                continue;
            }
            // Add the scaled high-pass to RGB; keep the original alpha.
            for c in 0..3 {
                o[c] += amount * (o[c] - low[c]);
            }
        }
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &out)
    }

    fn scaled(&self, factor: f32) -> Self {
        // Only the radius is spatial; amount and threshold are scale-free.
        Self {
            amount: self.amount,
            radius: self.radius * factor,
            threshold: self.threshold,
        }
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
    fn unsharp_amount_zero_is_identity() {
        let src = gray_columns(&[80, 120, 160, 200, 200, 160], 4);
        assert_eq!(UnsharpMask::new(0.0, 3.0, 0.0).apply(&src), src);
    }

    #[test]
    fn unsharp_of_solid_color_is_unchanged() {
        let src = gray_columns(&[90, 90, 90, 90, 90], 5);
        assert_eq!(UnsharpMask::new(150.0, 2.0, 0.0).apply(&src), src);
    }

    #[test]
    fn unsharp_increases_edge_contrast() {
        // A soft ramp: unsharp should widen the light/dark spread at the edge.
        let src = gray_columns(&[100, 110, 140, 150], 4);
        let out = UnsharpMask::new(200.0, 2.0, 0.0).apply(&src);
        let orig = src.data();
        let sharp = out.data();
        let span = |d: &[u8]| {
            let vals: Vec<i32> = (0..4).map(|x| d[(4 + x) * 4] as i32).collect();
            vals.iter().max().unwrap() - vals.iter().min().unwrap()
        };
        assert!(
            span(sharp) > span(orig),
            "unsharp widened the edge span: {} vs {}",
            span(sharp),
            span(orig)
        );
    }

    #[test]
    fn high_threshold_suppresses_sharpening() {
        // With the threshold above every local change, unsharp is a no-op.
        let src = gray_columns(&[100, 110, 140, 150], 4);
        assert_eq!(UnsharpMask::new(200.0, 2.0, 255.0).apply(&src), src);
    }

    #[test]
    fn scaled_multiplies_radius_only() {
        let s = UnsharpMask::new(150.0, 20.0, 10.0).scaled(0.5);
        assert_eq!((s.amount, s.radius, s.threshold), (150.0, 10.0, 10.0));
    }
}
