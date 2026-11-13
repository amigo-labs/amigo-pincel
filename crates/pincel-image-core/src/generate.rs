//! Pixel helpers for image generation (docs/specs/comfyui-generate.md,
//! Terms): the generation size, fitting an image between the target box
//! and the generation size without distortion, and the input and mask a
//! generation reads from the document.

use crate::document::{Document, ImageBuffer};
use crate::geometry::Rect;
use crate::render::compose_region;
use crate::transform::{Interpolation, scale};

/// The narrowest and widest aspect ratio a generation uses (1:4 – 4:1).
const RATIO_LIMIT: f64 = 4.0;

/// Width and height for `native` pixels (SD 1.5: 512², SDXL and Flux:
/// 1024²) at the aspect ratio of a `w × h` box, clamped to 1:4 – 4:1: each
/// side rounded to the nearest multiple of 64, at least 64.
pub fn generation_size(native: u32, w: u32, h: u32) -> (u32, u32) {
    let ratio = (f64::from(w.max(1)) / f64::from(h.max(1))).clamp(1.0 / RATIO_LIMIT, RATIO_LIMIT);
    let n = f64::from(native);
    let side = |v: f64| -> u32 { (((v / 64.0).round() as u32).max(1)) * 64 };
    (side((n * ratio).sqrt()), side((n / ratio).sqrt()))
}

/// `src` scaled to cover `w × h` with its aspect ratio kept and the
/// overlap cropped evenly on both sides (Terms, "Fitting").
pub fn fit(src: &ImageBuffer, w: u32, h: u32) -> ImageBuffer {
    let (sw, sh) = (src.width(), src.height());
    if w == 0 || h == 0 || sw == 0 || sh == 0 {
        return ImageBuffer::new_transparent(w, h);
    }
    if (sw, sh) == (w, h) {
        return src.clone();
    }
    let s = (f64::from(w) / f64::from(sw)).max(f64::from(h) / f64::from(sh));
    // At least the target on both axes, so the crop never runs short.
    let cw = ((f64::from(sw) * s).round() as u32).max(w);
    let ch = ((f64::from(sh) * s).round() as u32).max(h);
    let scaled = scale(src, cw, ch, Interpolation::Supersampling);
    let (dx, dy) = ((cw - w) / 2, (ch - h) / 2);
    let mut out = ImageBuffer::new_transparent(w, h);
    let row = (w * 4) as usize;
    for y in 0..h {
        let from = (((y + dy) * cw + dx) * 4) as usize;
        let to = (y * w * 4) as usize;
        out.data_mut()[to..to + row].copy_from_slice(&scaled.data()[from..from + row]);
    }
    out
}

/// A one-channel mask (`mw × mh`) fitted like [`fit`].
pub fn fit_mask(mask: &[u8], mw: u32, mh: u32, w: u32, h: u32) -> Vec<u8> {
    let rgba: Vec<u8> = mask.iter().flat_map(|&m| [m, m, m, 255]).collect();
    let Ok(buf) = ImageBuffer::from_raw(mw, mh, rgba) else {
        return vec![0; (w * h) as usize];
    };
    fit(&buf, w, h)
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[0])
        .collect()
}

/// Multiplies each pixel's alpha by `mask` (one byte per pixel, 255 =
/// keep); an empty mask leaves `img` as it is.
pub fn apply_mask(img: &mut ImageBuffer, mask: &[u8]) {
    if mask.is_empty() {
        return;
    }
    for (px, &m) in img.data_mut().as_chunks_mut::<4>().0.iter_mut().zip(mask) {
        px[3] = ((u32::from(px[3]) * u32::from(m) + 127) / 255) as u8;
        if px[3] == 0 {
            px.copy_from_slice(&[0; 4]);
        }
    }
}

/// The selection's coverage inside `area` (one byte per pixel, row-major);
/// empty without a selection.
pub fn generation_mask(doc: &Document, area: Rect) -> Vec<u8> {
    let Some(sel) = doc.selection.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity((area.w * area.h) as usize);
    for y in 0..area.h {
        for x in 0..area.w {
            let (px, py) = (area.x + x as i32, area.y + y as i32);
            let cov = if px < 0 || py < 0 {
                0
            } else {
                sel.coverage(px as u32, py as u32)
            };
            out.push(cov);
        }
    }
    out
}

