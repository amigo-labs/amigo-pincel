//! Bit-exact port of the part of Leptonica 1.82.0 that Tesseract runs on a
//! 32-bpp RGB line image before recognition: `pixScale(pixs, s, s)` with all
//! the branches `pixScaleGeneral` takes for equal x / y factors, and
//! `pixConvertRGBToLuminance`.
//!
//! Ported functions (Leptonica `scale1.c`, `enhance.c`, `pix2.c`,
//! `pixconv.c`): `pixScale`, `pixScaleGeneral`, `pixScaleSmooth` /
//! `scaleSmoothLow`, `pixScaleAreaMap` / `scaleColorAreaMapLow`,
//! `pixScaleAreaMap2` / `scaleAreaMapLow2`, `pixScaleColorLI` /
//! `scaleColorLILow`, `pixScaleColor2xLI` / `scaleColor2xLILow` /
//! `scaleColor2xLILineLow`, `pixScaleColor4xLI` / `pixScaleGray4xLI` /
//! `scaleGray4xLILow` / `scaleGray4xLILineLow`, `pixUnsharpMasking` /
//! `pixUnsharpMaskingFast` / `pixUnsharpMaskingGrayFast` /
//! `pixUnsharpMaskingGray2D`, `pixGetRGBComponent`, `pixCreateRGBImage`,
//! `composeRGBPixel`, the size checks of `pixCreateHeader`, and
//! `pixConvertRGBToGray`.
//!
//! The arithmetic mirrors the C types exactly: `l_float32` is `f32`, an
//! expression with a double literal (`16.`, `0.5`, `i + 1.0`) is evaluated
//! in `f64`, comparisons of an `l_float32` against a double literal
//! (`maxscale < 0.7`) promote to `f64`, and `(l_int32)` casts truncate as
//! x86-64 does ([`c_int`]). Pixel words are `0xRRGGBBAA`; the low (alpha)
//! byte is carried or zeroed exactly where Leptonica carries or zeroes it.

/// A 32-bpp RGB image in Leptonica's layout: one u32 per pixel, 0xRRGGBBAA
/// (red in the high byte), rows of `width` pixels with no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rgb32 {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) data: Vec<u32>,
}

/// An 8-bpp grey image, rows of `width` bytes with no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Grey8 {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) data: Vec<u8>,
}

/// Leptonica `pixScale(pixs, scale, scale)` for a 32-bpp RGB pix (spp = 3).
///
/// Where Leptonica returns NULL this returns an empty image (`width == 0`,
/// `height == 0`, no data); Leptonica never produces an empty pix, so an
/// empty result means exactly "Leptonica fails". That happens for:
///
/// - an invalid source: zero width or height, `data.len() != width *
///   height`, or a size `pixCreate` refuses (more than 2^24 - 1 pixels per
///   row or 2^31 bytes);
/// - `scale <= 0`, NaN or infinite, or an output size `pixCreate` refuses;
/// - area mapping (`0.02 <= scale < 0.7`, `scale` promoted to `f64`) whose
///   rounded output width or height is 0, e.g. a 1-pixel-high line at 0.33;
/// - the exact factors 0.5 / 0.25 / 0.125 / 0.0625, which halve the size
///   1 / 2 / 3 / 4 times with truncation, once a halving reaches 0 (a source
///   narrower or lower than 2, 4, 8 or 16 pixels).
///
/// For `scale < 0.02` a source smaller than the filter size gives the 1×1
/// image holding the centre source pixel, as in `pixScaleSmooth`.
pub(crate) fn scale_rgb(src: &Rgb32, scale: f32) -> Rgb32 {
    let Some((ws, hs)) = source_dims(src.width, src.height, src.data.len()) else {
        return Rgb32::empty();
    };
    let src = Pix {
        w: ws,
        h: hs,
        data: src.data.as_slice(),
    };
    pix_scale(&src, scale).unwrap_or_else(Rgb32::empty)
}

/// Leptonica `pixConvertRGBToLuminance` (pixConvertRGBToGray with weights 0,0,0).
///
/// The default weights are 0.3 / 0.5 / 0.2 (`L_RED_WEIGHT` …); the alpha
/// byte is ignored. An invalid source (zero width or height, or
/// `data.len() != width * height`) gives an empty image.
pub(crate) fn rgb_to_luminance(src: &Rgb32) -> Grey8 {
    let Some(len) = src.width.checked_mul(src.height) else {
        return Grey8::empty();
    };
    if len == 0 || src.data.len() != len {
        return Grey8::empty();
    }
    let mut rwt = L_RED_WEIGHT;
    let mut gwt = L_GREEN_WEIGHT;
    let mut bwt = L_BLUE_WEIGHT;
    let sum = rwt + gwt + bwt;
    if (f64::from(sum) - 1.0).abs() > 0.0001 {
        rwt /= sum;
        gwt /= sum;
        bwt /= sum;
    }
    let data = src
        .data
        .iter()
        .map(|&word| {
            let r = ((word >> RED_SHIFT) & 0xff) as f32;
            let g = ((word >> GREEN_SHIFT) & 0xff) as f32;
            let b = ((word >> BLUE_SHIFT) & 0xff) as f32;
            let val = c_int(f64::from(rwt * r + gwt * g + bwt * b) + 0.5);
            // SET_DATA_BYTE stores the low byte.
            val as u8
        })
        .collect();
    Grey8 {
        width: src.width,
        height: src.height,
        data,
    }
}

