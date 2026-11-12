//! Layer compositing (spec §6).

mod blend;
mod cache;

pub use blend::blend_channel;
pub use cache::RenderCache;

use crate::color::{BlendMode, Color};
use crate::document::tree::subtree_end;
use crate::document::{ImageBuffer, Layer};
use crate::geometry::Rect;
use std::sync::OnceLock;

/// Converts an sRGB-encoded channel in `[0, 1]` to linear light.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a linear-light channel in `[0, 1]` to sRGB-encoded.
fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Composites visible layers bottom-to-top into a single RGBA8 buffer (spec §6.1).
///
/// Each layer's opacity is applied to its alpha before blending; blend math is
/// performed in linear light per the W3C compositing model. Hidden layers and
/// fully transparent layers are skipped. The output is the size of the first
/// raster layer (all raster layers are canvas-sized); a slice without one
/// yields a 0×0 buffer.
///
/// Groups composite in isolation (spec §4.2, docs/specs/image-layer-groups.md):
/// a group's children blend into a transparent buffer of their own, which
/// then blends into the layers below with the group's opacity and blend
/// mode; a hidden group hides its whole subtree. A group whose entry is not
/// in `layers` counts as the root, so any run of whole subtrees composes.
pub fn compose(layers: &[Layer]) -> ImageBuffer {
    let Some((w, h)) = canvas_size(layers) else {
        return ImageBuffer::new_transparent(0, 0);
    };
    let mut out = ImageBuffer::new_transparent(w, h);
    composite_run(&mut out, layers, 0, 0);
    out
}

/// The size of the first raster layer, which every raster layer shares.
fn canvas_size(layers: &[Layer]) -> Option<(u32, u32)> {
    layers
        .iter()
        .find(|l| !l.is_group())
        .map(|l| (l.pixels.width(), l.pixels.height()))
}

/// Blends a run of whole subtrees onto `dst`, which covers the canvas from
/// `(ox, oy)`. Without groups this is exactly the flat bottom-to-top loop.
fn composite_run(dst: &mut ImageBuffer, layers: &[Layer], ox: u32, oy: u32) {
    let mut i = 0;
    while let Some(layer) = layers.get(i) {
        let end = subtree_end(layers, i);
        if layer.visible && layer.opacity > 0.0 {
            if layer.is_group() {
                let mut isolated = ImageBuffer::new_transparent(dst.width(), dst.height());
                composite_run(&mut isolated, &layers[i + 1..end], ox, oy);
                let folded = Layer {
                    pixels: isolated,
                    ..Layer::from_pixels(String::new(), ImageBuffer::new_transparent(0, 0))
                }
                .with_opacity(layer.opacity)
                .with_blend_mode(layer.blend_mode);
                composite_layer(dst, &folded, 0, 0);
            } else {
                composite_layer(dst, layer, ox, oy);
            }
        }
        i = end;
    }
}

/// Composites only `region` of the visible layers (spec §6.1), yielding a
/// buffer the size of `region` clipped to the canvas. The pixels are
/// exactly the matching sub-rect of [`compose`]; the cost scales with the
/// region, which is what lets the UI render just the visible part of a
/// large canvas. A region outside the canvas yields a 0×0 buffer.
pub fn compose_region(layers: &[Layer], region: Rect) -> ImageBuffer {
    let Some((cw, ch)) = canvas_size(layers) else {
        return ImageBuffer::new_transparent(0, 0);
    };
    let (cw, ch) = (i64::from(cw), i64::from(ch));
    let x0 = i64::from(region.x).clamp(0, cw);
    let y0 = i64::from(region.y).clamp(0, ch);
    let x1 = (i64::from(region.x) + i64::from(region.w)).clamp(0, cw);
    let y1 = (i64::from(region.y) + i64::from(region.h)).clamp(0, ch);
    let mut out = ImageBuffer::new_transparent((x1 - x0) as u32, (y1 - y0) as u32);
    composite_run(&mut out, layers, x0 as u32, y0 as u32);
    out
}

/// Composites visible layers over an opaque `background`, yielding a fully
/// opaque buffer (used by Flatten Image, spec §5.2).
///
/// Equivalent to painting [`compose`] over a solid `background`; the result's
/// alpha is always 255. The over-background blend is performed in linear light
/// to match [`compose`]. The output size matches the first layer (0×0 if empty).
pub fn compose_over(layers: &[Layer], background: Color) -> ImageBuffer {
    over_background(compose(layers), background)
}

/// Composites a straight-alpha buffer over an opaque `background` in linear
/// light, yielding a fully opaque buffer (Flatten Image).
pub(crate) fn over_background(mut out: ImageBuffer, background: Color) -> ImageBuffer {
    let over = OverBackground::new(background);
    for px in out.data_mut().as_chunks_mut::<4>().0 {
        let [r, g, b] = over.pixel(*px);
        *px = [r, g, b, 255];
    }
    out
}

