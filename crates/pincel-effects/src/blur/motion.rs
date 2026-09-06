//! Motion blur (Fineliner spec §11.1): average along a line of a given distance and angle.

use super::sample_bilinear;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Motion blur averaging samples along a straight line.
///
/// The line is `distance` pixels long, oriented at `angle` degrees (0° = +x,
/// measured clockwise in image space), and centred on each pixel. A `distance`
/// of 1 is the identity. Valid UI ranges are 1–500 px and 0–360° (Fineliner spec §11.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionBlur {
    /// Line length in pixels.
    pub distance: f32,
    /// Line orientation in degrees.
    pub angle: f32,
}

impl MotionBlur {
    /// Creates a motion blur of the given distance (px) and angle (degrees).
    pub fn new(distance: f32, angle: f32) -> Self {
        Self { distance, angle }
    }
}

impl Effect for MotionBlur {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let samples = self.distance.round().max(1.0) as i32;
        if samples <= 1 {
            return src.clone();
        }
        let (w, h) = (src.width() as usize, src.height() as usize);
        let rad = self.angle.to_radians();
        let (dx, dy) = (rad.cos(), rad.sin());
        let half = (samples - 1) as f32 / 2.0;
        let buf = src.to_premultiplied_f32();
        let mut out = vec![0.0f32; buf.len()];
        let inv_n = 1.0 / samples as f32;
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for i in 0..samples {
                    let t = i as f32 - half;
                    let s = sample_bilinear(&buf, w, h, x as f32 + dx * t, y as f32 + dy * t);
                    for c in 0..4 {
                        acc[c] += s[c];
                    }
                }
                let o = (y * w + x) * 4;
                for c in 0..4 {
                    out[o + c] = acc[c] * inv_n;
                }
            }
        }
        EffectImage::from_premultiplied_f32(src.width(), src.height(), &out)
    }

    fn scaled(&self, factor: f32) -> Self {
        Self {
            distance: self.distance * factor,
            angle: self.angle,
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
    fn motion_blur_distance_one_is_identity() {
        let src = solid(4, 4, [10, 20, 30, 255]);
        assert_eq!(MotionBlur::new(1.0, 45.0).apply(&src), src);
    }

    #[test]
    fn motion_blur_of_solid_color_is_unchanged() {
        let src = solid(6, 6, [123, 45, 67, 255]);
        assert_eq!(MotionBlur::new(9.0, 30.0).apply(&src), src);
    }

    #[test]
    fn horizontal_motion_spreads_along_x_only() {
        // Single white pixel; a horizontal blur spreads left/right, not up/down.
        let mut data = vec![0u8; 7 * 7 * 4];
        let center = (3 * 7 + 3) * 4;
        data[center..center + 4].copy_from_slice(&[255, 255, 255, 255]);
        let src = EffectImage::from_rgba8(7, 7, data).unwrap();
        let out = MotionBlur::new(5.0, 0.0).apply(&src);
        let alpha_at = |x: usize, y: usize| out.data()[(y * 7 + x) * 4 + 3];
        assert!(alpha_at(1, 3) > 0, "spreads horizontally");
        assert!(alpha_at(5, 3) > 0, "spreads horizontally");
        assert_eq!(alpha_at(3, 1), 0, "does not spread vertically");
        assert_eq!(alpha_at(3, 5), 0, "does not spread vertically");
    }

    #[test]
    fn scaled_multiplies_distance_only() {
        let s = MotionBlur::new(20.0, 90.0).scaled(0.25);
        assert_eq!((s.distance, s.angle), (5.0, 90.0));
    }
}