impl Rgb32 {
    /// The result that stands for Leptonica's NULL.
    fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            data: Vec::new(),
        }
    }

    /// A zeroed image of a size [`pix_create_ok`] accepted.
    fn zeroed(w: i32, h: i32) -> Self {
        let (w, h) = (dim(w), dim(h));
        Self {
            width: w,
            height: h,
            data: vec![0; w * h],
        }
    }
}

impl Grey8 {
    /// An empty image for an invalid source.
    fn empty() -> Self {
        Self {
            width: 0,
            height: 0,
            data: Vec::new(),
        }
    }
}

const RED_SHIFT: u32 = 24;
const GREEN_SHIFT: u32 = 16;
const BLUE_SHIFT: u32 = 8;

/// `L_RED_WEIGHT` … in Leptonica's `pix.h`.
const L_RED_WEIGHT: f32 = 0.3;
const L_GREEN_WEIGHT: f32 = 0.5;
const L_BLUE_WEIGHT: f32 = 0.2;

/// A borrowed 32-bpp source with C-typed dimensions.
struct Pix<'a> {
    w: i32,
    h: i32,
    data: &'a [u32],
}

impl Rgb32 {
    fn as_pix(&self) -> Pix<'_> {
        Pix {
            w: i32::try_from(self.width).unwrap_or(0),
            h: i32::try_from(self.height).unwrap_or(0),
            data: &self.data,
        }
    }
}

/// C's `(l_int32)` cast of a floating-point value as x86-64 executes it
/// (`cvttsd2si`): truncation toward zero, and `i32::MIN` for NaN and for
/// values outside the `i32` range (undefined behaviour in C).
fn c_int(v: f64) -> i32 {
    if v > -2_147_483_649.0 && v < 2_147_483_648.0 {
        v as i32
    } else {
        i32::MIN
    }
}

/// A non-negative C dimension or index as `usize` (negative gives 0).
fn dim(v: i32) -> usize {
    usize::try_from(v).unwrap_or(0)
}

/// The size checks of `pixCreateHeader` for a `w` × `h` pix of `depth` bpp.
fn pix_create_ok(w: i32, h: i32, depth: u64) -> bool {
    if w <= 0 || h <= 0 {
        return false;
    }
    let wpl = (u64::from(w.unsigned_abs()) * depth).div_ceil(32);
    if wpl > (1 << 24) - 1 {
        return false;
    }
    4 * wpl * u64::from(h.unsigned_abs()) < (1 << 31)
}

/// The dimensions of a valid source, or `None` where Leptonica could not
/// hold it.
fn source_dims(width: usize, height: usize, len: usize) -> Option<(i32, i32)> {
    let w = i32::try_from(width).ok()?;
    let h = i32::try_from(height).ok()?;
    if !pix_create_ok(w, h, 32) || width.checked_mul(height)? != len {
        return None;
    }
    Some((w, h))
}

/// The word at linear index `idx` (C pointer arithmetic over rows with
/// `wpl == w`); 0 outside the buffer, which the ported index math never
/// reaches.
fn word_at(data: &[u32], idx: i64) -> u32 {
    usize::try_from(idx)
        .ok()
        .and_then(|i| data.get(i))
        .copied()
        .unwrap_or(0)
}

/// `composeRGBPixel`: alpha byte 0, no masking of the components.
fn compose(r: i32, g: i32, b: i32) -> u32 {
    ((r as u32) << RED_SHIFT) | ((g as u32) << GREEN_SHIFT) | ((b as u32) << BLUE_SHIFT)
}

/// `pixScale` → `pixScaleGeneral` for a 32-bpp RGB source.
fn pix_scale(src: &Pix<'_>, scale: f32) -> Option<Rgb32> {
    let s = f64::from(scale);
    // pixScale: the default sharpening.
    let (sharpfract, sharpwidth): (f32, i32) = if s < 0.7 { (0.2, 1) } else { (0.4, 2) };

    // pixScaleGeneral.
    if s <= 0.0 {
        return None;
    }
    if scale == 1.0 {
        return Some(copy(src));
    }
    if s < 0.7 {
        let pix2 = if s < 0.02 {
            scale_smooth(src, scale)?
        } else {
            scale_area_map(src, scale)?
        };
        if s > 0.2 && sharpfract > 0.0 && sharpwidth > 0 {
            unsharp_masking(&pix2, sharpwidth, sharpfract)
        } else {
            Some(pix2)
        }
    } else {
        let pix2 = scale_color_li(src, scale)?;
        if s < 1.4 && sharpfract > 0.0 && sharpwidth > 0 {
            unsharp_masking(&pix2, sharpwidth, sharpfract)
        } else {
            Some(pix2)
        }
    }
}

