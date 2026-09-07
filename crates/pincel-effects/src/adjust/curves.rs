//! Curves (Fineliner spec §12.3): a per-channel tone curve from control points,
//! interpolated with a monotone cubic spline and applied through a 256-LUT.

use super::identity_lut;
use crate::effect::Effect;
use crate::image::EffectImage;

/// Which channel a curve targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveChannel {
    /// Applies to R, G and B together.
    Composite,
    /// Red only.
    Red,
    /// Green only.
    Green,
    /// Blue only.
    Blue,
    /// Alpha only.
    Alpha,
}

/// A tone curve over one channel. `points` are `[x, y]` control points in
/// `[0,1]`; two points `(0,0),(1,1)` are the identity.
#[derive(Debug, Clone, PartialEq)]
pub struct Curves {
    /// Target channel.
    pub channel: CurveChannel,
    /// Control points, `[x, y]` each in `[0,1]`.
    pub points: Vec<[f32; 2]>,
}

impl Curves {
    /// Creates a curves adjustment over `channel` with the given control points.
    pub fn new(channel: CurveChannel, points: Vec<[f32; 2]>) -> Self {
        Self { channel, points }
    }
}

/// Builds a 256-entry LUT from control points via a monotone cubic spline
/// (Fritsch–Carlson), falling back to identity for fewer than two points.
fn build_lut(points: &[[f32; 2]]) -> [u8; 256] {
    if points.len() < 2 {
        return identity_lut();
    }
    let mut pts = points.to_vec();
    pts.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    let n = pts.len();

    // Secants and monotone tangents.
    let mut d = vec![0.0f32; n - 1];
    for i in 0..n - 1 {
        let dx = (pts[i + 1][0] - pts[i][0]).max(1e-6);
        d[i] = (pts[i + 1][1] - pts[i][1]) / dx;
    }
    let mut m = vec![0.0f32; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 {
            0.0
        } else {
            (d[i - 1] + d[i]) / 2.0
        };
    }
    for i in 0..n - 1 {
        if d[i].abs() < 1e-9 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let a = m[i] / d[i];
            let b = m[i + 1] / d[i];
            let sq = a * a + b * b;
            if sq > 9.0 {
                let tau = 3.0 / sq.sqrt();
                m[i] = tau * a * d[i];
                m[i + 1] = tau * b * d[i];
            }
        }
    }

    let eval = |x: f32| -> f32 {
        if x <= pts[0][0] {
            return pts[0][1];
        }
        if x >= pts[n - 1][0] {
            return pts[n - 1][1];
        }
        let mut j = 0;
        while j < n - 1 && x > pts[j + 1][0] {
            j += 1;
        }
        let h = (pts[j + 1][0] - pts[j][0]).max(1e-6);
        let t = (x - pts[j][0]) / h;
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        h00 * pts[j][1] + h10 * h * m[j] + h01 * pts[j + 1][1] + h11 * h * m[j + 1]
    };

    let mut lut = [0u8; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        let y = eval(i as f32 / 255.0).clamp(0.0, 1.0);
        *v = (y * 255.0).round() as u8;
    }
    lut
}

impl Effect for Curves {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let lut = build_lut(&self.points);
        let mut out = src.data().to_vec();
        for px in out.as_chunks_mut::<4>().0 {
            match self.channel {
                CurveChannel::Composite => {
                    px[0] = lut[px[0] as usize];
                    px[1] = lut[px[1] as usize];
                    px[2] = lut[px[2] as usize];
                }
                CurveChannel::Red => px[0] = lut[px[0] as usize],
                CurveChannel::Green => px[1] = lut[px[1] as usize],
                CurveChannel::Blue => px[2] = lut[px[2] as usize],
                CurveChannel::Alpha => px[3] = lut[px[3] as usize],
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_identity_is_a_no_op() {
        let src = EffectImage::from_rgba8(2, 1, vec![10, 90, 200, 255, 0, 128, 255, 60]).unwrap();
        let curve = Curves::new(CurveChannel::Composite, vec![[0.0, 0.0], [1.0, 1.0]]);
        assert_eq!(curve.apply(&src), src);
    }

    #[test]
    fn lut_endpoints_are_exact() {
        let lut = build_lut(&[[0.0, 0.0], [1.0, 1.0]]);
        assert_eq!(lut[0], 0);
        assert_eq!(lut[255], 255);
        assert_eq!(lut[128], 128);
    }

    #[test]
    fn raising_the_midpoint_brightens_midtones() {
        let lut = build_lut(&[[0.0, 0.0], [0.5, 0.75], [1.0, 1.0]]);
        assert!(lut[128] > 128, "midtone raised: {}", lut[128]);
        assert_eq!(lut[0], 0);
        assert_eq!(lut[255], 255);
    }

    #[test]
    fn red_channel_curve_leaves_others_untouched() {
        let src = EffectImage::from_rgba8(1, 1, vec![100, 100, 100, 255]).unwrap();
        let out = Curves::new(CurveChannel::Red, vec![[0.0, 0.0], [1.0, 0.0]]).apply(&src);
        assert_eq!(out.data(), &[0, 100, 100, 255]);
    }
}
