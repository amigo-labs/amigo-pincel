//! Render effects (docs/specs/effects-render.md): paint.net's Render menu,
//! effects that generate an image instead of filtering one.
//!
//! Clouds and Turbulence paint seeded value noise between two colours and
//! combine it with the existing pixels by a [`RenderBlend`]; Julia Fractal
//! and Mandelbrot Fractal paint an escape-time rainbow, opaque everywhere.

mod fractal;
mod noise;

pub use fractal::{JuliaFractal, MandelbrotFractal};
pub use noise::{Clouds, Turbulence};

/// How a rendered colour combines with the existing pixel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RenderBlend {
    /// The rendered colour replaces the pixel, alpha included.
    #[default]
    Normal,
    /// Darkens: existing × rendered.
    Multiply,
    /// Lightens: the inverse of multiplying the inverses.
    Screen,
    /// Multiply on dark existing channels, Screen on light ones.
    Overlay,
}

impl RenderBlend {
    /// The blend for wire option `index` (`0` = normal, `1` = multiply,
    /// `2` = screen, `3` = overlay; anything else reads as normal).
    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Multiply,
            2 => Self::Screen,
            3 => Self::Overlay,
            _ => Self::Normal,
        }
    }

    /// Combines the straight-alpha `rendered` colour with the existing
    /// pixel `base`. Normal returns `rendered`; the others blend each RGB
    /// channel, mix the result into `base` by the rendered alpha and keep
    /// `base`'s alpha.
    pub(crate) fn combine(self, base: [u8; 4], rendered: [u8; 4]) -> [u8; 4] {
        let op: fn(f32, f32) -> f32 = match self {
            Self::Normal => return rendered,
            Self::Multiply => |b, r| b * r,
            Self::Screen => |b, r| 1.0 - (1.0 - b) * (1.0 - r),
            Self::Overlay => |b, r| {
                if b < 0.5 {
                    2.0 * b * r
                } else {
                    1.0 - 2.0 * (1.0 - b) * (1.0 - r)
                }
            },
        };
        let mix = f32::from(rendered[3]) / 255.0;
        let mut out = base;
        for ch in 0..3 {
            let b = f32::from(base[ch]) / 255.0;
            let blended = op(b, f32::from(rendered[ch]) / 255.0);
            let v = b + (blended - b) * mix;
            out[ch] = (v * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_normal_replaces_including_alpha() {
        let out = RenderBlend::Normal.combine([10, 20, 30, 255], [1, 2, 3, 0]);
        assert_eq!(out, [1, 2, 3, 0]);
    }

    #[test]
    fn combine_multiply_and_screen_with_black_and_white() {
        let base = [100, 150, 200, 77];
        assert_eq!(
            RenderBlend::Multiply.combine(base, [0, 0, 0, 255]),
            [0, 0, 0, 77]
        );
        assert_eq!(RenderBlend::Multiply.combine(base, [255; 4]), base);
        assert_eq!(
            RenderBlend::Screen.combine(base, [255; 4]),
            [255, 255, 255, 77]
        );
        assert_eq!(RenderBlend::Screen.combine(base, [0, 0, 0, 255]), base);
    }

    #[test]
    fn combine_transparent_rendered_colour_leaves_the_pixel() {
        for blend in [
            RenderBlend::Multiply,
            RenderBlend::Screen,
            RenderBlend::Overlay,
        ] {
            assert_eq!(
                blend.combine([9, 99, 199, 200], [0, 0, 0, 0]),
                [9, 99, 199, 200]
            );
        }
    }

    #[test]
    fn from_index_out_of_range_is_normal() {
        assert_eq!(RenderBlend::from_index(3), RenderBlend::Overlay);
        assert_eq!(RenderBlend::from_index(7), RenderBlend::Normal);
    }
}