/// `pixCopy`.
fn copy(src: &Pix<'_>) -> Rgb32 {
    Rgb32 {
        width: dim(src.w),
        height: dim(src.h),
        data: src.data.to_vec(),
    }
}

/// `pixScaleSmooth` for `scale < 0.02` (32 bpp).
fn scale_smooth(src: &Pix<'_>, scale: f32) -> Option<Rgb32> {
    let (ws, hs) = (src.w, src.h);
    // size = 1.0 / minscale, stored in an l_float32.
    let size = (1.0 / f64::from(scale)) as f32;
    let isize = c_int(f64::from(size) + 0.5).clamp(2, 10000);

    if ws < isize || hs < isize {
        let val = word_at(
            src.data,
            i64::from(hs / 2) * i64::from(ws) + i64::from(ws / 2),
        );
        return Some(Rgb32 {
            width: 1,
            height: 1,
            data: vec![val],
        });
    }

    let wd = c_int(f64::from(scale * ws as f32) + 0.5).max(1);
    let hd = c_int(f64::from(scale * hs as f32) + 0.5).max(1);
    if !pix_create_ok(wd, hd, 32) {
        return None;
    }
    let mut dst = Rgb32::zeroed(wd, hd);
    scale_smooth_low(&mut dst.data, wd, hd, src, isize);
    Some(dst)
}

/// `scaleSmoothLow`, 32-bpp branch.
fn scale_smooth_low(datad: &mut [u32], wd: i32, hd: i32, src: &Pix<'_>, size: i32) {
    let (ws, hs) = (src.w, src.h);
    // norm = 1. / (l_float32)(size * size)
    let norm = (1.0 / f64::from(size.wrapping_mul(size) as f32)) as f32;
    let wratio = ws as f32 / wd as f32;
    let hratio = hs as f32 / hd as f32;
    let srow: Vec<i32> = (0..hd)
        .map(|i| c_int(f64::from(hratio * i as f32)).min(hs - size))
        .collect();
    let scol: Vec<i32> = (0..wd)
        .map(|j| c_int(f64::from(wratio * j as f32)).min(ws - size))
        .collect();

    let wpls = i64::from(ws);
    for (i, &row) in srow.iter().enumerate() {
        let lines = i64::from(row) * wpls;
        let lined = i * dim(wd);
        for (j, &xstart) in scol.iter().enumerate() {
            let (mut rval, mut gval, mut bval) = (0i32, 0i32, 0i32);
            for m in 0..i64::from(size) {
                let ppixel = lines + m * wpls + i64::from(xstart);
                for n in 0..i64::from(size) {
                    let pixel = word_at(src.data, ppixel + n);
                    rval = rval.wrapping_add(((pixel >> RED_SHIFT) & 0xff) as i32);
                    gval = gval.wrapping_add(((pixel >> GREEN_SHIFT) & 0xff) as i32);
                    bval = bval.wrapping_add(((pixel >> BLUE_SHIFT) & 0xff) as i32);
                }
            }
            let rval = c_int(f64::from(rval as f32 * norm));
            let gval = c_int(f64::from(gval as f32 * norm));
            let bval = c_int(f64::from(bval as f32 * norm));
            datad[lined + j] = compose(rval, gval, bval);
        }
    }
}

/// `pixScaleAreaMap` for `0.02 <= scale < 0.7` (32 bpp).
fn scale_area_map(src: &Pix<'_>, scale: f32) -> Option<Rgb32> {
    // Special cases: 2x, 4x, 8x, 16x reduction.
    let halvings = if scale == 0.5 {
        1
    } else if scale == 0.25 {
        2
    } else if scale == 0.125 {
        3
    } else if scale == 0.0625 {
        4
    } else {
        0
    };
    if halvings > 0 {
        let mut pix = scale_area_map2(src)?;
        for _ in 1..halvings {
            pix = scale_area_map2(&pix.as_pix())?;
        }
        return Some(pix);
    }

    let (ws, hs) = (src.w, src.h);
    let wd = c_int(f64::from(scale * ws as f32) + 0.5);
    let hd = c_int(f64::from(scale * hs as f32) + 0.5);
    if wd < 1 || hd < 1 || !pix_create_ok(wd, hd, 32) {
        return None;
    }
    let mut dst = Rgb32::zeroed(wd, hd);
    scale_color_area_map_low(&mut dst.data, wd, hd, src);
    Some(dst)
}

/// `pixScaleAreaMap2` → `scaleAreaMapLow2`, 32-bpp branch.
fn scale_area_map2(src: &Pix<'_>) -> Option<Rgb32> {
    let wd = src.w / 2;
    let hd = src.h / 2;
    if !pix_create_ok(wd, hd, 32) {
        return None;
    }
    let mut dst = Rgb32::zeroed(wd, hd);
    let wpls = i64::from(src.w);
    for i in 0..hd {
        let lines = 2 * i64::from(i) * wpls;
        let lined = dim(i) * dim(wd);
        for j in 0..wd {
            let x = 2 * i64::from(j);
            let mut rval = 0i32;
            let mut gval = 0i32;
            let mut bval = 0i32;
            for idx in [
                lines + x,
                lines + x + 1,
                lines + wpls + x,
                lines + wpls + x + 1,
            ] {
                let pixel = word_at(src.data, idx);
                rval += ((pixel >> RED_SHIFT) & 0xff) as i32;
                gval += ((pixel >> GREEN_SHIFT) & 0xff) as i32;
                bval += ((pixel >> BLUE_SHIFT) & 0xff) as i32;
            }
            dst.data[lined + dim(j)] = compose(rval >> 2, gval >> 2, bval >> 2);
        }
    }
    Some(dst)
}

