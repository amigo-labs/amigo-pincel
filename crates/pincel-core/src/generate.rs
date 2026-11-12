//! Pixel helpers for image generation in Pixel mode
//! (`docs/specs/comfyui-generate-pixel.md`, terms of
//! `docs/specs/comfyui-generate.md`): the generation size, fitting a
//! variant to the target box — averaged when it shrinks, repeated with
//! hard edges when it grows — the mapping into the sprite's color mode
//! (palette for indexed, gray for grayscale, unchanged for RGBA), the input
//! and mask a generation reads, and the one-step command that adds the
//! applied variant as a new layer.

use std::collections::HashMap;

use thiserror::Error;

use crate::clipboard::{image_layer_rgba, render, subtree};
use crate::command::{AddLayer, AnyCommand, CommandBatch, PutCel, placement_above};
use crate::document::{
    Cel, CelMap, ColorMode, FrameIndex, Layer, LayerId, LayerKind, Palette, PixelBuffer, Sprite,
};
use crate::geometry::{Point, Rect};
use crate::render::{LayerFilter, RenderError};

/// The narrowest and widest aspect ratio a generation uses (1:4 – 4:1).
const RATIO_LIMIT: f64 = 4.0;

/// Alpha from which an indexed pixel is visible: 50 % (AC-03).
const HALF_ALPHA: u8 = 128;

