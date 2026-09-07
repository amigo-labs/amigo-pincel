//! Posterize (Fineliner spec §12.8): reduce each channel to N levels.

use super::map_rgb;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Quantizes each channel to `levels` evenly spaced values (2–255).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Posterize {
    /// Number of levels per channel, 2–255.
    pub levels: u32,
}

impl Posterize {
    /// Creates a posterize adjustment.
    pub fn new(levels: u32) -> Self {
        Self { levels }
    }
}

impl Effect for Posterize {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let n = self.levels.max(2) as f32;
        let q = |v: f32| (v * (n - 1.0)).round() / (n - 1.0);
        map_rgb(src, |[r, g, b]| [q(r), q(g), q(b)])
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn posterize_two_levels_snaps_to_black_or_white() {
        // With 2 levels the only outputs are 0 and 255.
        let src =
            EffectImage::from_rgba8(2, 1, vec![10, 130, 200, 255, 60, 128, 250, 255]).unwrap();
        let out = Posterize::new(2).apply(&src);
        for px in out.data().as_chunks::<4>().0 {
            for &c in &px[0..3] {
                assert!(c == 0 || c == 255, "got {c}");
            }
        }
    }

    #[test]
    fn posterize_preserves_endpoints() {
        // 0 and 255 map to themselves at any level count.
        let src = EffectImage::from_rgba8(1, 1, vec![0, 255, 0, 255]).unwrap();
        let out = Posterize::new(5).apply(&src);
        assert_eq!(&out.data()[0..3], &[0, 255, 0]);
    }
}
