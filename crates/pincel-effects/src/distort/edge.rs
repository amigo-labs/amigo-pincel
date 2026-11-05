//! Edge behaviour and bilinear resampling shared by the resampling
//! distortions (docs/specs/effects-distort.md: Bulge, Dents, Polar
//! Inversion, Tile Reflection, Twist).

use crate::blur::whole;
use crate::image::EffectImage;

/// What a resampling distortion reads where a mapped position falls outside
/// the layer. Travels on the wire as its index (`0` = Clamp … `3` =
/// Transparent).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EdgeBehavior {
    /// The nearest edge pixel.
    #[default]
    Clamp,
    /// The layer tiled: the opposite edge.
    Wrap,
    /// The layer mirrored at its edge.
    Reflect,
    /// Fully transparent.
    Transparent,
}

impl EdgeBehavior {
    /// Reads a wire index (non-finite reads as 0, then rounded and clamped
    /// to `0..=3`).
    pub fn from_param(v: f32) -> Self {
        match whole(v, 0, 3) {
            0 => Self::Clamp,
            1 => Self::Wrap,
            2 => Self::Reflect,
            _ => Self::Transparent,
        }
    }

    /// Maps a tap coordinate onto `0..n`.
    fn tap(self, i: i64, n: usize) -> usize {
        let n = n as i64;
        let m = match self {
            Self::Clamp | Self::Transparent => i.clamp(0, n - 1),
            Self::Wrap => i.rem_euclid(n),
            Self::Reflect => {
                let m = i.rem_euclid(2 * n);
                if m < n { m } else { 2 * n - 1 - m }
            }
        };
        m as usize
    }
}

/// Bilinearly samples a flat premultiplied RGBA f32 buffer at `(x, y)`,
/// reading taps outside the layer as `edge` says. `None` for a position
/// that reads nothing: outside the pixel area (`-0.5..w - 0.5`) under
/// [`EdgeBehavior::Transparent`] (inside it, taps clamp), or non-finite.
pub(crate) fn sample(
    buf: &[f32],
    w: usize,
    h: usize,
    x: f32,
    y: f32,
    edge: EdgeBehavior,
) -> Option<[f32; 4]> {
    let inside = x >= -0.5 && x < w as f32 - 0.5 && y >= -0.5 && y < h as f32 - 0.5;
    if !x.is_finite() || !y.is_finite() || (edge == EdgeBehavior::Transparent && !inside) {
        return None;
    }
    let (fx, fy) = (x.floor(), y.floor());
    let (wx, wy) = (x - fx, y - fy);
    let (ix, iy) = (fx as i64, fy as i64);
    let x0 = edge.tap(ix, w);
    let x1 = edge.tap(ix.saturating_add(1), w);
    let y0 = edge.tap(iy, h);
    let y1 = edge.tap(iy.saturating_add(1), h);
    let mut out = [0.0f32; 4];
    for (c, v) in out.iter_mut().enumerate() {
        let p00 = buf[(y0 * w + x0) * 4 + c];
        let p10 = buf[(y0 * w + x1) * 4 + c];
        let p01 = buf[(y1 * w + x0) * 4 + c];
        let p11 = buf[(y1 * w + x1) * 4 + c];
        let top = p00 + (p10 - p00) * wx;
        let bot = p01 + (p11 - p01) * wx;
        *v = top + (bot - top) * wy;
    }
    Some(out)
}

/// Where a resampling distortion reads output pixel `(x, y)` from.
pub(crate) enum Source {
    /// The pixel stays as it is (outside a Bulge or Twist circle).
    Keep,
    /// Read the source at this position.
    At(f32, f32),
}

/// Runs a resampling distortion: `map` names each output pixel's source
/// position; reads go through [`sample`] with `edge`. Kept pixels and
/// non-finite positions under a non-transparent edge keep their exact
/// bytes.
pub(crate) fn remap(
    src: &EffectImage,
    edge: EdgeBehavior,
    map: impl Fn(f32, f32) -> Source,
) -> EffectImage {
    let (w, h) = (src.width() as usize, src.height() as usize);
    if w == 0 || h == 0 {
        return src.clone();
    }
    let buf = src.to_premultiplied_f32();
    let mut out = vec![0.0f32; buf.len()];
    let mut keep = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let read = match map(x as f32, y as f32) {
                Source::Keep => None,
                Source::At(sx, sy) => match sample(&buf, w, h, sx, sy, edge) {
                    Some(px) => Some(px),
                    None if edge == EdgeBehavior::Transparent => Some([0.0; 4]),
                    None => None,
                },
            };
            match read {
                Some(px) => out[i * 4..i * 4 + 4].copy_from_slice(&px),
                None => keep[i] = true,
            }
        }
    }
    let mut data = EffectImage::from_premultiplied_f32(src.width(), src.height(), &out).into_raw();
    for (i, _) in keep.iter().enumerate().filter(|(_, k)| **k) {
        data[i * 4..i * 4 + 4].copy_from_slice(&src.data()[i * 4..i * 4 + 4]);
    }
    EffectImage::from_rgba8(src.width(), src.height(), data)
        .expect("same size as the source buffer")
}

/// The pixel position a centre fraction (`0..=1` of the image) names, as
/// Radial Blur reads it: `0.5` of an odd side is its middle pixel.
pub(crate) fn centre(fraction: f32, side: u32) -> f32 {
    fraction * side.saturating_sub(1) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Vec<f32> {
        // A 4 x 1 premultiplied row with values 0, 1, 2, 3 in every channel.
        (0..4).flat_map(|v| [v as f32; 4]).collect()
    }

    fn at(x: f32, edge: EdgeBehavior) -> Option<f32> {
        sample(&row(), 4, 1, x, 0.0, edge).map(|p| p[0])
    }

    #[test]
    fn sample_edge_behaviours_read_outside_positions() {
        use EdgeBehavior::*;
        assert_eq!(at(5.0, Clamp), Some(3.0));
        assert_eq!(at(5.0, Wrap), Some(1.0));
        assert_eq!(at(5.0, Reflect), Some(2.0));
        assert_eq!(at(5.0, Transparent), None);
        assert_eq!(at(-2.0, Clamp), Some(0.0));
        assert_eq!(at(-2.0, Wrap), Some(2.0));
        assert_eq!(at(-2.0, Reflect), Some(1.0));
    }

    #[test]
    fn sample_interpolates_inside_and_at_the_edge_of_the_pixel_area() {
        assert_eq!(at(1.5, EdgeBehavior::Clamp), Some(1.5));
        assert_eq!(at(3.4, EdgeBehavior::Transparent), Some(3.0));
        assert_eq!(at(3.6, EdgeBehavior::Transparent), None);
        assert_eq!(at(3.5, EdgeBehavior::Wrap), Some(1.5));
    }

    #[test]
    fn sample_non_finite_positions_count_as_outside() {
        assert_eq!(at(f32::NAN, EdgeBehavior::Transparent), None);
        assert_eq!(at(f32::INFINITY, EdgeBehavior::Clamp), None);
    }

    #[test]
    fn from_param_rounds_and_clamps_the_index() {
        assert_eq!(EdgeBehavior::from_param(1.4), EdgeBehavior::Wrap);
        assert_eq!(EdgeBehavior::from_param(9.0), EdgeBehavior::Transparent);
        assert_eq!(EdgeBehavior::from_param(f32::NAN), EdgeBehavior::Clamp);
    }
}