/// `16. * (l_float32)ws / (l_float32)wd`: computed in double, stored in an
/// `l_float32`.
fn subpixel_ratio(ws: i32, wd: i32) -> f32 {
    (16.0 * f64::from(ws as f32) / f64::from(wd as f32)) as f32
}

/// The red, green and blue bytes of a pixel word.
fn channels(pixel: u32) -> [i32; 3] {
    [
        ((pixel >> RED_SHIFT) & 0xff) as i32,
        ((pixel >> GREEN_SHIFT) & 0xff) as i32,
        ((pixel >> BLUE_SHIFT) & 0xff) as i32,
    ]
}

/// `scaleColorAreaMapLow`.
fn scale_color_area_map_low(datad: &mut [u32], wd: i32, hd: i32, src: &Pix<'_>) {
    let (ws, hs) = (src.w, src.h);
    let scx = subpixel_ratio(ws, wd);
    let scy = subpixel_ratio(hs, hd);
    let wm2 = ws - 2;
    let hm2 = hs - 2;
    let wpls = i64::from(ws);
    let data = src.data;

    for i in 0..hd {
        let yu = c_int(f64::from(scy * i as f32));
        let yl = c_int(f64::from(scy) * (f64::from(i) + 1.0));
        let yup = yu >> 4;
        let yuf = yu & 0x0f;
        let ylp = yl >> 4;
        let ylf = yl & 0x0f;
        let dely = ylp - yup;
        let lines = i64::from(yup) * wpls;
        let lined = dim(i) * dim(wd);
        let row = |k: i32| lines + i64::from(k) * wpls;
        for j in 0..wd {
            let xu = c_int(f64::from(scx * j as f32));
            let xl = c_int(f64::from(scx) * (f64::from(j) + 1.0));
            let xup = xu >> 4;
            let xuf = xu & 0x0f;
            let xlp = xl >> 4;
            let xlf = xl & 0x0f;
            let delx = xlp - xup;

            // Near the edge: just use a src pixel value (alpha included).
            if xlp > wm2 || ylp > hm2 {
                datad[lined + dim(j)] = word_at(data, lines + i64::from(xup));
                continue;
            }

            let area = ((16 - xuf) + 16 * (delx - 1) + xlf) * ((16 - yuf) + 16 * (dely - 1) + ylf);

            let pixel00 = channels(word_at(data, lines + i64::from(xup)));
            let pixel10 = channels(word_at(data, lines + i64::from(xlp)));
            let pixel01 = channels(word_at(data, row(dely) + i64::from(xup)));
            let pixel11 = channels(word_at(data, row(dely) + i64::from(xlp)));
            let area00 = (16 - xuf) * (16 - yuf);
            let area10 = xlf * (16 - yuf);
            let area01 = (16 - xuf) * ylf;
            let area11 = xlf * ylf;
            let mut sum = [0i32; 3];
            for c in 0..3 {
                sum[c] = area00 * pixel00[c]
                    + area01 * pixel01[c]
                    + area10 * pixel10[c]
                    + area11 * pixel11[c];
            }
            // Full interior src pixels.
            let mut vin = [0i32; 3];
            for k in 1..dely {
                for m in 1..delx {
                    let p = channels(word_at(data, row(k) + i64::from(xup + m)));
                    for c in 0..3 {
                        vin[c] += 256 * p[c];
                    }
                }
            }
            // Side parts that are full in one direction.
            let mut vmid = [0i32; 3];
            let areal = (16 - xuf) * 16;
            let arear = xlf * 16;
            let areat = 16 * (16 - yuf);
            let areab = 16 * ylf;
            for k in 1..dely {
                let p = channels(word_at(data, row(k) + i64::from(xup)));
                for c in 0..3 {
                    vmid[c] += areal * p[c];
                }
            }
            for k in 1..dely {
                let p = channels(word_at(data, row(k) + i64::from(xlp)));
                for c in 0..3 {
                    vmid[c] += arear * p[c];
                }
            }
            for m in 1..delx {
                let p = channels(word_at(data, lines + i64::from(xup + m)));
                for c in 0..3 {
                    vmid[c] += areat * p[c];
                }
            }
            for m in 1..delx {
                let p = channels(word_at(data, row(dely) + i64::from(xup + m)));
                for c in 0..3 {
                    vmid[c] += areab * p[c];
                }
            }
            let val = |c: usize| (sum[c] + vin[c] + vmid[c] + 128) / area;
            datad[lined + dim(j)] = compose(val(0), val(1), val(2));
        }
    }
}

