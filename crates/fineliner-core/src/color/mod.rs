//! Color representation and blend modes.
//!
//! See `docs/specs/fineliner.md` §4. Pixel data is stored as straight-alpha
//! RGBA8; effects and blending convert to RGBA32f in `[0.0, 1.0]`.

use serde::{Deserialize, Serialize};

/// A straight-alpha RGBA color, 8 bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    /// Red channel, 0–255.
    pub r: u8,
    /// Green channel, 0–255.
    pub g: u8,
    /// Blue channel, 0–255.
    pub b: u8,
    /// Alpha channel, 0–255 (255 = opaque).
    pub a: u8,
}

impl Color {
    /// Opaque black `(0, 0, 0, 255)` — the default foreground color (spec §4.2).
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    /// Opaque white `(255, 255, 255, 255)` — the default background color (spec §4.2).
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };

    /// Fully transparent `(0, 0, 0, 0)`.
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Creates a color from RGBA8 components.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Whether `self` is within `tol` Euclidean RGBA8 distance of `other`.
    ///
    /// `tol == 0` requires an exact match. The shared tolerance test of the
    /// Fill tool and the Magic Wand (spec §9.2 / §9.3).
    pub fn within_tolerance(self, other: Color, tol: u8) -> bool {
        let dr = self.r as i32 - other.r as i32;
        let dg = self.g as i32 - other.g as i32;
        let db = self.b as i32 - other.b as i32;
        let da = self.a as i32 - other.a as i32;
        let dist2 = dr * dr + dg * dg + db * db + da * da;
        let tol = tol as i32;
        dist2 <= tol * tol
    }

    /// Converts to RGBA32f in `[0.0, 1.0]` (no premultiplication, no gamma change).
    pub fn to_rgba32f(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    /// Builds a color from RGBA32f, clamping to `[0.0, 1.0]` and rounding.
    pub fn from_rgba32f(c: [f32; 4]) -> Self {
        Self {
            r: (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            g: (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            b: (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            a: (c[3].clamp(0.0, 1.0) * 255.0).round() as u8,
        }
    }
}

/// Per-layer blend mode applied during compositing (spec §4.4).
///
/// Formulas reference the W3C Compositing and Blending specification:
/// <https://www.w3.org/TR/compositing-1/>.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BlendMode {
    /// Standard source-over compositing.
    #[default]
    Normal,
    /// `src * dst`.
    Multiply,
    /// `1 - (1 - src)(1 - dst)`.
    Screen,
    /// Hard Light with src/dst swapped.
    Overlay,
    /// `min(src, dst)`.
    Darken,
    /// `max(src, dst)`.
    Lighten,
    /// `dst / (1 - src)`.
    ColorDodge,
    /// `1 - (1 - dst) / src`.
    ColorBurn,
    /// Overlay with src/dst swapped.
    HardLight,
    /// Pegtop soft-light formula.
    SoftLight,
    /// `abs(src - dst)`.
    Difference,
    /// `src + dst - 2 * src * dst`.
    Exclusion,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_rgba32f_round_trip_preserves_value() {
        for c in [Color::BLACK, Color::WHITE, Color::rgba(12, 200, 7, 130)] {
            assert_eq!(Color::from_rgba32f(c.to_rgba32f()), c);
        }
    }

    #[test]
    fn from_rgba32f_clamps_out_of_range_input() {
        let c = Color::from_rgba32f([-1.0, 2.0, 0.5, 1.0]);
        assert_eq!(c, Color::rgba(0, 255, 128, 255));
    }

    #[test]
    fn blend_mode_default_is_normal() {
        assert_eq!(BlendMode::default(), BlendMode::Normal);
    }
}
