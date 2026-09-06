//! Property-based tests for compositing (spec §7.4): `compose` is deterministic
//! and idempotent — running it twice on identical input yields identical output.

use fineliner_core::color::BlendMode;
use fineliner_core::{compose, Color, ImageBuffer, Layer};
use proptest::prelude::*;

const MODES: [BlendMode; 12] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::Darken,
    BlendMode::Lighten,
    BlendMode::ColorDodge,
    BlendMode::ColorBurn,
    BlendMode::HardLight,
    BlendMode::SoftLight,
    BlendMode::Difference,
    BlendMode::Exclusion,
];

fn layer_from(rgba: [u8; 4], mode: BlendMode, opacity: f32) -> Layer {
    let mut buf = ImageBuffer::new_transparent(2, 2);
    let c = Color::rgba(rgba[0], rgba[1], rgba[2], rgba[3]);
    for y in 0..2 {
        for x in 0..2 {
            buf.set_pixel(x, y, c);
        }
    }
    Layer::from_pixels("l", buf)
        .with_blend_mode(mode)
        .with_opacity(opacity)
}

proptest! {
    #[test]
    fn compose_is_deterministic(
        bottom in any::<[u8; 4]>(),
        top in any::<[u8; 4]>(),
        mode_idx in 0usize..12,
        opacity in 0.0f32..=1.0,
    ) {
        let mode = MODES[mode_idx];
        let stack = || vec![
            layer_from(bottom, BlendMode::Normal, 1.0),
            layer_from(top, mode, opacity),
        ];
        let a = compose(&stack());
        let b = compose(&stack());
        prop_assert_eq!(a, b);
    }

    #[test]
    fn composite_alpha_never_below_backdrop(
        bottom_a in 0u8..=255,
        top in any::<[u8; 4]>(),
    ) {
        // Source-over never reduces coverage: αo >= αb.
        let layers = vec![
            layer_from([100, 100, 100, bottom_a], BlendMode::Normal, 1.0),
            layer_from(top, BlendMode::Normal, 1.0),
        ];
        let out = compose(&layers);
        let result_a = out.get_pixel(0, 0).unwrap().a;
        prop_assert!(result_a >= bottom_a);
    }
}