/// The RGB bytes of `buf` composited over `background`, exactly the color
/// channels [`over_background`] gives, without copying `buf` first (the
/// JPEG encoder's input).
pub(crate) fn over_background_rgb(buf: &ImageBuffer, background: Color) -> Vec<u8> {
    let over = OverBackground::new(background);
    let mut rgb = Vec::with_capacity(buf.data().len() / 4 * 3);
    for px in buf.data().as_chunks::<4>().0 {
        rgb.extend_from_slice(&over.pixel(*px));
    }
    rgb
}

/// The over-background blend `sa * s + (1 - sa) * bg` in linear light,
/// operation for operation as the float path, with the sRGB decode, the
/// alpha division and the encode taken from the exact lookup tables.
struct OverBackground {
    bg_lin: [f32; 3],
    decode: &'static [f32; 256],
    alpha: &'static [f32; 256],
    encoder: &'static Encoder,
}

impl OverBackground {
    fn new(background: Color) -> Self {
        Self {
            bg_lin: [
                srgb_to_linear(background.r as f32 / 255.0),
                srgb_to_linear(background.g as f32 / 255.0),
                srgb_to_linear(background.b as f32 / 255.0),
            ],
            decode: decode_table(),
            alpha: alpha_table(),
            encoder: encoder(),
        }
    }

    #[inline]
    fn pixel(&self, [r, g, b, a]: [u8; 4]) -> [u8; 3] {
        let sa = self.alpha[usize::from(a)];
        let channel = |c: u8, bg: f32| {
            let co = sa * self.decode[usize::from(c)] + (1.0 - sa) * bg;
            self.encoder.encode(co)
        };
        [
            channel(r, self.bg_lin[0]),
            channel(g, self.bg_lin[1]),
            channel(b, self.bg_lin[2]),
        ]
    }
}

/// sRGB byte → linear light: exactly `srgb_to_linear(b / 255)` for each
/// byte, computed once (docs/specs/performance-image-kernel.md).
fn decode_table() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| srgb_to_linear(i as f32 / 255.0)))
}

/// Alpha byte → `a / 255`, exactly the division it replaces.
fn alpha_table() -> &'static [f32; 256] {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| std::array::from_fn(|i| i as f32 / 255.0))
}

/// Bits dropped from an `f32` to find its bucket in [`Encoder`]: buckets of
/// 65 536 neighbouring floats, fewer than the floats between two thresholds
/// anywhere in `(0, 1)`, so a bucket holds at most one threshold.
const BUCKET_SHIFT: u32 = 16;

/// Linear light → sRGB byte, exactly as
/// `(linear_to_srgb(c).clamp(0, 1) * 255).round()` but without `powf`.
/// The thresholds are the smallest floats that the float conversion rounds
/// up to each byte; a value's byte is the number of thresholds at or below
/// it. Floats order like their bit patterns, so the bucket of `c`'s high
/// bits gives the byte at the bucket's start and the one threshold that may
/// lie inside it.
struct Encoder {
    /// Per bucket: the byte at its first float.
    base: Vec<u8>,
    /// Per bucket: the threshold inside it, or `+∞` when there is none.
    next: Vec<f32>,
}

/// `thresholds[k]` (k ≥ 1): the smallest non-negative `f32` that the float
/// conversion encodes to `k` or more; `thresholds[0]` is unused.
fn encode_thresholds() -> [f32; 256] {
    let reference = |c: f32| (linear_to_srgb(c).clamp(0.0, 1.0) * 255.0).round() as u8;
    let one = 1.0f32.to_bits();
    let mut thresholds = [0.0f32; 256];
    for (k, t) in thresholds.iter_mut().enumerate().skip(1) {
        // Non-negative floats order like their bit patterns.
        let (mut lo, mut hi) = (0u32, one);
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if usize::from(reference(f32::from_bits(mid))) >= k {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        *t = f32::from_bits(hi);
    }
    thresholds
}

impl Encoder {
    fn build() -> Self {
        let one = 1.0f32.to_bits();
        let buckets = (one >> BUCKET_SHIFT) as usize;
        let mut base = vec![0u8; buckets];
        let mut next = vec![f32::INFINITY; buckets];
        for &t in &encode_thresholds()[1..] {
            let b = (t.to_bits() >> BUCKET_SHIFT) as usize;
            if b >= buckets {
                continue;
            }
            assert!(next[b].is_infinite(), "two thresholds in bucket {b}");
            next[b] = t;
            // Every later bucket starts at or above this threshold.
            for v in &mut base[b + 1..] {
                *v += 1;
            }
        }
        Self { base, next }
    }

    #[inline]
    fn encode(&self, c: f32) -> u8 {
        // NaN, negative and zero encode to 0, 1 and above to 255.
        if c.is_nan() || c <= 0.0 {
            return 0;
        }
        if c >= 1.0 {
            return 255;
        }
        let b = (c.to_bits() >> BUCKET_SHIFT) as usize;
        self.base[b] + u8::from(c >= self.next[b])
    }
}

fn encoder() -> &'static Encoder {
    static ENCODER: OnceLock<Encoder> = OnceLock::new();
    ENCODER.get_or_init(Encoder::build)
}

