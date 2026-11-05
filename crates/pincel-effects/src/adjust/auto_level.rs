//! Auto-Level (docs/specs/adjustments-paintnet.md): stretches the luma range
//! of the target to the full 0–255 range.

use super::luma_x10000;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Maps the lowest luma L0 and the highest luma L1 among the opaque target
/// pixels to 0 and 255: every channel `c` becomes
/// `(c − L0) · 255 / (L1 − L0)`, clamped. No parameters; when the target
/// has no opaque pixel or only one luma, nothing changes. Alpha is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AutoLevel;

impl AutoLevel {
    /// Applies the stretch to the pixels for which `in_target(x, y)` holds,
    /// taking L0 and L1 from those pixels only; the rest stay as they are.
    pub fn apply_in(&self, src: &EffectImage, in_target: impl Fn(u32, u32) -> bool) -> EffectImage {
        let width = src.width().max(1);
        let pixel_at = |i: usize| (i as u32 % width, i as u32 / width);
        let px = src.data().as_chunks::<4>().0;
        let range = px
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                let (x, y) = pixel_at(*i);
                p[3] > 0 && in_target(x, y)
            })
            .map(|(_, p)| luma_x10000(p[0], p[1], p[2]))
            .fold(None, |acc: Option<(u32, u32)>, l| match acc {
                None => Some((l, l)),
                Some((lo, hi)) => Some((lo.min(l), hi.max(l))),
            });
        let Some((lo, hi)) = range.filter(|(lo, hi)| hi > lo) else {
            return src.clone();
        };
        let (lo, span) = (f64::from(lo) / 10_000.0, f64::from(hi - lo) / 10_000.0);
        let mut lut = [0u8; 256];
        for (i, v) in lut.iter_mut().enumerate() {
            *v = ((i as f64 - lo) * 255.0 / span).round().clamp(0.0, 255.0) as u8;
        }
        let mut out = src.data().to_vec();
        for (i, p) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = pixel_at(i);
            if in_target(x, y) {
                for c in &mut p[..3] {
                    *c = lut[usize::from(*c)];
                }
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }
}

impl Effect for AutoLevel {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        self.apply_in(src, |_, _| true)
    }

    fn scaled(&self, _factor: f32) -> Self {
        *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_level_maps_the_luma_range_to_full_scale() {
        let src = EffectImage::from_rgba8(2, 1, vec![50, 50, 50, 255, 150, 150, 150, 255]).unwrap();
        let out = AutoLevel.apply(&src);
        assert_eq!(out.data(), &[0, 0, 0, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn auto_level_outside_the_target_is_untouched() {
        let data = vec![50, 50, 50, 255, 150, 150, 150, 255, 90, 90, 90, 255];
        let src = EffectImage::from_rgba8(3, 1, data).unwrap();
        let out = AutoLevel.apply_in(&src, |x, _| x < 2);
        assert_eq!(&out.data()[8..], &[90, 90, 90, 255]);
    }
}
