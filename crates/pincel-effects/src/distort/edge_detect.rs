//! Edge Detect (Fineliner spec §11.3): Sobel / Prewitt / Laplacian, amount.

use super::{gradient, luma_at};
use crate::effect::Effect;
use crate::image::EffectImage;

/// Edge-detection kernel family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeAlgorithm {
    /// Sobel gradient magnitude (centre tap 2).
    Sobel,
    /// Prewitt gradient magnitude (centre tap 1).
    Prewitt,
    /// Laplacian second derivative magnitude.
    Laplacian,
}

/// Edge Detect: greyscale edge magnitude of the luminance field. Alpha is
/// preserved. `amount` (0–100%) scales the edge brightness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeDetect {
    /// Which kernel family to use.
    pub algorithm: EdgeAlgorithm,
    /// Edge gain as a percentage (0–100).
    pub amount: f32,
}

impl EdgeDetect {
    /// Creates an edge-detect effect.
    pub fn new(algorithm: EdgeAlgorithm, amount: f32) -> Self {
        Self { algorithm, amount }
    }
}

impl Effect for EdgeDetect {
    fn apply(&self, src: &EffectImage) -> EffectImage {
        let (w, h) = (src.width() as usize, src.height() as usize);
        let data = src.data();
        let gain = self.amount / 100.0;
        let mut out = vec![0u8; data.len()];
        for y in 0..h {
            for x in 0..w {
                let (xi, yi) = (x as i32, y as i32);
                let mag = match self.algorithm {
                    EdgeAlgorithm::Sobel => {
                        let (gx, gy) = gradient(data, w, h, xi, yi, 2.0);
                        (gx * gx + gy * gy).sqrt()
                    }
                    EdgeAlgorithm::Prewitt => {
                        let (gx, gy) = gradient(data, w, h, xi, yi, 1.0);
                        (gx * gx + gy * gy).sqrt()
                    }
                    EdgeAlgorithm::Laplacian => {
                        let c = luma_at(data, w, h, xi, yi);
                        let lap = luma_at(data, w, h, xi - 1, yi)
                            + luma_at(data, w, h, xi + 1, yi)
                            + luma_at(data, w, h, xi, yi - 1)
                            + luma_at(data, w, h, xi, yi + 1)
                            - 4.0 * c;
                        (lap / 4.0).abs()
                    }
                };
                let g = (mag * gain * 255.0).clamp(0.0, 255.0).round() as u8;
                let o = (y * w + x) * 4;
                out[o] = g;
                out[o + 1] = g;
                out[o + 2] = g;
                out[o + 3] = data[o + 3];
            }
        }
        EffectImage::from_rgba8(src.width(), src.height(), out).expect("same dimensions")
    }

    fn scaled(&self, _factor: f32) -> Self {
        // 3×3 neighbourhood; independent of image scale.
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
    fn edge_detect_of_solid_is_black() {
        // No edges anywhere → all zero (alpha preserved).
        let src = gray_columns(&[128, 128, 128, 128], 4);
        for algo in [
            EdgeAlgorithm::Sobel,
            EdgeAlgorithm::Prewitt,
            EdgeAlgorithm::Laplacian,
        ] {
            let out = EdgeDetect::new(algo, 100.0).apply(&src);
            for px in out.data().as_chunks::<4>().0 {
                assert_eq!([px[0], px[1], px[2]], [0, 0, 0]);
                assert_eq!(px[3], 255);
            }
        }
    }

    #[test]
    fn edge_detect_lights_up_a_seam() {
        // A step edge produces bright pixels at the seam column.
        let src = gray_columns(&[0, 0, 255, 255], 3);
        let out = EdgeDetect::new(EdgeAlgorithm::Sobel, 100.0).apply(&src);
        let brightest = out
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0])
            .max()
            .unwrap();
        assert!(
            brightest > 100,
            "seam produced a bright edge ({})",
            brightest
        );
    }

    #[test]
    fn amount_zero_is_black() {
        let src = gray_columns(&[0, 255, 0, 255], 3);
        let out = EdgeDetect::new(EdgeAlgorithm::Sobel, 0.0).apply(&src);
        assert!(out.data().as_chunks::<4>().0.iter().all(|p| p[0] == 0));
    }
}