/// `pixScaleColorLI` for `scale >= 0.7` (`scale` promoted to `f64`).
fn scale_color_li(src: &Pix<'_>, scale: f32) -> Option<Rgb32> {
    if scale == 1.0 {
        return Some(copy(src));
    }
    if scale == 2.0 {
        return scale_color_2x_li(src);
    }
    if scale == 4.0 {
        return scale_color_4x_li(src);
    }
    let (ws, hs) = (src.w, src.h);
    let wd = c_int(f64::from(scale * ws as f32) + 0.5);
    let hd = c_int(f64::from(scale * hs as f32) + 0.5);
    if !pix_create_ok(wd, hd, 32) {
        return None;
    }
    let mut dst = Rgb32::zeroed(wd, hd);
    scale_color_li_low(&mut dst.data, wd, hd, src);
    Some(dst)
}

/// `scaleColorLILow`.
fn scale_color_li_low(datad: &mut [u32], wd: i32, hd: i32, src: &Pix<'_>) {
    let (ws, hs) = (src.w, src.h);
    let scx = subpixel_ratio(ws, wd);
    let scy = subpixel_ratio(hs, hd);
    let wm2 = ws - 2;
    let hm2 = hs - 2;
    let wpls = i64::from(ws);
    let data = src.data;

    for i in 0..hd {
        let ypm = c_int(f64::from(scy * i as f32));
        let yp = ypm >> 4;
        let yf = ypm & 0x0f;
        let lines = i64::from(yp) * wpls;
        let lined = dim(i) * dim(wd);
        for j in 0..wd {
            let xpm = c_int(f64::from(scx * j as f32));
            let xp = xpm >> 4;
            let xf = xpm & 0x0f;
            let at = |idx: i64| word_at(data, idx);

            let pixels1 = at(lines + i64::from(xp));
            let (pixels2, pixels3, pixels4) = if xp > wm2 || yp > hm2 {
                if yp > hm2 && xp <= wm2 {
                    // Pixels near the bottom.
                    let p2 = at(lines + i64::from(xp) + 1);
                    (p2, pixels1, p2)
                } else if xp > wm2 && yp <= hm2 {
                    // Pixels near the right side.
                    let p3 = at(lines + wpls + i64::from(xp));
                    (pixels1, p3, p3)
                } else {
                    // Pixels at the lower right corner.
                    (pixels1, pixels1, pixels1)
                }
            } else {
                (
                    at(lines + i64::from(xp) + 1),
                    at(lines + wpls + i64::from(xp)),
                    at(lines + wpls + i64::from(xp) + 1),
                )
            };

            let area00 = ((16 - xf) * (16 - yf)) as u32;
            let area10 = (xf * (16 - yf)) as u32;
            let area01 = ((16 - xf) * yf) as u32;
            let area11 = (xf * yf) as u32;
            let sum = |shift: u32| {
                area00 * ((pixels1 >> shift) & 0xff)
                    + area10 * ((pixels2 >> shift) & 0xff)
                    + area01 * ((pixels3 >> shift) & 0xff)
                    + area11 * ((pixels4 >> shift) & 0xff)
                    + 128
            };
            datad[lined + dim(j)] = ((sum(RED_SHIFT) << 16) & 0xff00_0000)
                | ((sum(GREEN_SHIFT) << 8) & 0x00ff_0000)
                | (sum(BLUE_SHIFT) & 0x0000_ff00);
        }
    }
}

/// `pixScaleColor2xLI` → `scaleColor2xLILow`.
fn scale_color_2x_li(src: &Pix<'_>) -> Option<Rgb32> {
    let (ws, hs) = (src.w, src.h);
    if !pix_create_ok(2 * ws, 2 * hs, 32) {
        return None;
    }
    let mut dst = Rgb32::zeroed(2 * ws, 2 * hs);
    let wpls = dim(ws);
    let wpld = 2 * wpls;
    let hsm = dim(hs - 1);
    for i in 0..hsm {
        let lines = &src.data[i * wpls..(i + 2) * wpls];
        let lined = &mut dst.data[2 * i * wpld..(2 * i + 2) * wpld];
        scale_color_2x_li_line_low(lined, wpld, lines, wpls, false);
    }
    let lines = &src.data[hsm * wpls..(hsm + 1) * wpls];
    let lined = &mut dst.data[2 * hsm * wpld..(2 * hsm + 2) * wpld];
    scale_color_2x_li_line_low(lined, wpld, lines, wpls, true);
    Some(dst)
}

/// Red, green and blue of a word as `u32` (the 2x code keeps them unsigned).
fn rgb_u32(pixel: u32) -> (u32, u32, u32) {
    (pixel >> 24, (pixel >> 16) & 0xff, (pixel >> 8) & 0xff)
}