/// The active layer's pixels inside `area` — a group's composite for a
/// group, a hidden layer's own pixels as they are; `None` when the layer is
/// fully transparent there (nothing to refine).
pub fn generation_input(doc: &Document, area: Rect) -> Option<ImageBuffer> {
    let active = doc.active_layer_index();
    let layer = doc.active_layer();
    let pixels = if layer.is_group() {
        compose_region(&doc.layers[active + 1..doc.subtree_end(active)], area)
    } else {
        layer.pixels.copy_region(area).ok()?
    };
    pixels
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[3] != 0)
        .then_some(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::selection::SelectionMask;

    #[test]
    fn generation_size_keeps_the_pixel_count_and_the_ratio_limit() {
        assert_eq!(generation_size(1024 * 1024, 800, 800), (1024, 1024));
        assert_eq!(generation_size(512 * 512, 3, 3), (512, 512));
        assert_eq!(generation_size(1024 * 1024, 1600, 900), (1344, 768));
        // 10000×500 is 20:1, generated at the 4:1 limit.
        assert_eq!(generation_size(1024 * 1024, 10_000, 500), (2048, 512));
        assert_eq!(generation_size(1024 * 1024, 500, 10_000), (512, 2048));
        assert_eq!(generation_size(64, 1, 1), (64, 64));
    }

    fn halves(w: u32, h: u32) -> ImageBuffer {
        // Left half red, right half blue.
        let mut b = ImageBuffer::new_transparent(w, h);
        for y in 0..h {
            for x in 0..w {
                let c = if x < w / 2 {
                    Color::rgba(255, 0, 0, 255)
                } else {
                    Color::rgba(0, 0, 255, 255)
                };
                b.set_pixel(x, y, c);
            }
        }
        b
    }

    #[test]
    fn fit_covers_and_crops_evenly_without_distortion() {
        // 4:1 into 2:1: scaled to height, cropped left and right.
        let wide = halves(400, 100);
        let out = fit(&wide, 100, 50);
        assert_eq!((out.width(), out.height()), (100, 50));
        assert_eq!(out.get_pixel(0, 25), Some(Color::rgba(255, 0, 0, 255)));
        assert_eq!(out.get_pixel(99, 25), Some(Color::rgba(0, 0, 255, 255)));
        // A square into 3×3 and into a very wide box.
        let sq = halves(64, 64);
        assert_eq!(fit(&sq, 3, 3).width(), 3);
        let strip = fit(&sq, 200, 10);
        assert_eq!((strip.width(), strip.height()), (200, 10));
        assert_eq!(strip.get_pixel(0, 5), Some(Color::rgba(255, 0, 0, 255)));
    }

    #[test]
    fn fit_mask_and_apply_mask_blend_by_coverage() {
        let mask = vec![255, 0, 128, 255];
        assert_eq!(fit_mask(&mask, 2, 2, 2, 2), mask);
        let mut img = ImageBuffer::from_raw(2, 2, [10, 20, 30, 255].repeat(4)).unwrap();
        apply_mask(&mut img, &mask);
        assert_eq!(img.get_pixel(0, 0), Some(Color::rgba(10, 20, 30, 255)));
        assert_eq!(img.get_pixel(1, 0), Some(Color::TRANSPARENT));
        assert_eq!(img.get_pixel(0, 1).map(|c| c.a), Some(128));
    }

    #[test]
    fn mask_and_input_come_from_the_selection_and_the_active_layer() {
        let mut doc = Document::new(8, 8).unwrap();
        let area = Rect::new(2, 2, 4, 4);
        assert!(generation_mask(&doc, area).is_empty());
        assert!(generation_input(&doc, area).is_none(), "transparent layer");
        doc.layers[0]
            .pixels
            .set_pixel(3, 3, Color::rgba(1, 2, 3, 255));
        let input = generation_input(&doc, area).unwrap();
        assert_eq!((input.width(), input.height()), (4, 4));
        assert_eq!(input.get_pixel(1, 1), Some(Color::rgba(1, 2, 3, 255)));
        let mut sel = SelectionMask::new_empty(8, 8);
        sel.set(3, 2, 200);
        doc.selection = Some(sel);
        let mask = generation_mask(&doc, area);
        assert_eq!(mask.len(), 16);
        assert_eq!(mask[1], 200);
        assert_eq!(mask[0], 0);
    }
}
