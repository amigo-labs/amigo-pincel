//! Pixel blending shared by the image and tilemap composition paths.
//!
//! All separable blend modes render via the W3C compositing formula; the
//! four non-separable HSL modes fall back to `Normal` (spec §15 Decision
//! Log, 2026-07-09 — see [`blend_pixel_into`]). See
//! `docs/specs/pincel.md` §4.

use crate::document::BlendMode;

/// Blend one source pixel into `dst` under the layer's blend mode.
///
/// `Normal` takes the dedicated fast path. The four non-separable HSL modes
/// (Hue / Saturation / Color / Luminosity) render as Normal in Phase 1 —
/// see the spec §15 Decision Log (2026-07-09). Everything else goes through
/// the W3C separable-blend compositing formula.
pub(super) fn blend_pixel_into(mode: BlendMode, dst: &mut [u8], sr: u8, sg: u8, sb: u8, sa: u8) {
    match mode {
        BlendMode::Normal
        | BlendMode::Hue
        | BlendMode::Saturation
        | BlendMode::Color
        | BlendMode::Luminosity => blend_normal_into(dst, sr, sg, sb, sa),
        _ => blend_separable_into(mode, dst, sr, sg, sb, sa),
    }
}

/// W3C compositing for separable blend modes, non-premultiplied 8-bit
/// channels: with backdrop `(Cb, αb)` and source `(Cs, αs)`,
///
/// ```text
/// αo    = αs + αb·(1 − αs)
/// co·αo = αs·(1 − αb)·Cs + αs·αb·B(Cb, Cs) + (1 − αs)·αb·Cb
/// ```
///
/// so a blend over a fully transparent backdrop leaves the source unchanged
/// and a fully transparent source is a no-op. Integer math is our own
/// rounding, not Aseprite's fixed-point blender — pixel-exact parity with
/// Aseprite is out of scope (spec §15 Decision Log).
fn blend_separable_into(mode: BlendMode, dst: &mut [u8], sr: u8, sg: u8, sb: u8, sa: u8) {
    if sa == 0 {
        return;
    }
    let da = u32::from(dst[3]);
    let sa32 = u32::from(sa);
    let inv_sa = 255 - sa32;
    // αo scaled to 0..=255, rounded.
    let oa = sa32 + (da * inv_sa + 127) / 255;
    if oa == 0 {
        dst[3] = 0;
        return;
    }
    let src = [sr, sg, sb];
    for (i, &cs) in src.iter().enumerate() {
        let cb = dst[i];
        let b = u32::from(blend_channel(mode, cb, cs));
        // Numerator of co·αo scaled by 255²; fits u32 (≤ 255³).
        let num = sa32 * (255 - da) * u32::from(cs) + sa32 * da * b + inv_sa * da * u32::from(cb);
        let denom = 255 * oa;
        // The min guards the ±1 rounding of `oa` at the top of the range.
        dst[i] = ((num + denom / 2) / denom).min(255) as u8;
    }
    dst[3] = oa as u8;
}