/// `scaleColor2xLILineLow`: `lines` holds the current src line (and the
/// next one unless `last`), `lined` the two dest lines.
fn scale_color_2x_li_line_low(
    lined: &mut [u32],
    wpld: usize,
    lines: &[u32],
    ws: usize,
    last: bool,
) {
    let wsm = ws - 1;
    let avg2 = |a: (u32, u32, u32), b: (u32, u32, u32)| {
        (((a.0 + b.0) << 23) & 0xff00_0000)
            | (((a.1 + b.1) << 15) & 0x00ff_0000)
            | (((a.2 + b.2) << 7) & 0x0000_ff00)
    };
    let plain = |a: (u32, u32, u32)| (a.0 << 24) | (a.1 << 16) | (a.2 << 8);
    if !last {
        let (top, below) = lines.split_at(ws);
        let mut v2 = rgb_u32(top[0]);
        let mut v4 = rgb_u32(below[0]);
        for j in 0..wsm {
            let jd = 2 * j;
            let v1 = v2;
            let v3 = v4;
            v2 = rgb_u32(top[j + 1]);
            v4 = rgb_u32(below[j + 1]);
            lined[jd] = plain(v1);
            lined[jd + 1] = avg2(v1, v2);
            lined[wpld + jd] = avg2(v1, v3);
            lined[wpld + jd + 1] = (((v1.0 + v2.0 + v3.0 + v4.0) << 22) & 0xff00_0000)
                | (((v1.1 + v2.1 + v3.1 + v4.1) << 14) & 0x00ff_0000)
                | (((v1.2 + v2.2 + v3.2 + v4.2) << 6) & 0x0000_ff00);
        }
        let v1 = v2;
        let v3 = v4;
        let pixel = plain(v1);
        lined[2 * wsm] = pixel;
        lined[2 * wsm + 1] = pixel;
        let pixel = avg2(v1, v3);
        lined[wpld + 2 * wsm] = pixel;
        lined[wpld + 2 * wsm + 1] = pixel;
    } else {
        let mut v2 = rgb_u32(lines[0]);
        for j in 0..wsm {
            let jd = 2 * j;
            let v1 = v2;
            v2 = rgb_u32(lines[j + 1]);
            let pixel = plain(v1);
            lined[jd] = pixel;
            lined[wpld + jd] = pixel;
            let pixel = avg2(v1, v2);
            lined[jd + 1] = pixel;
            lined[wpld + jd + 1] = pixel;
        }
        let pixel = plain(v2);
        lined[2 * wsm] = pixel;
        lined[2 * wsm + 1] = pixel;
        lined[wpld + 2 * wsm] = pixel;
        lined[wpld + 2 * wsm + 1] = pixel;
    }
}

/// `pixGetRGBComponent` for red (0), green (1) or blue (2).
fn component(src: &Pix<'_>, comp: usize) -> Vec<u8> {
    let shift = 24 - 8 * comp as u32;
    src.data.iter().map(|&p| (p >> shift) as u8).collect()
}

/// `pixCreateRGBImage` from three equally sized channels (alpha byte 0).
fn create_rgb_image(w: i32, h: i32, r: &[u8], g: &[u8], b: &[u8]) -> Rgb32 {
    let data = r
        .iter()
        .zip(g)
        .zip(b)
        .map(|((&r, &g), &b)| (u32::from(r) << 24) | (u32::from(g) << 16) | (u32::from(b) << 8))
        .collect();
    Rgb32 {
        width: dim(w),
        height: dim(h),
        data,
    }
}

/// `pixScaleColor4xLI`: each channel through `pixScaleGray4xLI`.
fn scale_color_4x_li(src: &Pix<'_>) -> Option<Rgb32> {
    let (ws, hs) = (src.w, src.h);
    if !pix_create_ok(4 * ws, 4 * hs, 32) {
        return None;
    }
    let [r, g, b] = [0, 1, 2].map(|c| scale_gray_4x_li(&component(src, c), dim(ws), dim(hs)));
    Some(create_rgb_image(4 * ws, 4 * hs, &r, &g, &b))
}

/// `pixScaleGray4xLI` → `scaleGray4xLILow`.
fn scale_gray_4x_li(src: &[u8], ws: usize, hs: usize) -> Vec<u8> {
    let wpld = 4 * ws;
    let mut dst = vec![0u8; wpld * 4 * hs];
    let hsm = hs - 1;
    for i in 0..hsm {
        let lines = &src[i * ws..(i + 2) * ws];
        let lined = &mut dst[4 * i * wpld..(4 * i + 4) * wpld];
        scale_gray_4x_li_line_low(lined, wpld, lines, ws, false);
    }
    let lines = &src[hsm * ws..(hsm + 1) * ws];
    let lined = &mut dst[4 * hsm * wpld..(4 * hsm + 4) * wpld];
    scale_gray_4x_li_line_low(lined, wpld, lines, ws, true);
    dst
}

