//! Preview rendering: compose → nearest-neighbour upscale → optional grid
//! → PNG bytes. Shared by `doc.preview` and `export.png`.

use pincel_core::{CelMap, ComposeRequest, FrameIndex, Rect, Sprite, compose};

use crate::AgentError;

/// Largest preview edge in pixels (AC-45).
pub(crate) const PREVIEW_MAX_EDGE: u32 = 2048;
/// Longest edge the automatic preview scale aims for (AC-11).
const AUTO_TARGET: u32 = 256;
/// Smallest scale that gets a grid (AC-12).
pub(crate) const GRID_MIN_SCALE: u32 = 4;
/// Grid line color: `#808080` at 50 % alpha.
const GRID_RGB: u8 = 0x80;
const GRID_ALPHA: u8 = 128;

/// Smallest integer scale that makes the longest side at least 256 px
/// (1 when it already is).
pub(crate) fn auto_scale(width: u32, height: u32) -> u32 {
    let longest = width.max(height).max(1);
    AUTO_TARGET.div_ceil(longest).max(1)
}

/// Compose `region` of `frame` at 1:1 (visible layers, no onion skin).
pub(crate) fn compose_region(
    sprite: &Sprite,
    cels: &CelMap,
    frame: FrameIndex,
    region: Rect,
) -> Result<Vec<u8>, AgentError> {
    let mut request = ComposeRequest::full(frame, sprite.width, sprite.height);
    request.viewport = region;
    let mut pixels = Vec::new();
    compose(sprite, cels, &request, &mut pixels)
        .map_err(|e| AgentError::tool(format!("cannot compose frame {}: {e}", frame.0)))?;
    Ok(pixels)
}

/// Nearest-neighbour upscale of an RGBA8 `width`×`height` buffer by `s`.
pub(crate) fn upscale(pixels: &[u8], width: u32, height: u32, s: u32) -> Vec<u8> {
    if s == 1 {
        return pixels.to_vec();
    }
    let (w, h, s) = (width as usize, height as usize, s as usize);
    let out_stride = w * s * 4;
    let mut out = vec![0u8; out_stride * h * s];
    for y in 0..h {
        let row = &mut out[y * s * out_stride..(y * s + 1) * out_stride];
        for x in 0..w {
            let src = &pixels[(y * w + x) * 4..(y * w + x) * 4 + 4];
            for k in 0..s {
                let o = (x * s + k) * 4;
                row[o..o + 4].copy_from_slice(src);
            }
        }
        for k in 1..s {
            let (head, tail) = out.split_at_mut((y * s + k) * out_stride);
            tail[..out_stride].copy_from_slice(&head[y * s * out_stride..(y * s + 1) * out_stride]);
        }
    }
    out
}

/// Source-over of the grid color onto one pixel.
pub(crate) fn blend_grid(px: [u8; 4]) -> [u8; 4] {
    let sa = u32::from(GRID_ALPHA);
    let da = u32::from(px[3]);
    // Alphas in 0..=255; out_a = sa + da * (255 - sa) / 255, all ×255.
    let out_a255 = sa * 255 + da * (255 - sa);
    if out_a255 == 0 {
        return [0, 0, 0, 0];
    }
    let ch = |d: u8| -> u8 {
        let num = u32::from(GRID_RGB) * sa * 255 + u32::from(d) * da * (255 - sa);
        ((num + out_a255 / 2) / out_a255) as u8
    };
    [
        ch(px[0]),
        ch(px[1]),
        ch(px[2]),
        ((out_a255 + 127) / 255) as u8,
    ]
}

/// Draw the grid on the first row and first column of every `s`×`s` block
/// of an upscaled buffer of `width`×`height` output pixels.
pub(crate) fn draw_grid(pixels: &mut [u8], width: u32, height: u32, s: u32) {
    let (w, s) = (width as usize, s as usize);
    for y in 0..height as usize {
        for x in 0..w {
            if x % s == 0 || y % s == 0 {
                let i = (y * w + x) * 4;
                let px = [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]];
                pixels[i..i + 4].copy_from_slice(&blend_grid(px));
            }
        }
    }
}

/// Encode an RGBA8 buffer as PNG.
pub(crate) fn encode_png(pixels: &[u8], width: u32, height: u32) -> Result<Vec<u8>, AgentError> {
    let fail = |e: png::EncodingError| AgentError::tool(format!("PNG encoding failed: {e}"));
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(fail)?;
    writer.write_image_data(pixels).map_err(fail)?;
    writer.finish().map_err(fail)?;
    Ok(out)
}

/// Compose `region` of `frame`, upscale by `scale` and encode as PNG,
/// refusing output edges above `max_edge`. Returns the PNG and its size.
pub(crate) fn render_png(
    sprite: &Sprite,
    cels: &CelMap,
    frame: FrameIndex,
    region: Rect,
    scale: u32,
    grid: bool,
    max_edge: u32,
) -> Result<(Vec<u8>, u32, u32), AgentError> {
    let out_w = u64::from(region.width) * u64::from(scale);
    let out_h = u64::from(region.height) * u64::from(scale);
    if out_w > u64::from(max_edge) || out_h > u64::from(max_edge) {
        return Err(AgentError::tool(format!(
            "the image would be {out_w}×{out_h} px, above the {max_edge} px cap per edge; pass a `region` to show part of the canvas, or a smaller `scale`"
        )));
    }
    let (out_w, out_h) = (out_w as u32, out_h as u32);
    let pixels = compose_region(sprite, cels, frame, region)?;
    let mut big = upscale(&pixels, region.width, region.height, scale);
    if grid {
        draw_grid(&mut big, out_w, out_h, scale);
    }
    Ok((encode_png(&big, out_w, out_h)?, out_w, out_h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_scale_reaches_256_on_the_longest_side() {
        assert_eq!(auto_scale(1, 1), 256);
        assert_eq!(auto_scale(32, 16), 8);
        assert_eq!(auto_scale(100, 10), 3);
        assert_eq!(auto_scale(256, 4), 1);
        assert_eq!(auto_scale(4096, 4096), 1);
    }

    #[test]
    fn upscale_repeats_each_pixel_into_a_block() {
        let px = [1, 2, 3, 4, 5, 6, 7, 8];
        let out = upscale(&px, 2, 1, 2);
        assert_eq!(out.len(), 4 * 2 * 4);
        assert_eq!(&out[0..8], &[1, 2, 3, 4, 1, 2, 3, 4]);
        assert_eq!(&out[16..24], &[1, 2, 3, 4, 1, 2, 3, 4]);
        assert_eq!(&out[24..32], &[5, 6, 7, 8, 5, 6, 7, 8]);
    }

    #[test]
    fn blend_grid_over_transparent_is_half_grey() {
        assert_eq!(blend_grid([0, 0, 0, 0]), [0x80, 0x80, 0x80, 128]);
        assert_eq!(blend_grid([255, 255, 255, 255]), [191, 191, 191, 255]);
    }
}
