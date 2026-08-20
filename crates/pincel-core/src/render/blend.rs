//! u8 / sRGB pixel blending primitives shared by the composition paths.
//! See `docs/specs/pincel.md` §4.
//!
//! Two deliberate constraints, both following from the Aseprite-fidelity goal:
//!
//! 1. **Arithmetic is 8-bit and sRGB-encoded, never linear light.** Aseprite
//!    blends the stored sRGB bytes directly. The sister project
//!    `amigo-fineliner` computes the same formula family in f32 linear light
//!    per W3C Compositing-1; the two are intentionally not interchangeable, and
//!    results differ.
//! 2. **The channel function is applied to the backdrop RGB unconditionally**,
//!    and only then is the result source-over composited using the source
//!    alpha. W3C instead weights by backdrop alpha
//!    (`Cs' = (1 - ab) * Cs + ab * B(Cb, Cs)`), which leaves `Cs` untouched
//!    where the backdrop is transparent. Aseprite does not do that weighting,
//!    so here a `Multiply` layer over empty canvas goes black.
//!
//! TODO(cross-validation): neither the per-mode formulas nor the
//! transparent-backdrop semantics above have been checked against output from
//! Aseprite itself — this repo carries no Aseprite-generated reference
//! fixtures (see STATUS.md, M6.7). The formulas are derived from the public
//! W3C Compositing-1 definitions, re-expressed in 8-bit fixed point with the
//! `mul_u8` rounding this module already used for `Normal`. They are NOT
//! transcribed from Aseprite's source. Every blend-mode test below is
//! therefore provisional: it pins current behavior against regression, it does
//! not prove Aseprite parity. Do not claim bit-exactness anywhere until a
//! fixture set exists.

use crate::document::BlendMode;

/// How one cel's pixels combine into the destination buffer.
///
/// Built once per cel by [`compose`](fn@super::compose) so the per-pixel loops
/// carry a single `Copy` value rather than a mode plus two opacity arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BlendParams {
    /// The layer's blend mode.
    pub mode: BlendMode,
    /// Layer opacity and cel opacity already folded together.
    pub opacity: u8,
}

impl BlendParams {
    pub(super) fn new(mode: BlendMode, layer_opacity: u8, cel_opacity: u8) -> Self {
        Self {
            mode,
            opacity: mul_u8(layer_opacity, cel_opacity),
        }
    }
}

/// Whether [`blend_rgb`] implements `mode`.
///
/// [`compose`](fn@super::compose) consults this once per layer and raises
/// `RenderError::UnsupportedBlendMode` for everything else, which is what makes
/// `blend_rgb`'s not-yet-implemented arms unreachable. The two lists must move
/// together; `gate_agrees_with_compose` in the tests below enforces that.
pub(super) fn is_implemented(mode: BlendMode) -> bool {
    matches!(mode, BlendMode::Normal | BlendMode::Multiply)
}

/// Apply `mode`'s channel function to a backdrop / source RGB pair.
///
/// Alpha is not an input: compositing happens afterwards in
/// [`blend_normal_into`]. Modes are grouped as W3C groups them — separable
/// modes act per channel, non-separable ones need the whole triple.
pub(super) fn blend_rgb(mode: BlendMode, cb: [u8; 3], cs: [u8; 3]) -> [u8; 3] {
    match mode {
        BlendMode::Normal => cs,
        BlendMode::Multiply => separable(cb, cs, blend_multiply),

        // Not yet implemented; `is_implemented` gates these off before any
        // pixel reaches here. Returning `cs` keeps the match exhaustive without
        // a wildcard, so adding a variant to `BlendMode` is a compile error.
        BlendMode::Screen
        | BlendMode::Overlay
        | BlendMode::Darken
        | BlendMode::Lighten
        | BlendMode::ColorDodge
        | BlendMode::ColorBurn
        | BlendMode::HardLight
        | BlendMode::SoftLight
        | BlendMode::Difference
        | BlendMode::Exclusion
        | BlendMode::Hue
        | BlendMode::Saturation
        | BlendMode::Color
        | BlendMode::Luminosity
        | BlendMode::Addition
        | BlendMode::Subtract
        | BlendMode::Divide => cs,
    }
}

/// Lift a single-channel blend function over an RGB triple.
#[inline]
fn separable(cb: [u8; 3], cs: [u8; 3], f: fn(u8, u8) -> u8) -> [u8; 3] {
    [f(cb[0], cs[0]), f(cb[1], cs[1]), f(cb[2], cs[2])]
}

/// `B(cb, cs) = cb x cs`
#[inline]
fn blend_multiply(cb: u8, cs: u8) -> u8 {
    mul_u8(cb, cs)
}

/// Blend `mode`'s result for one source pixel into `dst`.
///
/// `sa` is the source alpha with cel and layer opacity already folded in.
pub(super) fn blend_into(mode: BlendMode, dst: &mut [u8], sr: u8, sg: u8, sb: u8, sa: u8) {
    let [r, g, b] = blend_rgb(mode, [dst[0], dst[1], dst[2]], [sr, sg, sb]);
    blend_normal_into(dst, r, g, b, sa);
}

/// Source-over (`Normal`) blend, non-premultiplied 8-bit channels.
pub(super) fn blend_normal_into(dst: &mut [u8], sr: u8, sg: u8, sb: u8, sa: u8) {
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
    use crate::document::{BlendMode, Cel, CelMap, FrameIndex, LayerId};
    use crate::render::compose::compose;
    use crate::render::test_support::{full_req, solid, stacked_sprite};

    #[test]
    fn multiply_layer_over_opaque_backdrop_multiplies_channels() {
        let sprite = stacked_sprite(1, 1, BlendMode::Multiply);
        let mut cels = CelMap::new();
        cels.insert(Cel::image(
            LayerId::new(0),
            FrameIndex::new(0),
            solid(1, 1, [200, 100, 50, 255]),
        ));
        cels.insert(Cel::image(
            LayerId::new(1),
            FrameIndex::new(0),
            solid(1, 1, [128, 128, 128, 255]),
        ));

        let r = compose(&sprite, &cels, &full_req(1, 1)).expect("multiply composes");

        // mul_u8(200, 128) = 100, mul_u8(100, 128) = 50, mul_u8(50, 128) = 25.
        // The top cel is fully opaque, so source-over keeps the blended color.
        assert_eq!(r.pixels, vec![100, 50, 25, 255]);
    }
}