/// `scaleGray4xLILineLow`: `lines` holds the current src line (and the
/// next one unless `last`), `lined` the four dest lines.
fn scale_gray_4x_li_line_low(lined: &mut [u8], wpld: usize, lines: &[u8], ws: usize, last: bool) {
    let wsm = ws - 1;
    let wsm4 = 4 * wsm;
    // SET_DATA_BYTE stores the low byte.
    let mut set = |row: usize, col: usize, v: i32| lined[row * wpld + col] = v as u8;
    if !last {
        let (top, below) = lines.split_at(ws);
        let mut s2 = i32::from(top[0]);
        let mut s4 = i32::from(below[0]);
        for j in 0..wsm {
            let jd = 4 * j;
            let s1 = s2;
            let s3 = s4;
            s2 = i32::from(top[j + 1]);
            s4 = i32::from(below[j + 1]);
            let (s1t, s2t, s3t, s4t) = (3 * s1, 3 * s2, 3 * s3, 3 * s4);
            set(0, jd, s1);
            set(0, jd + 1, (s1t + s2) / 4);
            set(0, jd + 2, (s1 + s2) / 2);
            set(0, jd + 3, (s1 + s2t) / 4);
            set(1, jd, (s1t + s3) / 4);
            set(1, jd + 1, (9 * s1 + s2t + s3t + s4) / 16);
            set(1, jd + 2, (s1t + s2t + s3 + s4) / 8);
            set(1, jd + 3, (s1t + 9 * s2 + s3 + s4t) / 16);
            set(2, jd, (s1 + s3) / 2);
            set(2, jd + 1, (s1t + s2 + s3t + s4) / 8);
            set(2, jd + 2, (s1 + s2 + s3 + s4) / 4);
            set(2, jd + 3, (s1 + s2t + s3 + s4t) / 8);
            set(3, jd, (s1 + s3t) / 4);
            set(3, jd + 1, (s1t + s2 + 9 * s3 + s4t) / 16);
            set(3, jd + 2, (s1 + s2 + s3t + s4t) / 8);
            set(3, jd + 3, (s1 + s2t + s3t + 9 * s4) / 16);
        }
        let s1 = s2;
        let s3 = s4;
        let s1t = 3 * s1;
        let s3t = 3 * s3;
        for k in 0..4 {
            set(0, wsm4 + k, s1);
            set(1, wsm4 + k, (s1t + s3) / 4);
            set(2, wsm4 + k, (s1 + s3) / 2);
            set(3, wsm4 + k, (s1 + s3t) / 4);
        }
    } else {
        let mut s2 = i32::from(lines[0]);
        for j in 0..wsm {
            let jd = 4 * j;
            let s1 = s2;
            s2 = i32::from(lines[j + 1]);
            let s1t = 3 * s1;
            let s2t = 3 * s2;
            for row in 0..4 {
                set(row, jd, s1);
                set(row, jd + 1, (s1t + s2) / 4);
                set(row, jd + 2, (s1 + s2) / 2);
                set(row, jd + 3, (s1 + s2t) / 4);
            }
        }
        for row in 0..4 {
            for k in 0..4 {
                set(row, wsm4 + k, s2);
            }
        }
    }
}

/// `pixUnsharpMasking` for 32 bpp with `halfwidth` 1 or 2, which goes
/// through `pixUnsharpMaskingFast` (both directions) → per channel
/// `pixUnsharpMaskingGrayFast` → `pixUnsharpMaskingGray2D`, and recombines
/// the channels with `pixCreateRGBImage` (alpha byte 0).
fn unsharp_masking(src: &Rgb32, halfwidth: i32, fract: f32) -> Option<Rgb32> {
    if fract <= 0.0 || halfwidth <= 0 {
        return Some(src.clone());
    }
    if halfwidth != 1 && halfwidth != 2 {
        // The general path (pixUnsharpMaskingGray with a block convolution)
        // is never taken by pixScale.
        return None;
    }
    let pix = src.as_pix();
    let hw = dim(halfwidth);
    let [r, g, b] = [0, 1, 2]
        .map(|c| unsharp_masking_gray_2d(&component(&pix, c), src.width, src.height, hw, fract));
    Some(create_rgb_image(pix.w, pix.h, &r, &g, &b))
}

