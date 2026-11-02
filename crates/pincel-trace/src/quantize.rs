//! Colour reduction, ported from the reference implementation's
//! `quantize.ts` (`docs/specs/vectorize-core.md`, Technical notes).
//!
//! Median cut runs over the histogram of distinct colour bins, not over
//! pixels: over pixels a dominant background eats the colour budget and
//! small colours vanish. Assignment and duplicate merging work in CIELAB,
//! where black and dark blue are far apart (in RGB they look close).

use std::collections::HashMap;

use crate::color::Rgb;
use crate::error::{TraceError, try_vec};
use crate::potrace::js_round;
use crate::quality::{Lab, lab_dist2, rgb_to_lab};

/// Palette, per-pixel palette index (−1: transparent) and area per colour.
#[derive(Clone, Debug)]
pub(crate) struct Quantization {
    pub idx: Vec<i16>,
    pub palette: Vec<Rgb>,
    pub area: Vec<u64>,
}

#[derive(Clone, Copy)]
struct HistEntry {
    r: f64,
    g: f64,
    b: f64,
    cnt: f64,
}

struct ColorBox {
    e: Vec<HistEntry>,
    range: [f64; 3],
    avg: Rgb,
}

fn bin_key(r: u8, g: u8, b: u8) -> usize {
    (usize::from(r >> 3) << 10) | (usize::from(g >> 3) << 5) | usize::from(b >> 3)
}

fn mkbox(e: Vec<HistEntry>) -> ColorBox {
    let (mut rmn, mut rmx, mut gmn, mut gmx, mut bmn, mut bmx) =
        (255.0, 0.0, 255.0, 0.0, 255.0, 0.0);
    let (mut sr, mut sg, mut sb, mut sc) = (0.0, 0.0, 0.0, 0.0);
    for p in &e {
        if p.r < rmn {
            rmn = p.r;
        }
        if p.r > rmx {
            rmx = p.r;
        }
        if p.g < gmn {
            gmn = p.g;
        }
        if p.g > gmx {
            gmx = p.g;
        }
        if p.b < bmn {
            bmn = p.b;
        }
        if p.b > bmx {
            bmx = p.b;
        }
        sr += p.r * p.cnt;
        sg += p.g * p.cnt;
        sb += p.b * p.cnt;
        sc += p.cnt;
    }
    if sc == 0.0 {
        sc = 1.0;
    }
    let avg = Rgb::new(
        js_round(sr / sc) as u8,
        js_round(sg / sc) as u8,
        js_round(sb / sc) as u8,
    );
    ColorBox {
        e,
        range: [rmx - rmn, gmx - gmn, bmx - bmn],
        avg,
    }
}

/// At most `k` colours; near-duplicates (ΔE < 2.5) merged.
pub(crate) fn build_palette(d: &[u8], k: usize) -> Vec<Rgb> {
    // Insertion order matters: the reference iterates a JavaScript Map.
    let mut slots: HashMap<usize, usize> = HashMap::new();
    let mut sums: Vec<[f64; 4]> = Vec::new();
    for px in d.as_chunks::<4>().0 {
        if px[3] < 128 {
            continue;
        }
        let key = bin_key(px[0], px[1], px[2]);
        let slot = *slots.entry(key).or_insert_with(|| {
            sums.push([0.0; 4]);
            sums.len() - 1
        });
        let s = &mut sums[slot];
        s[0] += 1.0;
        s[1] += f64::from(px[0]);
        s[2] += f64::from(px[1]);
        s[3] += f64::from(px[2]);
    }
    if sums.is_empty() {
        return Vec::new();
    }
    let entries: Vec<HistEntry> = sums
        .iter()
        .map(|s| HistEntry {
            r: s[1] / s[0],
            g: s[2] / s[0],
            b: s[3] / s[0],
            cnt: s[0],
        })
        .collect();
    let mut boxes = vec![mkbox(entries)];
    while boxes.len() < k {
        let mut bi: Option<usize> = None;
        let mut best = -1.0;
        for (i, bx) in boxes.iter().enumerate() {
            if bx.e.len() < 2 {
                continue;
            }
            let rr = bx.range[0].max(bx.range[1]).max(bx.range[2]);
            if rr > best {
                best = rr;
                bi = Some(i);
            }
        }
        let Some(bi) = bi else { break };
        let mut bx = boxes.remove(bi);
        let [rr, rg, rb] = bx.range;
        let ch = if rr >= rg && rr >= rb {
            0
        } else if rg >= rb {
            1
        } else {
            2
        };
        let key = |p: &HistEntry| match ch {
            0 => p.r,
            1 => p.g,
            _ => p.b,
        };
        // Stable, like JavaScript's Array.prototype.sort.
        bx.e.sort_by(|p, q| key(p).total_cmp(&key(q)));
        let m = bx.e.len() >> 1;
        let hi = bx.e.split_off(m);
        boxes.insert(bi, mkbox(hi));
        boxes.insert(bi, mkbox(bx.e));
    }
    let mut pal: Vec<Rgb> = Vec::new();
    let mut labs: Vec<Lab> = Vec::new();
    for bx in &boxes {
        let c = bx.avg;
        let cl = rgb_to_lab(f64::from(c.r), f64::from(c.g), f64::from(c.b));
        if !labs.iter().any(|l| lab_dist2(l, &cl) < 6.25) {
            pal.push(c);
            labs.push(cl);
        }
    }
    pal
}

fn nearest_lab(pal_lab: &[Lab], lab: &Lab) -> usize {
    let mut bi = 0;
    let mut bd = f64::INFINITY;
    for (i, l) in pal_lab.iter().enumerate() {
        let dd = lab_dist2(l, lab);
        if dd < bd {
            bd = dd;
            bi = i;
        }
    }
    bi
}

/// Assigns every pixel with alpha ≥ 128 to a palette colour: per colour
/// bin, the colour nearest in CIELAB to the bin's first pixel decides.
pub(crate) fn quantize_with_palette(
    d: &[u8],
    palette: Vec<Rgb>,
) -> Result<Quantization, TraceError> {
    let mut idx = try_vec(d.len() / 4, -1_i16)?;
    let mut area = vec![0_u64; palette.len()];
    if palette.is_empty() {
        return Ok(Quantization { idx, palette, area });
    }
    let pal_lab: Vec<Lab> = palette
        .iter()
        .map(|c| rgb_to_lab(f64::from(c.r), f64::from(c.g), f64::from(c.b)))
        .collect();
    let mut lut = vec![-1_i16; 32_768];
    for (p, px) in d.as_chunks::<4>().0.iter().enumerate() {
        if px[3] < 128 {
            continue;
        }
        let key = bin_key(px[0], px[1], px[2]);
        let mut j = lut[key];
        if j < 0 {
            let lab = rgb_to_lab(f64::from(px[0]), f64::from(px[1]), f64::from(px[2]));
            j = nearest_lab(&pal_lab, &lab) as i16;
            lut[key] = j;
        }
        idx[p] = j;
        area[j as usize] += 1;
    }
    Ok(Quantization { idx, palette, area })
}

/// Palette of at most `k` colours plus the assignment.
pub(crate) fn quantize_image(d: &[u8], k: usize) -> Result<Quantization, TraceError> {
    quantize_with_palette(d, build_palette(d, k))
}
