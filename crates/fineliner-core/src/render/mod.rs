//! Layer compositing (spec §6).

mod blend;

pub use blend::blend_channel;

use crate::color::Color;
use crate::document::{ImageBuffer, Layer};

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
/// layer (all layers are canvas-sized); an empty slice yields a 0×0 buffer.
pub fn compose(layers: &[Layer]) -> ImageBuffer {
    let Some(first) = layers.first() else {
        return ImageBuffer::new_transparent(0, 0);
    };
    let w = first.pixels.width();
    let h = first.pixels.height();
    let mut out = ImageBuffer::new_transparent(w, h);

    for layer in layers {
        if !layer.visible || layer.opacity <= 0.0 {
            continue;
        }
        composite_layer(&mut out, layer);
    }
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
/// light, yielding a fully opaque buffer (also used by the JPEG encoder).
pub(crate) fn over_background(mut out: ImageBuffer, background: Color) -> ImageBuffer {
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

/// Blends one layer onto the accumulator `dst` in place.
fn composite_layer(dst: &mut ImageBuffer, layer: &Layer) {
    let w = dst.width();
    let h = dst.height();
    let opacity = layer.opacity.clamp(0.0, 1.0);
    let mode = layer.blend_mode;

    for y in 0..h {
        for x in 0..w {
            let Some(s) = layer.pixels.get_pixel(x, y) else {
                continue;
            };
            let sa = (s.a as f32 / 255.0) * opacity;
            if sa <= 0.0 {
                continue;
            }
            // Current backdrop pixel.
            let d = dst.get_pixel(x, y).unwrap_or(Color::TRANSPARENT);
            let da = d.a as f32 / 255.0;

            let s_lin = [
                srgb_to_linear(s.r as f32 / 255.0),
                srgb_to_linear(s.g as f32 / 255.0),
                srgb_to_linear(s.b as f32 / 255.0),
            ];
            let d_lin = [
                srgb_to_linear(d.r as f32 / 255.0),
                srgb_to_linear(d.g as f32 / 255.0),
                srgb_to_linear(d.b as f32 / 255.0),
            ];

            let ao = sa + da * (1.0 - sa);
            let mut rgb = [0.0f32; 3];
            if ao > 0.0 {
                for i in 0..3 {
                    // Blended source color (W3C): (1-da)*Cs + da*B(Cb,Cs).
                    let blended =
                        (1.0 - da) * s_lin[i] + da * blend_channel(mode, d_lin[i], s_lin[i]);
                    // Source-over with the blended source, premultiplied.
                    let co = sa * blended + da * d_lin[i] * (1.0 - sa);
                    rgb[i] = co / ao;
                }
            }

            dst.set_pixel(
                x,
                y,
                Color::rgba(
                    (linear_to_srgb(rgb[0]).clamp(0.0, 1.0) * 255.0).round() as u8,
                    (linear_to_srgb(rgb[1]).clamp(0.0, 1.0) * 255.0).round() as u8,
                    (linear_to_srgb(rgb[2]).clamp(0.0, 1.0) * 255.0).round() as u8,
                    (ao.clamp(0.0, 1.0) * 255.0).round() as u8,
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::BlendMode;

    fn solid_layer(w: u32, h: u32, c: Color) -> Layer {
        let mut buf = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                buf.set_pixel(x, y, c);
            }
        }
        Layer::from_pixels("l", buf)
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
}
