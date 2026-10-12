//! Separable blend-mode functions in linear light (spec §4.4).
//!
//! Each function takes the backdrop channel `cb` and source channel `cs`, both
//! in linear `[0.0, 1.0]`, and returns the blended channel. Formulas follow the
//! W3C Compositing and Blending spec: <https://www.w3.org/TR/compositing-1/>.
//! Soft Light uses the Pegtop formula as required by the spec.

use crate::color::BlendMode;

/// Applies a blend mode to a single linear channel pair.
pub fn blend_channel(mode: BlendMode, cb: f32, cs: f32) -> f32 {
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => cs * cb,
        BlendMode::Screen => cs + cb - cs * cb,
        BlendMode::Overlay => hard_light(cs, cb),
        BlendMode::Darken => cs.min(cb),
        BlendMode::Lighten => cs.max(cb),
        BlendMode::ColorDodge => {
            if cb <= 0.0 {
                0.0
            } else if cs >= 1.0 {
                1.0
            } else {
                (cb / (1.0 - cs)).min(1.0)
            }
        }
        BlendMode::ColorBurn => {
            if cb >= 1.0 {
                1.0
            } else if cs <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - cb) / cs).min(1.0)
            }
        }
        BlendMode::HardLight => hard_light(cb, cs),
        BlendMode::SoftLight => (1.0 - 2.0 * cs) * cb * cb + 2.0 * cs * cb,
        BlendMode::Difference => (cs - cb).abs(),
        BlendMode::Exclusion => cs + cb - 2.0 * cs * cb,
    }
}

/// Hard Light with backdrop `cb` and source `cs` (also reused for Overlay).
fn hard_light(cb: f32, cs: f32) -> f32 {
    if cs <= 0.5 {
        2.0 * cs * cb
    } else {
        1.0 - 2.0 * (1.0 - cs) * (1.0 - cb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-6;

    #[test]
    fn normal_returns_source() {
        assert_eq!(blend_channel(BlendMode::Normal, 0.3, 0.7), 0.7);
    }

    #[test]
    fn multiply_with_white_is_identity_with_black_is_black() {
        assert!((blend_channel(BlendMode::Multiply, 0.4, 1.0) - 0.4).abs() < EPS);
        assert!((blend_channel(BlendMode::Multiply, 0.4, 0.0)).abs() < EPS);
    }

    #[test]
    fn screen_with_black_is_identity() {
        assert!((blend_channel(BlendMode::Screen, 0.4, 0.0) - 0.4).abs() < EPS);
        assert!((blend_channel(BlendMode::Screen, 0.4, 1.0) - 1.0).abs() < EPS);
    }

    #[test]
    fn difference_is_absolute_delta() {
        assert!((blend_channel(BlendMode::Difference, 0.7, 0.2) - 0.5).abs() < EPS);
    }

    #[test]
    fn exclusion_half_half_is_half() {
        assert!((blend_channel(BlendMode::Exclusion, 0.5, 0.5) - 0.5).abs() < EPS);
    }

    #[test]
    fn darken_and_lighten_pick_extremes() {
        assert_eq!(blend_channel(BlendMode::Darken, 0.2, 0.8), 0.2);
        assert_eq!(blend_channel(BlendMode::Lighten, 0.2, 0.8), 0.8);
    }

    #[test]
    fn color_dodge_and_burn_handle_endpoints() {
        assert_eq!(blend_channel(BlendMode::ColorDodge, 0.0, 0.9), 0.0);
        assert_eq!(blend_channel(BlendMode::ColorDodge, 0.5, 1.0), 1.0);
        assert_eq!(blend_channel(BlendMode::ColorBurn, 1.0, 0.1), 1.0);
        assert_eq!(blend_channel(BlendMode::ColorBurn, 0.5, 0.0), 0.0);
    }

    #[test]
    fn overlay_is_hard_light_with_swapped_operands() {
        let cb = 0.3;
        let cs = 0.6;
        assert!(
            (blend_channel(BlendMode::Overlay, cb, cs)
                - blend_channel(BlendMode::HardLight, cs, cb))
            .abs()
                < EPS
        );
    }

    #[test]
    fn soft_light_neutral_gray_source_is_near_identity() {
        // Pegtop at cs = 0.5 returns cb exactly.
        assert!((blend_channel(BlendMode::SoftLight, 0.42, 0.5) - 0.42).abs() < EPS);
    }
}