/// `pixUnsharpMaskingGray2D` for `halfwidth` 1 or 2: the border of width
/// `halfwidth` is copied, the interior sharpened.
fn unsharp_masking_gray_2d(src: &[u8], w: usize, h: usize, hw: usize, fract: f32) -> Vec<u8> {
    let mut dst = src.to_vec();
    let mut f = vec![0f32; w * h];
    let width = 2 * hw + 1;
    // Horizontal low pass, stored in an fpix.
    for i in 0..h {
        let line = &src[i * w..(i + 1) * w];
        for j in hw..w.saturating_sub(hw) {
            let val: i32 = line[j - hw..j - hw + width]
                .iter()
                .map(|&b| i32::from(b))
                .sum();
            f[i * w + j] = val as f32;
        }
    }
    // Vertical low pass, then N = I + fract * (I - L).
    let norm = if hw == 1 {
        (1.0f64 / 9.0) as f32
    } else {
        (1.0f64 / 25.0) as f32
    };
    for i in hw..h.saturating_sub(hw) {
        for j in hw..w.saturating_sub(hw) {
            let mut acc = 0f32;
            for k in 0..width {
                acc += f[(i - hw + k) * w + j];
            }
            let val = norm * acc;
            let sval = i32::from(src[i * w + j]) as f32;
            let ival = c_int(f64::from(sval + fract * (sval - val)) + 0.5).clamp(0, 255);
            dst[i * w + j] = ival as u8;
        }
    }
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: usize, height: usize, data: Vec<u32>) -> Rgb32 {
        Rgb32 {
            width,
            height,
            data,
        }
    }

    /// A deterministic pseudo-random image.
    fn noise(width: usize, height: usize, seed: u32) -> Rgb32 {
        let mut state = seed;
        let data = (0..width * height)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                state
            })
            .collect();
        image(width, height, data)
    }

    #[test]
    fn scale_rgb_by_one_is_identity_including_alpha() {
        let src = noise(7, 5, 3);
        assert_eq!(scale_rgb(&src, 1.0), src);
    }

    #[test]
    fn scale_rgb_output_sizes_follow_leptonica_rounding() {
        let src = noise(60, 40, 1);
        let size = |s: f32| {
            let out = scale_rgb(&src, s);
            assert_eq!(out.data.len(), out.width * out.height);
            (out.width, out.height)
        };
        assert_eq!(size(0.9), (54, 36));
        assert_eq!(size(2.0), (120, 80));
        assert_eq!(size(4.0), (240, 160));
        assert_eq!(size(0.5), (30, 20));
        assert_eq!(size(0.25), (15, 10));
        assert_eq!(size(0.0625), (3, 2));
        // (l_int32)(0.33 * 40 + 0.5) = 13, (l_int32)(0.33 * 60 + 0.5) = 20.
        assert_eq!(size(0.33), (20, 13));
        // 36 / 40 as Tesseract computes it.
        assert_eq!(size(36.0 / 40.0), (54, 36));
        // pixScaleSmooth: isize = 100 > 40 → the 1×1 centre pixel.
        let tiny = scale_rgb(&src, 0.01);
        assert_eq!((tiny.width, tiny.height), (1, 1));
        assert_eq!(tiny.data[0], src.data[20 * 60 + 30]);
    }

    #[test]
    fn scale_rgb_returns_empty_where_leptonica_returns_null() {
        let line = noise(5, 1, 2);
        // Area mapping to height (l_int32)(0.33 + 0.5) = 0.
        assert_eq!(scale_rgb(&line, 0.33).width, 0);
        // 2x area map of a 1-pixel-high image.
        assert_eq!(scale_rgb(&line, 0.5).height, 0);
        assert_eq!(scale_rgb(&noise(3, 3, 1), 0.25).width, 0);
        assert_eq!(scale_rgb(&line, 0.0).width, 0);
        assert_eq!(scale_rgb(&line, -1.0).width, 0);
        assert_eq!(scale_rgb(&line, f32::NAN).width, 0);
        assert_eq!(scale_rgb(&line, f32::INFINITY).width, 0);
        assert_eq!(scale_rgb(&image(0, 0, Vec::new()), 2.0).width, 0);
        assert_eq!(scale_rgb(&image(2, 2, vec![0; 3]), 2.0).width, 0);
    }

    #[test]
    fn scale_rgb_never_panics_on_tiny_images() {
        let scales = [
            0.001, 0.019, 0.02, 0.0625, 0.125, 0.2, 0.21, 0.25, 0.5, 0.69, 0.7, 0.9, 1.0, 1.39,
            1.4, 2.0, 3.0, 4.0, 36.0,
        ];
        for w in 1..6 {
            for h in 1..6 {
                let src = noise(w, h, (w * 10 + h) as u32);
                for &s in &scales {
                    let out = scale_rgb(&src, s);
                    assert_eq!(out.data.len(), out.width * out.height);
                }
            }
        }
    }

    #[test]
    fn scale_rgb_2x_matches_leptonica_on_2x2() {
        // pixScale(pix, 2.0, 2.0) in Leptonica 1.82.0 (no sharpening at 2.0).
        let src = image(
            2,
            2,
            vec![0xff00_0011, 0x00ff_0022, 0x0000_ff33, 0x8040_2044],
        );
        let expected = [
            0xff00_0000,
            0x7f7f_0000,
            0x00ff_0000,
            0x00ff_0000,
            0x7f00_7f00,
            0x5f4f_4700,
            0x409f_1000,
            0x409f_1000,
            0x0000_ff00,
            0x4020_8f00,
            0x8040_2000,
            0x8040_2000,
            0x0000_ff00,
            0x4020_8f00,
            0x8040_2000,
            0x8040_2000,
        ];
        let out = scale_rgb(&src, 2.0);
        assert_eq!((out.width, out.height), (4, 4));
        assert_eq!(out.data, expected);
    }

    #[test]
    fn rgb_to_luminance_uses_leptonica_weights() {
        // Values from pixConvertRGBToLuminance in Leptonica 1.82.0.
        let src = image(
            5,
            1,
            vec![
                0xff00_0000,
                0x00ff_0000,
                0x0000_ffff,
                0xffff_ff00,
                0x0102_0300,
            ],
        );
        let grey = rgb_to_luminance(&src);
        assert_eq!((grey.width, grey.height), (5, 1));
        assert_eq!(grey.data, [77, 128, 51, 255, 2]);
    }
}