/// Why a generation step cannot run.
#[derive(Debug, Error, PartialEq)]
pub enum GenerateError {
    /// An indexed sprite's palette has no entry besides the transparent
    /// one (AC-10).
    #[error("The palette has no visible color")]
    NoVisibleColor,
    /// The target box or the variant has no pixels, or a buffer does not
    /// match its size.
    #[error("the target box is empty")]
    EmptyBox,
    /// Rendering the input layer failed.
    #[error(transparent)]
    Render(#[from] RenderError),
}

/// Width and height for `native` pixels (SD 1.5: 512², SDXL and Flux:
/// 1024²) at the aspect ratio of a `w × h` box, clamped to 1:4 – 4:1: each
/// side rounded to the nearest multiple of 64, at least 64 (the rule of
/// `pincel-image-core::generate::generation_size`).
pub fn generation_size(native: u32, w: u32, h: u32) -> (u32, u32) {
    let ratio = (f64::from(w.max(1)) / f64::from(h.max(1))).clamp(1.0 / RATIO_LIMIT, RATIO_LIMIT);
    let n = f64::from(native);
    let side = |v: f64| -> u32 { (((v / 64.0).round() as u32).max(1)) * 64 };
    (side((n * ratio).sqrt()), side((n / ratio).sqrt()))
}

/// The source pixels one target pixel covers on one axis, with the share
/// of each (summing to the window's width in source pixels).
type Taps = Vec<(usize, f64)>;

/// Per target index on an axis of `n` source pixels scaled by `s` with
/// the window starting at `offset`: nearest neighbour when growing, the
/// area-weighted run of covered pixels when shrinking.
fn taps(n: u32, len: u32, s: f64, offset: f64) -> Vec<Taps> {
    let last = n.saturating_sub(1) as usize;
    (0..len)
        .map(|i| {
            let a = offset + f64::from(i) / s;
            let b = offset + f64::from(i + 1) / s;
            if s >= 1.0 {
                let at = ((a + b) / 2.0).floor().max(0.0) as usize;
                return vec![(at.min(last), 1.0)];
            }
            let first = a.floor().max(0.0) as usize;
            let end = (b.ceil() as usize).min(n as usize);
            (first..end)
                .filter_map(|p| {
                    let lo = a.max(p as f64);
                    let hi = b.min(p as f64 + 1.0);
                    (hi > lo).then_some((p, hi - lo))
                })
                .collect()
        })
        .collect()
}

/// Straight RGBA `src` (`sw × sh`) scaled to cover `w × h` with its aspect
/// ratio kept and the overlap cropped evenly on both sides: shrinking
/// averages the covered source pixels by area, growing repeats them
/// (nearest neighbour, hard edges).
pub fn fit(src: &[u8], sw: u32, sh: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0; w as usize * h as usize * 4];
    if w == 0 || h == 0 || sw == 0 || sh == 0 || src.len() != sw as usize * sh as usize * 4 {
        return out;
    }
    let s = (f64::from(w) / f64::from(sw)).max(f64::from(h) / f64::from(sh));
    let ox = (f64::from(sw) - f64::from(w) / s) / 2.0;
    let oy = (f64::from(sh) - f64::from(h) / s) / 2.0;
    let cols = taps(sw, w, s, ox);
    let rows = taps(sh, h, s, oy);
    for (y, row) in rows.iter().enumerate() {
        for (x, col) in cols.iter().enumerate() {
            // Color weighted by alpha, so clear pixels add no dark fringe.
            let (mut area, mut alpha) = (0.0, 0.0);
            let mut color = [0.0f64; 3];
            for &(sy, wy) in row {
                for &(sx, wx) in col {
                    let i = (sy * sw as usize + sx) * 4;
                    let weight = wx * wy;
                    let a = f64::from(src[i + 3]) * weight;
                    area += weight;
                    alpha += a;
                    for (c, v) in color.iter_mut().zip(&src[i..i + 3]) {
                        *c += f64::from(*v) * a;
                    }
                }
            }
            if area <= 0.0 || alpha <= 0.0 {
                continue;
            }
            let o = (y * w as usize + x) * 4;
            for (k, c) in color.iter().enumerate() {
                out[o + k] = (c / alpha).round().clamp(0.0, 255.0) as u8;
            }
            out[o + 3] = (alpha / area).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// Multiplies each pixel's alpha by `mask` (one byte per pixel, 255 =
/// keep); an empty mask leaves `rgba` as it is.
pub fn apply_mask(rgba: &mut [u8], mask: &[u8]) {
    if mask.is_empty() {
        return;
    }
    for (px, &m) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(mask) {
        px[3] = ((u32::from(px[3]) * u32::from(m) + 127) / 255) as u8;
        if px[3] == 0 {
            px.fill(0);
        }
    }
}

/// The selection's coverage inside `area` (one byte per pixel, row-major);
/// empty without a selection.
pub fn generation_mask(sprite: &Sprite, area: Rect) -> Vec<u8> {
    if !sprite.has_selection() {
        return Vec::new();
    }
    let rect = sprite.selection.unwrap_or(area);
    let shape = sprite.selection_mask.as_ref();
    let mut out = Vec::with_capacity(area.width as usize * area.height as usize);
    for y in area.y..area.y + area.height as i32 {
        for x in area.x..area.x + area.width as i32 {
            let on = match shape {
                Some(m) => m.contains(x, y),
                None => rect.contains(Point::new(x, y)),
            };
            out.push(if on { 255 } else { 0 });
        }
    }
    out
}

/// `layer` at `frame` inside `area` as straight RGBA, as the sprite shows
/// it: an image layer's own cel (a linked cel followed, indexed and
/// grayscale cels through the palette), a group's or tilemap's rendered
/// pixels. `None` when it is fully transparent there (nothing to refine).
///
/// # Errors
///
/// [`GenerateError::Render`] when rendering a group or tilemap fails.
pub fn generation_input(
    sprite: &Sprite,
    cels: &CelMap,
    frame: FrameIndex,
    layer: LayerId,
    area: Rect,
) -> Result<Option<Vec<u8>>, GenerateError> {
    let canvas = Rect::new(0, 0, sprite.width, sprite.height);
    let area = area.intersect(canvas);
    if area.is_empty() {
        return Ok(None);
    }
    let full = match sprite.layer(layer) {
        None => return Ok(None),
        Some(l) if matches!(l.kind, LayerKind::Image) => {
            image_layer_rgba(sprite, cels, layer, frame)
        }
        Some(_) => render(
            sprite,
            cels,
            frame,
            LayerFilter::Only(subtree(sprite, layer)),
        )?,
    };
    let cw = sprite.width as usize;
    let mut out = Vec::with_capacity(area.width as usize * area.height as usize * 4);
    for y in area.y..area.y + area.height as i32 {
        let from = (y as usize * cw + area.x as usize) * 4;
        out.extend_from_slice(&full[from..from + area.width as usize * 4]);
    }
    Ok(out
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[3] != 0)
        .then_some(out))
}

/// The palette entries an indexed pixel may take: every entry but the
/// transparent one, with its OKLab color.
fn visible_entries(palette: &Palette, transparent: u8) -> Vec<(u8, [f64; 3])> {
    palette
        .colors
        .iter()
        .enumerate()
        .take(256)
        .filter(|(i, _)| *i != usize::from(transparent))
        .map(|(i, e)| (i as u8, oklab([e.rgba.r, e.rgba.g, e.rgba.b])))
        .collect()
}

/// Whether the sprite can hold a generated pixel: always for RGBA and
/// grayscale, for an indexed sprite only with a palette entry besides the
/// transparent one (AC-10).
pub fn has_visible_color(sprite: &Sprite) -> bool {
    match sprite.color_mode {
        ColorMode::Indexed { transparent_index } => {
            !visible_entries(&sprite.palette, transparent_index).is_empty()
        }
        ColorMode::Rgba | ColorMode::Grayscale => true,
    }
}

/// The entry nearest to `lab` by squared OKLab distance; the lowest index
/// on a tie.
fn nearest(entries: &[(u8, [f64; 3])], lab: [f64; 3]) -> u8 {
    let mut best = (f64::INFINITY, 0u8);
    for &(i, e) in entries {
        let d = (e[0] - lab[0]).powi(2) + (e[1] - lab[1]).powi(2) + (e[2] - lab[2]).powi(2);
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// Straight RGBA (`w × h`) in the sprite's color mode (AC-03 – AC-05):
/// indexed pixels take the nearest palette entry other than the transparent
/// one by perceived color (OKLab), or the transparent index below 50 %
/// alpha; grayscale pixels take their perceived luminance with their alpha;
/// RGBA stays as it is. The palette is not changed.
///
/// # Errors
///
/// [`GenerateError::NoVisibleColor`] for an indexed sprite without a
/// visible palette entry; [`GenerateError::EmptyBox`] when `rgba` does not
/// hold `w × h` pixels.
pub fn to_sprite_pixels(
    sprite: &Sprite,
    rgba: &[u8],
    w: u32,
    h: u32,
) -> Result<PixelBuffer, GenerateError> {
    if rgba.len() != w as usize * h as usize * 4 {
        return Err(GenerateError::EmptyBox);
    }
    let mode = sprite.color_mode;
    let data = match mode {
        ColorMode::Rgba => rgba.to_vec(),
        ColorMode::Grayscale => rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [gray([p[0], p[1], p[2]]), p[3]])
            .collect(),
        ColorMode::Indexed { transparent_index } => {
            let entries = visible_entries(&sprite.palette, transparent_index);
            if entries.is_empty() {
                return Err(GenerateError::NoVisibleColor);
            }
            // Variants repeat colors, scaled-up ones heavily.
            let mut seen: HashMap<[u8; 3], u8> = HashMap::new();
            rgba.as_chunks::<4>()
                .0
                .iter()
                .map(|p| {
                    if p[3] < HALF_ALPHA {
                        return transparent_index;
                    }
                    let c = [p[0], p[1], p[2]];
                    *seen.entry(c).or_insert_with(|| nearest(&entries, oklab(c)))
                })
                .collect()
        }
    };
    Ok(PixelBuffer {
        width: w,
        height: h,
        color_mode: mode,
        data,
    })
}

/// A variant (straight RGBA, `vw × vh`) fitted to `area`, its alpha
/// multiplied by `mask` (empty: none), in the sprite's color mode — what
/// the preview shows and Apply writes (AC-02 – AC-05).
///
/// # Errors
///
/// As [`to_sprite_pixels`]; [`GenerateError::EmptyBox`] for an empty
/// variant or area.
pub fn prepare_variant(
    sprite: &Sprite,
    variant: &[u8],
    vw: u32,
    vh: u32,
    area: Rect,
    mask: &[u8],
) -> Result<PixelBuffer, GenerateError> {
    if area.is_empty() || vw == 0 || vh == 0 {
        return Err(GenerateError::EmptyBox);
    }
    let mut rgba = fit(variant, vw, vh, area.width, area.height);
    apply_mask(&mut rgba, mask);
    to_sprite_pixels(sprite, &rgba, area.width, area.height)
}

/// `buf` (in the sprite's color mode) as straight RGBA, as the canvas
/// shows it: indexed through the palette, the transparent index clear.
pub fn shown_rgba(palette: &Palette, buf: &PixelBuffer) -> Vec<u8> {
    match buf.color_mode {
        ColorMode::Rgba => buf.data.clone(),
        ColorMode::Grayscale => buf
            .data
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        ColorMode::Indexed { transparent_index } => buf
            .data
            .iter()
            .flat_map(|&i| {
                if i == transparent_index {
                    return [0; 4];
                }
                palette
                    .colors
                    .get(usize::from(i))
                    .map_or([0; 4], |e| [e.rgba.r, e.rgba.g, e.rgba.b, e.rgba.a])
            })
            .collect(),
    }
}

/// One undo step that adds `layer` (a new image layer) directly above
/// `above` — above a group's subtree, inside the anchor's group — with a
/// canvas-sized cel on `frame` holding `pixels` at `area`'s position,
/// cropped to the canvas (AC-06, AC-09). Pixels outside `pixels` keep the
/// empty value: the transparent index of an indexed sprite.
pub fn add_generated_layer(
    sprite: &Sprite,
    mut layer: Layer,
    above: Option<LayerId>,
    frame: FrameIndex,
    area: Rect,
    pixels: &PixelBuffer,
) -> CommandBatch {
    let (index, parent) = placement_above(sprite, above);
    layer.parent = parent;
    let mode = sprite.color_mode;
    let (cw, ch) = (sprite.width, sprite.height);
    let mut buf = PixelBuffer::empty(cw, ch, mode);
    if let ColorMode::Indexed { transparent_index } = mode {
        buf.data.fill(transparent_index);
    }
    let bpp = mode.bytes_per_pixel();
    if pixels.color_mode == mode && pixels.is_well_formed() {
        for sy in 0..pixels.height {
            let cy = i64::from(area.y) + i64::from(sy);
            if !(0..i64::from(ch)).contains(&cy) {
                continue;
            }
            for sx in 0..pixels.width {
                let cx = i64::from(area.x) + i64::from(sx);
                if !(0..i64::from(cw)).contains(&cx) {
                    continue;
                }
                let si = (sy as usize * pixels.width as usize + sx as usize) * bpp;
                let di = (cy as usize * cw as usize + cx as usize) * bpp;
                buf.data[di..di + bpp].copy_from_slice(&pixels.data[si..si + bpp]);
            }
        }
    }
    let cel = Cel::image(layer.id, frame, buf);
    CommandBatch::new(vec![
        AnyCommand::from(AddLayer::at(layer, index)),
        AnyCommand::from(PutCel::new(cel)),
    ])
}

/// sRGB channel → linear light.
fn linear(u: u8) -> f64 {
    let v = f64::from(u) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light → sRGB channel.
fn encode(y: f64) -> u8 {
    let v = if y <= 0.003_130_8 {
        12.92 * y
    } else {
        1.055 * y.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0).round().clamp(0.0, 255.0) as u8
}

/// OKLab of an sRGB color (Björn Ottosson's reference matrices).
fn oklab(c: [u8; 3]) -> [f64; 3] {
    let (r, g, b) = (linear(c[0]), linear(c[1]), linear(c[2]));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    [
        0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
    ]
}

/// The gray with the same perceived lightness (OKLab L) as `c`.
fn gray(c: [u8; 3]) -> u8 {
    let l = oklab(c)[0];
    encode(l * l * l)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Bus, Command};
    use crate::document::{CelData, Frame, PaletteEntry, Rgba};
    use crate::selection::SelectionMask;

    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];

    fn sprite(w: u32, h: u32, mode: ColorMode) -> Sprite {
        let mut s = Sprite::builder(w, h)
            .add_layer(Layer::image(LayerId::new(0), "a"))
            .add_layer(Layer::image(LayerId::new(1), "b"))
            .add_frame(Frame::new(100))
            .add_frame(Frame::new(100))
            .build()
            .expect("sprite");
        s.color_mode = mode;
        s
    }

    fn indexed(colors: &[[u8; 3]], transparent: u8) -> Sprite {
        let mut s = sprite(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: transparent,
            },
        );
        s.palette = Palette::from_entries(
            colors
                .iter()
                .map(|c| PaletteEntry::new(Rgba::new(c[0], c[1], c[2], 255)))
                .collect(),
        );
        s
    }

    fn px(c: [u8; 4], n: usize) -> Vec<u8> {
        c.repeat(n)
    }

    #[test]
    fn generation_size_keeps_the_pixel_count_and_the_ratio_limit() {
        assert_eq!(generation_size(512 * 512, 32, 32), (512, 512));
        assert_eq!(generation_size(1024 * 1024, 16, 16), (1024, 1024));
        // 2:1 at 1024²: √(2·1024²) ≈ 1448 → 1472, √(1024²/2) ≈ 724 → 704.
        assert_eq!(generation_size(1024 * 1024, 64, 32), (1472, 704));
        // 20:1 is clamped to 4:1.
        assert_eq!(generation_size(1024 * 1024, 200, 10), (2048, 512));
    }

    #[test]
    fn fit_shrinking_averages_the_covered_pixels() {
        let src = [RED, BLUE, RED, BLUE].concat();
        assert_eq!(fit(&src, 2, 2, 1, 1), vec![128, 0, 128, 255]);
        // A 1×1 box is the variant's average color.
        let src4 = [RED, RED, RED, BLUE].concat();
        assert_eq!(fit(&src4, 2, 2, 1, 1), vec![191, 0, 64, 255]);
    }

    #[test]
    fn fit_shrinking_by_a_fraction_weighs_partial_coverage() {
        let row = [[0, 0, 0, 255], [90, 90, 90, 255], [180, 180, 180, 255]].concat();
        let src = row.repeat(3);
        // 3 → 2: each target pixel covers 1.5 source pixels.
        let out = [[30, 30, 30, 255], [150, 150, 150, 255]].concat();
        assert_eq!(fit(&src, 3, 3, 2, 2), out.repeat(2));
    }

    #[test]
    fn fit_shrinking_weighs_color_by_alpha() {
        let src = [RED, [0, 0, 0, 0]].concat().repeat(2);
        assert_eq!(fit(&src, 2, 2, 1, 1), vec![255, 0, 0, 128]);
    }

    #[test]
    fn fit_growing_repeats_pixels_with_hard_edges() {
        let src = [[0, 0, 0, 255], [255, 255, 255, 255]].concat();
        let b = [0, 0, 0, 255];
        let w = [255, 255, 255, 255];
        let row = [b, b, w, w].concat();
        assert_eq!(fit(&src, 2, 1, 4, 2), [row.clone(), row].concat());
    }

    #[test]
    fn fit_crops_the_overlap_evenly() {
        let c = |v: u8| [v, v, v, 255];
        let src = [c(10), c(20), c(30), c(40)].concat();
        // 4×1 covering 2×1 at scale 1: the middle two columns.
        assert_eq!(fit(&src, 4, 1, 2, 1), [c(20), c(30)].concat());
        // 8×2 covering 1×1 at scale 1/2: the middle 2×2 block.
        let wide: Vec<u8> = (0..2)
            .flat_map(|_| [0, 0, 0, 100, 200, 0, 0, 0].map(c).concat())
            .collect();
        assert_eq!(fit(&wide, 8, 2, 1, 1), c(150));
    }

    #[test]
    fn apply_mask_clears_outside_and_keeps_inside() {
        let mut rgba = [RED, BLUE].concat();
        apply_mask(&mut rgba, &[255, 0]);
        assert_eq!(rgba, [RED, [0; 4]].concat());
        let mut same = [RED, BLUE].concat();
        apply_mask(&mut same, &[]);
        assert_eq!(same, [RED, BLUE].concat());
    }

    #[test]
    fn to_sprite_pixels_indexed_takes_the_nearest_color_by_oklab() {
        // Index 0 matches exactly but is the transparent one; plain RGB
        // distance would pick index 1, perceived color picks index 2.
        let s = indexed(&[[0, 0, 200], [0, 0, 120], [60, 60, 255]], 0);
        let buf = to_sprite_pixels(&s, &[0, 0, 200, 255], 1, 1).expect("map");
        assert_eq!(buf.data, vec![2]);
        assert!(matches!(buf.color_mode, ColorMode::Indexed { .. }));
        assert_eq!(s.palette.len(), 3, "palette unchanged");
    }

    #[test]
    fn to_sprite_pixels_indexed_below_half_alpha_is_transparent() {
        let s = indexed(&[[9, 9, 9], [255, 0, 0]], 0);
        let rgba = [[255, 0, 0, 127], [255, 0, 0, 128]].concat();
        assert_eq!(
            to_sprite_pixels(&s, &rgba, 2, 1).expect("map").data,
            vec![0, 1]
        );
    }

    #[test]
    fn to_sprite_pixels_indexed_with_one_visible_color_uses_it() {
        let s = indexed(&[[0, 0, 0], [10, 200, 10]], 0);
        let rgba = [RED, BLUE].concat();
        assert_eq!(
            to_sprite_pixels(&s, &rgba, 2, 1).expect("map").data,
            vec![1, 1]
        );
    }

    #[test]
    fn to_sprite_pixels_indexed_without_a_visible_color_fails() {
        let s = indexed(&[[0, 0, 0]], 0);
        assert!(!has_visible_color(&s));
        assert_eq!(
            to_sprite_pixels(&s, &RED, 1, 1),
            Err(GenerateError::NoVisibleColor)
        );
        assert!(has_visible_color(&indexed(&[[0, 0, 0], [1, 1, 1]], 0)));
        assert!(has_visible_color(&sprite(1, 1, ColorMode::Rgba)));
    }

    #[test]
    fn to_sprite_pixels_grayscale_is_perceived_luminance_with_alpha() {
        let s = sprite(2, 1, ColorMode::Grayscale);
        let rgba = [[0, 255, 0, 200], [128, 128, 128, 255]].concat();
        let buf = to_sprite_pixels(&s, &rgba, 2, 1).expect("map");
        assert_eq!(buf.data, vec![211, 200, 128, 255]);
        assert_eq!(buf.color_mode, ColorMode::Grayscale);
    }

    #[test]
    fn to_sprite_pixels_rgba_keeps_colors_and_alpha() {
        let s = sprite(2, 1, ColorMode::Rgba);
        let rgba = [[12, 34, 56, 78], BLUE].concat();
        assert_eq!(to_sprite_pixels(&s, &rgba, 2, 1).expect("map").data, rgba);
    }

    #[test]
    fn prepare_variant_outside_the_mask_is_transparent() {
        let s = indexed(&[[0, 0, 0], [255, 0, 0]], 0);
        let variant = px(RED, 4);
        let buf =
            prepare_variant(&s, &variant, 2, 2, Rect::new(1, 1, 2, 1), &[255, 0]).expect("prepare");
        assert_eq!(buf.data, vec![1, 0]);
        assert_eq!(shown_rgba(&s.palette, &buf), [RED, [0; 4]].concat());
    }

    #[test]
    fn shown_rgba_takes_palette_and_gray_values() {
        let s = indexed(&[[0, 0, 0], [12, 34, 56]], 0);
        let buf = PixelBuffer {
            width: 2,
            height: 1,
            color_mode: s.color_mode,
            data: vec![1, 0],
        };
        assert_eq!(
            shown_rgba(&s.palette, &buf),
            [[12, 34, 56, 255], [0; 4]].concat()
        );
        let g = PixelBuffer {
            width: 1,
            height: 1,
            color_mode: ColorMode::Grayscale,
            data: vec![90, 200],
        };
        assert_eq!(shown_rgba(&s.palette, &g), vec![90, 90, 90, 200]);
    }

    #[test]
    fn add_generated_layer_adds_one_layer_above_the_anchor_as_one_undo_step() {
        let mut s = indexed(&[[0, 0, 0], [255, 0, 0]], 0);
        let mut cels = CelMap::new();
        let pixels = PixelBuffer {
            width: 2,
            height: 1,
            color_mode: s.color_mode,
            data: vec![1, 0],
        };
        let cmd = add_generated_layer(
            &s,
            Layer::image(LayerId::new(9), "Generated: slime"),
            Some(LayerId::new(0)),
            FrameIndex::new(1),
            Rect::new(3, 2, 2, 1),
            &pixels,
        );
        let mut bus = Bus::new();
        bus.execute_as(
            crate::command::HistoryKind::Generate,
            cmd.into(),
            &mut s,
            &mut cels,
        )
        .expect("apply");
        let ids: Vec<u32> = s.layers.iter().map(|l| l.id.0).collect();
        assert_eq!(ids, vec![0, 9, 1]);
        let cel = cels.get(LayerId::new(9), FrameIndex::new(1)).expect("cel");
        let CelData::Image(buf) = &cel.data else {
            panic!("not an image cel")
        };
        assert_eq!((buf.width, buf.height), (4, 4));
        // (3, 2) holds index 1, the masked-out (4, 2) is off-canvas; the
        // rest stays the transparent index.
        assert_eq!(buf.data[2 * 4 + 3], 1);
        assert_eq!(buf.data.iter().filter(|&&v| v != 0).count(), 1);
        assert!(cels.get(LayerId::new(9), FrameIndex::new(0)).is_none());
        assert_eq!(bus.undo_depth(), 1);
        assert!(bus.undo(&mut s, &mut cels));
        assert_eq!(s.layers.len(), 2);
        assert!(cels.get(LayerId::new(9), FrameIndex::new(1)).is_none());
    }

    #[test]
    fn add_generated_layer_fills_an_indexed_cel_with_the_transparent_index() {
        let mut s = indexed(&[[0, 0, 0], [255, 0, 0], [0, 255, 0]], 2);
        let mut cels = CelMap::new();
        let pixels = PixelBuffer {
            width: 1,
            height: 1,
            color_mode: s.color_mode,
            data: vec![1],
        };
        let mut cmd = add_generated_layer(
            &s,
            Layer::image(LayerId::new(5), "g"),
            None,
            FrameIndex::new(0),
            Rect::new(0, 0, 1, 1),
            &pixels,
        );
        cmd.apply(&mut s, &mut cels).expect("apply");
        assert_eq!(
            s.layers.last().map(|l| l.id.0),
            Some(5),
            "on top without an anchor"
        );
        let CelData::Image(buf) = &cels
            .get(LayerId::new(5), FrameIndex::new(0))
            .expect("cel")
            .data
        else {
            panic!("not an image cel")
        };
        assert_eq!(buf.data[0], 1);
        assert!(buf.data[1..].iter().all(|&v| v == 2));
    }

    #[test]
    fn add_generated_layer_above_a_tilemap_is_a_normal_layer_above_it() {
        let mut s = sprite(4, 4, ColorMode::Rgba);
        s.layers[0] = Layer::tilemap(LayerId::new(0), "map", crate::document::TilesetId::new(0));
        let mut cels = CelMap::new();
        let pixels = PixelBuffer {
            width: 1,
            height: 1,
            color_mode: ColorMode::Rgba,
            data: RED.to_vec(),
        };
        let mut cmd = add_generated_layer(
            &s,
            Layer::image(LayerId::new(7), "g"),
            Some(LayerId::new(0)),
            FrameIndex::new(0),
            Rect::new(0, 0, 1, 1),
            &pixels,
        );
        cmd.apply(&mut s, &mut cels).expect("apply");
        assert_eq!(s.layers[1].id, LayerId::new(7));
        assert_eq!(s.layers[1].kind, LayerKind::Image);
    }

    #[test]
    fn generation_input_reads_the_layer_unmasked_and_none_when_transparent() {
        let mut s = sprite(3, 1, ColorMode::Rgba);
        let mut cels = CelMap::new();
        let mut buf = PixelBuffer::empty(3, 1, ColorMode::Rgba);
        buf.data = [RED, BLUE, RED].concat();
        cels.insert(Cel::image(LayerId::new(0), FrameIndex::new(0), buf));
        // Frame 1 links to frame 0: read, not copied.
        let mut linked = Cel::image(
            LayerId::new(0),
            FrameIndex::new(1),
            PixelBuffer::empty(0, 0, ColorMode::Rgba),
        );
        linked.data = CelData::Linked(FrameIndex::new(0));
        cels.insert(linked);
        s.selection = Some(Rect::new(1, 0, 2, 1));
        s.selection_mask = SelectionMask::from_raw(3, 1, vec![0, 0, 255]);
        let area = Rect::new(1, 0, 2, 1);
        let input =
            generation_input(&s, &cels, FrameIndex::new(1), LayerId::new(0), area).expect("input");
        assert_eq!(
            input,
            Some([BLUE, RED].concat()),
            "the context outside the mask stays"
        );
        assert_eq!(
            generation_input(&s, &cels, FrameIndex::new(0), LayerId::new(1), area).expect("input"),
            None
        );
    }

    #[test]
    fn generation_mask_is_empty_without_a_selection_and_follows_its_shape() {
        let mut s = sprite(3, 1, ColorMode::Rgba);
        assert!(generation_mask(&s, Rect::new(0, 0, 3, 1)).is_empty());
        s.selection = Some(Rect::new(0, 0, 3, 1));
        assert_eq!(
            generation_mask(&s, Rect::new(0, 0, 3, 1)),
            vec![255, 255, 255]
        );
        s.selection_mask = SelectionMask::from_raw(3, 1, vec![255, 0, 255]);
        assert_eq!(generation_mask(&s, Rect::new(1, 0, 2, 1)), vec![0, 255]);
    }
}