/// The separable per-channel blend function `B(Cb, Cs)` for `mode`.
/// Formulas follow the W3C compositing-and-blending spec (plus Aseprite's
/// Addition / Subtract / Divide extensions), adapted to 8-bit integers.
fn blend_channel(mode: BlendMode, cb: u8, cs: u8) -> u8 {
    let b = u32::from(cb);
    let s = u32::from(cs);
    match mode {
        BlendMode::Multiply => mul_u8(cb, cs),
        BlendMode::Screen => screen(cb, cs),
        // Overlay(Cb, Cs) = HardLight(Cs, Cb).
        BlendMode::Overlay => hard_light(cs, cb),
        BlendMode::Darken => cb.min(cs),
        BlendMode::Lighten => cb.max(cs),
        BlendMode::ColorDodge => {
            if cb == 0 {
                0
            } else if cs == 255 {
                255
            } else {
                ((b * 255) / (255 - s)).min(255) as u8
            }
        }
        BlendMode::ColorBurn => {
            if cb == 255 {
                255
            } else if cs == 0 {
                0
            } else {
                255 - ((((255 - b) * 255) / s).min(255) as u8)
            }
        }
        BlendMode::HardLight => hard_light(cb, cs),
        BlendMode::SoftLight => soft_light(cb, cs),
        BlendMode::Difference => cb.abs_diff(cs),
        BlendMode::Exclusion => {
            // Cb + Cs − 2·Cb·Cs; saturating guards the rounding of the
            // product term at the extremes.
            ((b + s).saturating_sub((2 * b * s + 127) / 255)) as u8
        }
        BlendMode::Addition => (b + s).min(255) as u8,
        BlendMode::Subtract => cb.saturating_sub(cs),
        BlendMode::Divide => {
            if cb == 0 {
                0
            } else if cb >= cs {
                255
            } else {
                // cs > cb ≥ 1 here, so the divisor is non-zero.
                ((b * 255) / s) as u8
            }
        }
        // Handled by `blend_pixel_into` before dispatch; returning the
        // source channel keeps this arm equivalent to Normal.
        BlendMode::Normal
        | BlendMode::Hue
        | BlendMode::Saturation
        | BlendMode::Color
        | BlendMode::Luminosity => cs,
    }
}

/// `Screen(Cb, Cs) = Cb + Cs − Cb·Cs`.
fn screen(cb: u8, cs: u8) -> u8 {
    let b = u32::from(cb);
    let s = u32::from(cs);
    (b + s - (b * s + 127) / 255) as u8
}

/// `HardLight(Cb, Cs)`: `Multiply(Cb, 2·Cs)` for `Cs ≤ 0.5`, else
/// `Screen(Cb, 2·Cs − 1)`.
fn hard_light(cb: u8, cs: u8) -> u8 {
    if cs <= 127 {
        mul_u8(cb, cs * 2)
    } else {
        screen(cb, (2 * u16::from(cs) - 255) as u8)
    }
}

