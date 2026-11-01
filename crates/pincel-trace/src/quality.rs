//! Otsu threshold, despeckle and CIELAB, ported from the reference
//! implementation's `quality.ts` (`docs/specs/vectorize-core.md`).

use crate::error::{TraceError, try_vec};
use crate::potrace::js_round;

/// A CIELAB colour (D65).
pub(crate) type Lab = [f64; 3];

/// Brightness of an RGBA pixel blended towards white by its transparency.
pub(crate) fn effective_brightness(px: &[u8]) -> f64 {
    let a = f64::from(px[3]) / 255.0;
    let lum = 0.299 * f64::from(px[0]) + 0.587 * f64::from(px[1]) + 0.114 * f64::from(px[2]);
    lum * a + 255.0 * (1.0 - a)
}

/// Otsu threshold over the effective brightness; the middle of a plateau
/// of equal variances, found by exact comparison (an epsilon is meaningless
/// at variances around 1e12).
pub(crate) fn otsu(d: &[u8]) -> u16 {
    let mut hist = [0.0_f64; 256];
    let mut total = 0.0_f64;
    for px in d.chunks_exact(4) {
        let eff = js_round(effective_brightness(px));
        hist[eff as usize] += 1.0;
        total += 1.0;
    }
    if total == 0.0 {
        return 128;
    }
    let mut sum = 0.0;
    for (t, &hv) in hist.iter().enumerate() {
        sum += t as f64 * hv;
    }
    let (mut sum_b, mut w_b, mut max_var) = (0.0, 0.0, -1.0);
    let (mut first, mut last) = (128_u16, 128_u16);
    for (t, &hv) in hist.iter().enumerate() {
        w_b += hv;
        if w_b == 0.0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0.0 {
            break;
        }
        sum_b += t as f64 * hv;
        let m_b = sum_b / w_b;
        let m_f = (sum - sum_b) / w_f;
        let v = w_b * w_f * (m_b - m_f) * (m_b - m_f);
        if v > max_var {
            max_var = v;
            first = t as u16;
            last = t as u16;
        } else if v == max_var {
            last = t as u16;
        }
    }
    ((first + last) >> 1) + 1
}

fn median9(mut v: [u8; 9]) -> u8 {
    v.sort_unstable();
    v[4]
}

/// 3 × 3 median of R, G and B; alpha and the border are copied unchanged.
pub(crate) fn despeckle(d: &[u8], w: usize, h: usize) -> Result<Vec<u8>, TraceError> {
    let mut out = try_vec(d.len(), 0_u8)?;
    for y in 0..h {
        for x in 0..w {
            let o = (y * w + x) * 4;
            if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                out[o..o + 4].copy_from_slice(&d[o..o + 4]);
                continue;
            }
            let (mut vr, mut vg, mut vb) = ([0_u8; 9], [0_u8; 9], [0_u8; 9]);
            let mut n = 0;
            for dy in 0..3 {
                for dx in 0..3 {
                    let p = ((y + dy - 1) * w + (x + dx - 1)) * 4;
                    vr[n] = d[p];
                    vg[n] = d[p + 1];
                    vb[n] = d[p + 2];
                    n += 1;
                }
            }
            out[o] = median9(vr);
            out[o + 1] = median9(vg);
            out[o + 2] = median9(vb);
            out[o + 3] = d[o + 3];
        }
    }
    Ok(out)
}

fn srgb_to_linear(c: f64) -> f64 {
    let v = c / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB (0 – 255 per channel) → CIELAB (D65).
pub(crate) fn rgb_to_lab(r: f64, g: f64, b: f64) -> Lab {
    let lr = srgb_to_linear(r);
    let lg = srgb_to_linear(g);
    let lb = srgb_to_linear(b);
    let x = (0.4124 * lr + 0.3576 * lg + 0.1805 * lb) / 0.95047;
    let y = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
    let z = (0.0193 * lr + 0.1192 * lg + 0.9505 * lb) / 1.08883;
    let f = |t: f64| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Squared CIELAB distance (ΔE76²).
pub(crate) fn lab_dist2(a: &Lab, b: &Lab) -> f64 {
    let dl = a[0] - b[0];
    let da = a[1] - b[1];
    let db = a[2] - b[2];
    dl * dl + da * da + db * db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otsu_degenerate_inputs_do_not_crash() {
        assert_eq!(otsu(&[]), 128);
        let uni: Vec<u8> = [128, 128, 128, 255].repeat(100);
        assert!(otsu(&uni) > 0);
    }

    #[test]
    fn lab_separates_black_from_dark_blue_more_than_navy() {
        let black = rgb_to_lab(0.0, 0.0, 0.0);
        let darkblue = rgb_to_lab(0.0, 0.0, 80.0);
        let navy = rgb_to_lab(20.0, 20.0, 100.0);
        assert!(lab_dist2(&black, &darkblue) > lab_dist2(&darkblue, &navy) * 10.0);
    }

    #[test]
    fn despeckle_keeps_tiny_images_unchanged() {
        let d: Vec<u8> = (0..16).collect();
        assert_eq!(despeckle(&d, 2, 2).expect("despeckle"), d);
        let row: Vec<u8> = (0..28).collect();
        assert_eq!(despeckle(&row, 7, 1).expect("despeckle"), row);
    }
}