/// Blends one layer onto the accumulator `dst` in place. `dst` covers the
/// layer rect starting at `(ox, oy)`; pixels past the layer's edge are left
/// alone. The arithmetic is the W3C model in linear light, operation for
/// operation as before the lookup tables, so the bytes are identical
/// (docs/specs/performance-image-kernel.md AC-01).
fn composite_layer(dst: &mut ImageBuffer, layer: &Layer, ox: u32, oy: u32) {
    // One copy of the loop per mode, so the mode is decided once per layer
    // instead of once per channel.
    macro_rules! per_mode {
        ($($mode:ident),*) => {
            match layer.blend_mode {
                $(BlendMode::$mode => composite_rows(
                    dst,
                    layer,
                    ox,
                    oy,
                    matches!(BlendMode::$mode, BlendMode::Normal) && normal_opaque_copies(),
                    |cb, cs| blend_channel(BlendMode::$mode, cb, cs),
                ),)*
            }
        };
    }
    per_mode!(
        Normal, Multiply, Screen, Overlay, Darken, Lighten, ColorDodge, ColorBurn, HardLight,
        SoftLight, Difference, Exclusion
    );
}

/// `true` when an opaque Normal source (`sa == 1`) encodes to its own bytes
/// over every backdrop: then `ao` is 1, the backdrop term is multiplied by
/// 0, and each channel is `encode((1 − da)·Cs + da·Cs)`, which depends only
/// on the source byte and the backdrop alpha. All 65 536 pairs are checked
/// once; only if each gives the source byte back does the kernel copy.
fn normal_opaque_copies() -> bool {
    static COPIES: OnceLock<bool> = OnceLock::new();
    *COPIES.get_or_init(|| {
        let (lin, alpha, enc) = (decode_table(), alpha_table(), encoder());
        (0..=255u8).all(|c| {
            let sl = lin[usize::from(c)];
            alpha.iter().all(|&da| {
                let blended = (1.0 - da) * sl + da * blend_channel(BlendMode::Normal, sl, sl);
                enc.encode(blended) == c
            })
        })
    })
}

#[inline(always)]
fn composite_rows<F: Fn(f32, f32) -> f32>(
    dst: &mut ImageBuffer,
    layer: &Layer,
    ox: u32,
    oy: u32,
    opaque_copies: bool,
    blend: F,
) {
    let (w, h) = (dst.width() as usize, dst.height() as usize);
    let (lw, lh) = (
        layer.pixels.width() as usize,
        layer.pixels.height() as usize,
    );
    let (ox, oy) = (ox as usize, oy as usize);
    if ox >= lw || oy >= lh {
        return;
    }
    let cols = w.min(lw - ox);
    let rows = h.min(lh - oy);
    let opacity = layer.opacity.clamp(0.0, 1.0);
    let lin = decode_table();
    let alpha = alpha_table();
    let enc = encoder();
    let src = layer.pixels.data();
    let out = dst.data_mut();

    for y in 0..rows {
        let start = ((y + oy) * lw + ox) * 4;
        let srow = &src[start..start + cols * 4];
        let drow = &mut out[y * w * 4..(y * w + cols) * 4];
        let (spx, _) = srow.as_chunks::<4>();
        let (dpx, _) = drow.as_chunks_mut::<4>();
        for (s, d) in spx.iter().zip(dpx.iter_mut()) {
            let sa = alpha[usize::from(s[3])] * opacity;
            if sa <= 0.0 {
                continue;
            }
            if opaque_copies && sa == 1.0 {
                d.copy_from_slice(s);
                continue;
            }
            let da = alpha[usize::from(d[3])];
            let ao = sa + da * (1.0 - sa);
            let mut rgb = [0u8; 3];
            if ao > 0.0 {
                for i in 0..3 {
                    let sl = lin[usize::from(s[i])];
                    let dl = lin[usize::from(d[i])];
                    // Blended source color (W3C): (1-da)*Cs + da*B(Cb,Cs).
                    let blended = (1.0 - da) * sl + da * blend(dl, sl);
                    // Source-over with the blended source, premultiplied.
                    let co = sa * blended + da * dl * (1.0 - sa);
                    // `co / 1.0` is `co` exactly: skip the division over an
                    // opaque result.
                    rgb[i] = enc.encode(if ao == 1.0 { co } else { co / ao });
                }
            }
            d[..3].copy_from_slice(&rgb);
            d[3] = round_byte(ao.clamp(0.0, 1.0) * 255.0);
        }
    }
}