/// W3C `SoftLight`. The only non-integer blend function (it needs a square
/// root); computed in `f32` — precision is far beyond the 8-bit output.
fn soft_light(cb: u8, cs: u8) -> u8 {
    let b = f32::from(cb) / 255.0;
    let s = f32::from(cs) / 255.0;
    let r = if s <= 0.5 {
        b - (1.0 - 2.0 * s) * b * (1.0 - b)
    } else {
        let d = if b <= 0.25 {
            ((16.0 * b - 12.0) * b + 4.0) * b
        } else {
            b.sqrt()
        };
        b + (2.0 * s - 1.0) * (d - b)
    };
    (r.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Source-over (`Normal`) blend, non-premultiplied 8-bit channels.
fn blend_normal_into(dst: &mut [u8], sr: u8, sg: u8, sb: u8, sa: u8) {
    if sa == 0 {
        return;
    }
    if sa == 255 {
        dst[0] = sr;
        dst[1] = sg;
        dst[2] = sb;
        dst[3] = 255;
        return;
    }
    let dr = u32::from(dst[0]);
    let dg = u32::from(dst[1]);
    let db = u32::from(dst[2]);
    let da = u32::from(dst[3]);
    let sa32 = u32::from(sa);
    let inv = 255u32 - sa32;
    // αd · (1 − αs), rounded.
    let blend_a = (da * inv + 127) / 255;
    let oa = sa32 + blend_a;
    if oa == 0 {
        dst[3] = 0;
        return;
    }
    dst[0] = ((u32::from(sr) * sa32 + dr * blend_a) / oa) as u8;
    dst[1] = ((u32::from(sg) * sa32 + dg * blend_a) / oa) as u8;
    dst[2] = ((u32::from(sb) * sa32 + db * blend_a) / oa) as u8;
    dst[3] = oa as u8;
}

#[inline]
pub(super) fn mul_u8(a: u8, b: u8) -> u8 {
    (((u16::from(a)) * (u16::from(b)) + 127) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Cel, CelMap, Frame, FrameIndex, Layer, LayerId, Sprite};

    use super::super::test_support::{compose_owned, full_req, solid};

    /// 1×1 sprite: bottom layer with `bottom` pixels, top layer in `mode`
    /// with `top` pixels; returns the composed RGBA pixel.
    fn blend_compose(mode: BlendMode, bottom: [u8; 4], top: [u8; 4]) -> Vec<u8> {
        let mut fg = Layer::image(LayerId::new(1), "fg");
        fg.blend_mode = mode;
        let sprite = Sprite::builder(1, 1)
            .add_layer(Layer::image(LayerId::new(0), "bg"))
            .add_layer(fg)
            .add_frame(Frame::default())
            .build()
            .unwrap();
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, bottom),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, top),
        ));
        compose_owned(&sprite, &cels, &full_req(1, 1))
            .unwrap()
            .pixels
    }

    #[test]
    fn multiply_blends_channels() {
        // Opaque red over opaque gray: B = Cb·Cs per channel.
        let px = blend_compose(BlendMode::Multiply, [128, 128, 128, 255], [255, 0, 0, 255]);
        assert_eq!(px, vec![128, 0, 0, 255]);
    }

    #[test]
    fn screen_blends_channels() {
        // Screen red over gray: r = 128 + 255 − 128 = 255, g/b = 128.
        let px = blend_compose(BlendMode::Screen, [128, 128, 128, 255], [255, 0, 0, 255]);
        assert_eq!(px, vec![255, 128, 128, 255]);
    }

    #[test]
    fn overlay_matches_hardlight_swapped() {
        for (cb, cs) in [(0u8, 0u8), (30, 200), (127, 128), (200, 40), (255, 255)] {
            assert_eq!(
                blend_channel(BlendMode::Overlay, cb, cs),
                blend_channel(BlendMode::HardLight, cs, cb),
            );
        }
    }

    #[test]
    fn darken_takes_min_and_lighten_takes_max() {
        assert_eq!(blend_channel(BlendMode::Darken, 100, 200), 100);
        assert_eq!(blend_channel(BlendMode::Lighten, 100, 200), 200);
    }

    #[test]
    fn addition_saturates_at_255() {
        assert_eq!(blend_channel(BlendMode::Addition, 200, 100), 255);
        assert_eq!(blend_channel(BlendMode::Subtract, 100, 200), 0);
    }

    #[test]
    fn blend_with_translucent_source_matches_w3c_formula() {
        // 50%-alpha multiply red over opaque gray. Hand-derived from the
        // integer formula: αo = 255; red   num = 128·255·128 + 127·255·128
        // → co = 128; green/blue num = 127·255·128 → co = 64.
        let px = blend_compose(BlendMode::Multiply, [128, 128, 128, 255], [255, 0, 0, 128]);
        assert_eq!(px, vec![128, 64, 64, 255]);
    }

    #[test]
    fn blend_over_transparent_backdrop_equals_source() {
        // αb = 0 ⇒ result = source, for every separable mode and both an
        // opaque and a translucent source.
        let modes = [
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
            BlendMode::Addition,
            BlendMode::Subtract,
            BlendMode::Divide,
        ];
        for mode in modes {
            for sa in [255u8, 128] {
                let mut dst = [0u8, 0, 0, 0];
                blend_pixel_into(mode, &mut dst, 200, 100, 50, sa);
                assert_eq!(dst, [200, 100, 50, sa], "mode {mode:?} sa {sa}");
            }
        }
    }

    #[test]
    fn hsl_modes_fall_back_to_normal() {
        let normal = blend_compose(BlendMode::Normal, [128, 128, 128, 255], [255, 0, 0, 128]);
        for mode in [
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ] {
            assert_eq!(
                blend_compose(mode, [128, 128, 128, 255], [255, 0, 0, 128]),
                normal,
                "mode {mode:?}"
            );
        }
    }
}