/// `x.round() as u8` for `x` in `0 ..= 255` (or NaN), without the `roundf`
/// call that `f32::round` becomes on baseline x86-64: the integer part and
/// the fraction are exact below 2²⁴, and a fraction of one half rounds up,
/// as `round` does.
#[inline(always)]
fn round_byte(x: f32) -> u8 {
    let i = x as u32;
    let r = if x - i as f32 >= 0.5 { i + 1 } else { i };
    r as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::BlendMode;
    use proptest::prelude::*;

    fn solid_layer(w: u32, h: u32, c: Color) -> Layer {
        let mut buf = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                buf.set_pixel(x, y, c);
            }
        }
        Layer::from_pixels("l", buf)
    }

    /// A group entry with `children` placed into it (their `parent` set).
    fn grouped(group: Layer, children: Vec<Layer>) -> Vec<Layer> {
        let id = group.id;
        let mut out = vec![group];
        out.extend(children.into_iter().map(|c| {
            if c.parent.is_none() {
                c.with_parent(Some(id))
            } else {
                c
            }
        }));
        out
    }

    #[test]
    fn hidden_group_hides_children() {
        let bg = solid_layer(2, 2, Color::rgba(0, 0, 255, 255));
        let child = solid_layer(2, 2, Color::rgba(255, 0, 0, 255));
        let mut layers = vec![bg.clone()];
        layers.extend(grouped(
            Layer::new_group("G").with_visible(false),
            vec![child],
        ));
        assert_eq!(compose(&layers), compose(std::slice::from_ref(&bg)));
        // The child's own flag is untouched; showing the group brings it back.
        assert!(layers[2].visible);
        layers[1].visible = true;
        assert_eq!(
            compose(&layers).get_pixel(0, 0),
            Some(Color::rgba(255, 0, 0, 255))
        );
    }

    #[test]
    fn group_opacity_applies_once_to_composite_not_per_child() {
        let a = solid_layer(1, 1, Color::rgba(255, 0, 0, 255));
        let b = solid_layer(1, 1, Color::rgba(255, 0, 0, 255));
        let layers = grouped(Layer::new_group("G").with_opacity(0.5), vec![a, b]);
        let out = compose(&layers);
        assert_eq!((out.width(), out.height()), (1, 1));
        // 50 % coverage (128), not the 75 % (191) of two 50 % layers.
        assert_eq!(out.get_pixel(0, 0).map(|c| c.a), Some(128));
    }

    #[test]
    fn group_children_blend_in_isolation_from_layers_below() {
        let bg = solid_layer(1, 1, Color::rgba(0, 0, 255, 255));
        let child =
            solid_layer(1, 1, Color::rgba(255, 0, 0, 255)).with_blend_mode(BlendMode::Multiply);
        let mut layers = vec![bg];
        layers.extend(grouped(Layer::new_group("G"), vec![child]));
        // Inside the group the Multiply child starts from transparent, so it
        // lands as plain red over the blue background instead of black.
        assert_eq!(
            compose(&layers).get_pixel(0, 0),
            Some(Color::rgba(255, 0, 0, 255))
        );
    }

    #[test]
    fn group_blend_mode_blends_composite() {
        let bg = solid_layer(1, 1, Color::rgba(255, 255, 0, 255));
        let child = solid_layer(1, 1, Color::rgba(255, 0, 255, 255));
        let mut grouped_layers = vec![bg.clone()];
        grouped_layers.extend(grouped(
            Layer::new_group("G").with_blend_mode(BlendMode::Multiply),
            vec![child.clone()],
        ));
        let flat = [bg, child.with_blend_mode(BlendMode::Multiply)];
        // Yellow × magenta = red, exactly as a Multiply layer gives.
        assert_eq!(compose(&grouped_layers), compose(&flat));
        assert_eq!(
            compose(&grouped_layers).get_pixel(0, 0),
            Some(Color::rgba(255, 0, 0, 255))
        );
    }

    #[test]
    fn nested_group_opacities_multiply_level_by_level() {
        let child = solid_layer(1, 1, Color::rgba(0, 0, 0, 255));
        let inner = grouped(Layer::new_group("H").with_opacity(0.5), vec![child]);
        let mut layers = grouped(Layer::new_group("G").with_opacity(0.5), vec![]);
        let gid = layers[0].id;
        let mut inner = inner;
        inner[0].parent = Some(gid);
        layers.extend(inner);
        // 0.5 × 0.5 of an opaque pixel: 25 % coverage.
        assert_eq!(compose(&layers).get_pixel(0, 0).map(|c| c.a), Some(64));
    }

    #[test]
    fn compose_region_with_groups_matches_the_sub_rect_of_compose() {
        let mut a = ImageBuffer::new_transparent(4, 3);
        for y in 0..3 {
            for x in 0..4 {
                a.set_pixel(x, y, Color::rgba(x as u8 * 60, 90, y as u8 * 80, 200));
            }
        }
        let bg = solid_layer(4, 3, Color::rgba(20, 200, 40, 255));
        let mut layers = vec![Layer::new_group("empty"), bg];
        layers.extend(grouped(
            Layer::new_group("G")
                .with_opacity(0.6)
                .with_blend_mode(BlendMode::Screen),
            vec![Layer::from_pixels("a", a).with_blend_mode(BlendMode::Overlay)],
        ));
        let full = compose(&layers);
        assert_eq!((full.width(), full.height()), (4, 3));
        let region = compose_region(&layers, Rect::new(1, 1, 3, 2));
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(region.get_pixel(x, y), full.get_pixel(x + 1, y + 1));
            }
        }
    }

    #[test]
    fn compose_empty_is_zero_sized() {
        let out = compose(&[]);
        assert_eq!(out.width(), 0);
        assert_eq!(out.height(), 0);
    }

    #[test]
    fn compose_over_white_makes_result_opaque() {
        // A fully transparent layer over white yields opaque white everywhere.
        let layer = solid_layer(2, 2, Color::TRANSPARENT);
        let out = compose_over(std::slice::from_ref(&layer), Color::WHITE);
        assert_eq!(out.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn compose_over_opaque_layer_keeps_layer_color() {
        let layer = solid_layer(1, 1, Color::rgba(10, 200, 30, 255));
        let out = compose_over(std::slice::from_ref(&layer), Color::WHITE);
        assert_eq!(out.get_pixel(0, 0), Some(Color::rgba(10, 200, 30, 255)));
    }

    #[test]
    fn compose_region_matches_the_sub_rect_of_compose() {
        let mut bottom = ImageBuffer::new_transparent(5, 4);
        let mut top = ImageBuffer::new_transparent(5, 4);
        for y in 0..4 {
            for x in 0..5 {
                bottom.set_pixel(x, y, Color::rgba(x as u8 * 40, y as u8 * 50, 90, 255));
                top.set_pixel(x, y, Color::rgba(200, 10, x as u8 * 30, (y as u8 + 1) * 50));
            }
        }
        let layers = [
            Layer::from_pixels("b", bottom),
            Layer::from_pixels("t", top).with_blend_mode(BlendMode::Multiply),
        ];
        let full = compose(&layers);
        let region = compose_region(&layers, Rect::new(1, 2, 3, 2));
        assert_eq!((region.width(), region.height()), (3, 2));
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(region.get_pixel(x, y), full.get_pixel(x + 1, y + 2));
            }
        }
    }

    #[test]
    fn compose_region_clips_to_the_canvas() {
        let layer = solid_layer(4, 4, Color::rgba(1, 2, 3, 255));
        let clipped = compose_region(std::slice::from_ref(&layer), Rect::new(-2, 3, 10, 10));
        assert_eq!((clipped.width(), clipped.height()), (4, 1));
        let outside = compose_region(std::slice::from_ref(&layer), Rect::new(9, 9, 2, 2));
        assert_eq!((outside.width(), outside.height()), (0, 0));
    }

    #[test]
    fn single_opaque_layer_is_unchanged() {
        let layer = solid_layer(2, 2, Color::rgba(10, 200, 30, 255));
        let out = compose(std::slice::from_ref(&layer));
        assert_eq!(out.get_pixel(0, 0), Some(Color::rgba(10, 200, 30, 255)));
    }

    #[test]
    fn opaque_top_normal_layer_hides_backdrop() {
        let bottom = solid_layer(1, 1, Color::BLACK);
        let top = solid_layer(1, 1, Color::WHITE);
        let out = compose(&[bottom, top]);
        assert_eq!(out.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn hidden_layer_is_skipped() {
        let bottom = solid_layer(1, 1, Color::BLACK);
        let mut top = solid_layer(1, 1, Color::WHITE);
        top.visible = false;
        let out = compose(&[bottom, top]);
        assert_eq!(out.get_pixel(0, 0), Some(Color::BLACK));
    }

    #[test]
    fn zero_opacity_layer_leaves_the_output_unchanged() {
        let bottom = solid_layer(2, 2, Color::rgba(30, 60, 90, 200));
        let top = solid_layer(2, 2, Color::WHITE)
            .with_blend_mode(BlendMode::Difference)
            .with_opacity(0.0);
        assert_eq!(compose(&[bottom.clone(), top]), compose(&[bottom]));
    }

    #[test]
    fn multiply_opaque_black_over_white_is_black() {
        let bottom = solid_layer(1, 1, Color::WHITE);
        let mut top = solid_layer(1, 1, Color::BLACK);
        top.blend_mode = BlendMode::Multiply;
        let out = compose(&[bottom, top]);
        assert_eq!(out.get_pixel(0, 0), Some(Color::BLACK));
    }

    #[test]
    fn multiply_opaque_white_over_color_is_color() {
        let color = Color::rgba(255, 0, 0, 255);
        let bottom = solid_layer(1, 1, color);
        let mut top = solid_layer(1, 1, Color::WHITE);
        top.blend_mode = BlendMode::Multiply;
        let out = compose(&[bottom, top]);
        assert_eq!(out.get_pixel(0, 0), Some(color));
    }

    #[test]
    fn screen_opaque_white_over_black_is_white() {
        let bottom = solid_layer(1, 1, Color::BLACK);
        let mut top = solid_layer(1, 1, Color::WHITE);
        top.blend_mode = BlendMode::Screen;
        let out = compose(&[bottom, top]);
        assert_eq!(out.get_pixel(0, 0), Some(Color::WHITE));
    }

    #[test]
    fn fifty_percent_opacity_white_over_black_is_mid_gray() {
        let bottom = solid_layer(1, 1, Color::BLACK);
        let top = solid_layer(1, 1, Color::WHITE).with_opacity(0.5);
        let out = compose(&[bottom, top]);
        // Linear-light midpoint of black/white encodes to ~188 sRGB, not 128.
        let p = out.get_pixel(0, 0).unwrap();
        assert_eq!(p.a, 255);
        assert!((186..=190).contains(&p.r), "got {}", p.r);
        assert_eq!(p.r, p.g);
        assert_eq!(p.g, p.b);
    }

    #[test]
    fn compose_is_idempotent_for_same_input() {
        let bottom = solid_layer(3, 3, Color::rgba(20, 40, 60, 255));
        let top = solid_layer(3, 3, Color::rgba(200, 100, 50, 128));
        let a = compose(&[bottom.clone(), top.clone()]);
        let b = compose(&[bottom, top]);
        assert_eq!(a, b);
    }

    // ---- docs/specs/performance-image-kernel.md: the float path before the
    // lookup tables, kept as the reference the fast kernel must reproduce.

    fn composite_layer_reference(dst: &mut ImageBuffer, layer: &Layer, ox: u32, oy: u32) {
        let opacity = layer.opacity.clamp(0.0, 1.0);
        let mode = layer.blend_mode;
        for y in 0..dst.height() {
            for x in 0..dst.width() {
                let Some(s) = layer.pixels.get_pixel(x + ox, y + oy) else {
                    continue;
                };
                let sa = (s.a as f32 / 255.0) * opacity;
                if sa <= 0.0 {
                    continue;
                }
                let d = dst.get_pixel(x, y).unwrap_or(Color::TRANSPARENT);
                let da = d.a as f32 / 255.0;
                let sl = [s.r, s.g, s.b].map(|c| srgb_to_linear(c as f32 / 255.0));
                let dl = [d.r, d.g, d.b].map(|c| srgb_to_linear(c as f32 / 255.0));
                let ao = sa + da * (1.0 - sa);
                let mut rgb = [0.0f32; 3];
                if ao > 0.0 {
                    for i in 0..3 {
                        let blended = (1.0 - da) * sl[i] + da * blend_channel(mode, dl[i], sl[i]);
                        let co = sa * blended + da * dl[i] * (1.0 - sa);
                        rgb[i] = co / ao;
                    }
                }
                let enc = |c: f32| (linear_to_srgb(c).clamp(0.0, 1.0) * 255.0).round() as u8;
                dst.set_pixel(
                    x,
                    y,
                    Color::rgba(
                        enc(rgb[0]),
                        enc(rgb[1]),
                        enc(rgb[2]),
                        (ao.clamp(0.0, 1.0) * 255.0).round() as u8,
                    ),
                );
            }
        }
    }

    fn compose_region_reference(layers: &[Layer], region: Rect) -> ImageBuffer {
        let first = &layers[0];
        let (cw, ch) = (first.pixels.width() as i64, first.pixels.height() as i64);
        let x0 = i64::from(region.x).clamp(0, cw);
        let y0 = i64::from(region.y).clamp(0, ch);
        let x1 = (i64::from(region.x) + i64::from(region.w)).clamp(0, cw);
        let y1 = (i64::from(region.y) + i64::from(region.h)).clamp(0, ch);
        let mut out = ImageBuffer::new_transparent((x1 - x0) as u32, (y1 - y0) as u32);
        for layer in layers {
            if layer.visible && layer.opacity > 0.0 {
                composite_layer_reference(&mut out, layer, x0 as u32, y0 as u32);
            }
        }
        out
    }

    const ALL_MODES: [BlendMode; 12] = [
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

    #[test]
    fn an_opaque_normal_source_takes_the_copy_path() {
        // The exhaustive check behind the copy path holds, so the bench's
        // opaque bottom layer is copied; the equivalence tests below prove
        // the copy gives the float path's bytes.
        assert!(normal_opaque_copies());
    }

    #[test]
    fn round_byte_matches_round_for_every_float_up_to_255() {
        // Every float in 0 ..= 255 — the alpha `ao * 255` can be any of them.
        let end = 255.0f32.to_bits();
        for bits in (0..=end)
            .step_by(7)
            .chain([end, 0.5f32.to_bits(), 0.49999997f32.to_bits()])
        {
            let x = f32::from_bits(bits);
            assert_eq!(round_byte(x), x.round() as u8, "x = {x:e}");
        }
        assert_eq!(round_byte(f32::NAN), f32::NAN.round() as u8);
    }

    #[test]
    fn decode_table_matches_the_float_conversion_for_every_byte() {
        let table = decode_table();
        for b in 0..=255u8 {
            assert_eq!(
                table[b as usize].to_bits(),
                srgb_to_linear(b as f32 / 255.0).to_bits()
            );
        }
    }

    /// The encoder is exact if the reference rounding is monotonic in its
    /// input: its value only changes next to a threshold, so checking a wide
    /// window of neighbouring floats around every threshold, plus special
    /// and spread-out values, covers every place where the two could differ.
    #[test]
    fn encoder_matches_the_float_conversion_around_every_threshold() {
        let enc = encoder();
        let thresholds = encode_thresholds();
        let reference = |c: f32| (linear_to_srgb(c).clamp(0.0, 1.0) * 255.0).round() as u8;
        for (k, t) in thresholds.iter().enumerate().skip(1) {
            let t = t.to_bits();
            for bits in t.saturating_sub(4096)..=t + 4096 {
                let x = f32::from_bits(bits);
                assert_eq!(enc.encode(x), reference(x), "x = {x:e} near threshold {k}");
            }
        }
        // The first and last float of every bucket.
        for b in 0..(1.0f32.to_bits() >> BUCKET_SHIFT) {
            for bits in [b << BUCKET_SHIFT, ((b + 1) << BUCKET_SHIFT) - 1] {
                let x = f32::from_bits(bits);
                assert_eq!(enc.encode(x), reference(x), "x = {x:e} at bucket {b}");
            }
        }
        for x in [
            0.0,
            -0.0,
            -1.0,
            1.0,
            1.5,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NAN,
            1e-30,
        ] {
            assert_eq!(enc.encode(x), reference(x), "x = {x:e}");
        }
        for i in 0..=1_000_000u32 {
            let x = i as f32 / 1_000_000.0;
            assert_eq!(enc.encode(x), reference(x), "x = {x:e}");
        }
    }

    fn arb_layer(w: u32, h: u32) -> impl Strategy<Value = Layer> {
        (
            proptest::collection::vec(any::<u8>(), (w * h * 4) as usize),
            0usize..12,
            prop_oneof![
                Just(0.0f32),
                Just(1.0 / 255.0),
                Just(254.0 / 255.0),
                Just(1.0),
                0.0f32..=1.0,
            ],
            any::<bool>(),
            0u32..=2,
        )
            .prop_map(move |(data, mode, opacity, visible, shrink)| {
                // Some layers are smaller than the canvas, like a region
                // compose reading past a layer's edge.
                let (lw, lh) = ((w - shrink.min(w - 1)), (h - shrink.min(h - 1)));
                let data = data[..(lw * lh * 4) as usize].to_vec();
                let mut layer =
                    Layer::from_pixels("l", ImageBuffer::from_raw(lw, lh, data).unwrap())
                        .with_blend_mode(ALL_MODES[mode])
                        .with_opacity(opacity);
                layer.visible = visible || mode % 2 == 0;
                layer
            })
    }

    fn arb_stack() -> impl Strategy<Value = (Vec<Layer>, Rect)> {
        (1u32..=6, 1u32..=6).prop_flat_map(|(w, h)| {
            (
                proptest::collection::vec(arb_layer(w, h), 1..=4).prop_map(move |mut layers| {
                    // The first layer fixes the canvas size.
                    let base = &layers[0].pixels;
                    if (base.width(), base.height()) != (w, h) {
                        let mut full = ImageBuffer::new_transparent(w, h);
                        full.paste_region(Rect::new(0, 0, base.width(), base.height()), base)
                            .unwrap();
                        layers[0].pixels = full;
                    }
                    layers
                }),
                (-2i32..6, -2i32..6, 0u32..8, 0u32..8)
                    .prop_map(|(x, y, rw, rh)| Rect::new(x, y, rw, rh)),
            )
        })
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(2000))]

        #[test]
        fn fast_kernel_is_byte_identical_to_the_float_path((layers, region) in arb_stack()) {
            let (w, h) = (layers[0].pixels.width(), layers[0].pixels.height());
            let mut reference = ImageBuffer::new_transparent(w, h);
            for layer in &layers {
                if layer.visible && layer.opacity > 0.0 {
                    composite_layer_reference(&mut reference, layer, 0, 0);
                }
            }
            proptest::prop_assert_eq!(compose(&layers), reference);
            proptest::prop_assert_eq!(
                compose_region(&layers, region),
                compose_region_reference(&layers, region)
            );
        }
    }

    #[test]
    fn every_mode_opacity_and_alpha_pair_matches_the_float_path() {
        // Every source and backdrop alpha against a fixed colour pair, per
        // mode and for the opacities at the edges of the range.
        for mode in ALL_MODES {
            for opacity in [1.0 / 255.0, 0.5, 254.0 / 255.0, 1.0] {
                let mut bottom = ImageBuffer::new_transparent(256, 256);
                let mut top = ImageBuffer::new_transparent(256, 256);
                for y in 0..256u32 {
                    for x in 0..256u32 {
                        bottom.set_pixel(x, y, Color::rgba(x as u8, 255 - y as u8, 7, y as u8));
                        top.set_pixel(x, y, Color::rgba(y as u8, 200, x as u8, x as u8));
                    }
                }
                let layers = [
                    Layer::from_pixels("b", bottom),
                    Layer::from_pixels("t", top)
                        .with_blend_mode(mode)
                        .with_opacity(opacity),
                ];
                let mut reference = ImageBuffer::new_transparent(256, 256);
                for layer in &layers {
                    composite_layer_reference(&mut reference, layer, 0, 0);
                }
                assert!(
                    compose(&layers) == reference,
                    "{mode:?} at opacity {opacity}"
                );
            }
        }
    }

    // ---- The over-background blend before the lookup tables, kept as the
    // reference the table-driven version must reproduce byte for byte.

    fn over_background_reference(mut out: ImageBuffer, background: Color) -> ImageBuffer {
        let bg_lin = [
            srgb_to_linear(background.r as f32 / 255.0),
            srgb_to_linear(background.g as f32 / 255.0),
            srgb_to_linear(background.b as f32 / 255.0),
        ];
        for y in 0..out.height() {
            for x in 0..out.width() {
                let s = out.get_pixel(x, y).unwrap_or(Color::TRANSPARENT);
                let sa = s.a as f32 / 255.0;
                let s_lin = [
                    srgb_to_linear(s.r as f32 / 255.0),
                    srgb_to_linear(s.g as f32 / 255.0),
                    srgb_to_linear(s.b as f32 / 255.0),
                ];
                let mut rgb = [0u8; 3];
                for i in 0..3 {
                    let co = sa * s_lin[i] + (1.0 - sa) * bg_lin[i];
                    rgb[i] = (linear_to_srgb(co).clamp(0.0, 1.0) * 255.0).round() as u8;
                }
                out.set_pixel(x, y, Color::rgba(rgb[0], rgb[1], rgb[2], 255));
            }
        }
        out
    }

    /// Every (channel byte, alpha byte) pair in every channel: pixel
    /// `(c, a)` carries three different permutations of `c` and alpha `a`.
    fn every_channel_alpha_pair() -> ImageBuffer {
        let mut data = Vec::with_capacity(256 * 256 * 4);
        for a in 0..=255u8 {
            for c in 0..=255u8 {
                data.extend_from_slice(&[c, 255 - c, c ^ 0x5a, a]);
            }
        }
        ImageBuffer::from_raw(256, 256, data).expect("256 x 256 RGBA")
    }

    const BACKGROUNDS: [Color; 5] = [
        Color::WHITE,
        Color::BLACK,
        Color {
            r: 1,
            g: 128,
            b: 254,
            a: 255,
        },
        Color {
            r: 200,
            g: 13,
            b: 77,
            a: 255,
        },
        Color {
            r: 255,
            g: 0,
            b: 10,
            a: 0,
        },
    ];

    #[test]
    fn over_background_matches_the_float_path_for_every_channel_alpha_pair() {
        let src = every_channel_alpha_pair();
        for bg in BACKGROUNDS {
            assert!(
                over_background(src.clone(), bg) == over_background_reference(src.clone(), bg),
                "background {bg:?}"
            );
        }
    }

    #[test]
    fn over_background_rgb_matches_the_float_path_without_alpha() {
        let src = every_channel_alpha_pair();
        for bg in BACKGROUNDS {
            let reference = over_background_reference(src.clone(), bg);
            let rgb: Vec<u8> = reference
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|px| [px[0], px[1], px[2]])
                .collect();
            assert!(over_background_rgb(&src, bg) == rgb, "background {bg:?}");
        }
    }
}
