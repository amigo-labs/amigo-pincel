//! Blob finding and noise filtering (`src/textord/tordmain.cpp`).

use super::blob::{CBlob, outlines_to_blobs};
use super::blobbox::BlobBox;
use super::makerow::ToRow;
use super::outline::{Outline, extract_outlines};
use super::quspline::QSpline;
use super::stats::Stats;
use super::tospace::{Row, Werd};

/// `textord_max_noise_size`.
const MAX_NOISE_SIZE: i32 = 7;
/// `textord_noise_area_ratio`.
const NOISE_AREA_RATIO: f64 = 0.7;
/// `textord_initialx_ile`.
const INITIAL_X_ILE: f64 = 0.75;
/// `textord_initialasc_ile`.
const INITIAL_ASC_ILE: f64 = 0.90;
/// `textord_width_limit`.
const WIDTH_LIMIT: f64 = 8.0;
/// `textord_min_linesize`.
const MIN_LINESIZE: f64 = 1.25;
/// `textord_excess_blobsize`.
const EXCESS_BLOBSIZE: f64 = 1.3;
/// `MAX_NEAREST_DIST`.
const MAX_NEAREST_DIST: i32 = 600;

/// `CCStruct::kDescenderFraction`.
pub(crate) const DESCENDER_FRACTION: f64 = 0.25;
/// `CCStruct::kXHeightFraction`.
pub(crate) const XHEIGHT_FRACTION: f64 = 0.5;
/// `CCStruct::kAscenderFraction`.
pub(crate) const ASCENDER_FRACTION: f64 = 0.25;
/// `CCStruct::kXHeightCapRatio`.
pub(crate) const XHEIGHT_CAP_RATIO: f64 = XHEIGHT_FRACTION / (XHEIGHT_FRACTION + ASCENDER_FRACTION);

/// The blobs of the page's single block (`TO_BLOCK`) after
/// `find_components`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToBlock {
    pub(crate) blobs: Vec<BlobBox>,
    pub(crate) small: Vec<BlobBox>,
    pub(crate) noise: Vec<BlobBox>,
    pub(crate) large: Vec<BlobBox>,
    pub(crate) underlines: Vec<BlobBox>,
    pub(crate) line_size: f32,
    pub(crate) line_spacing: f32,
    pub(crate) max_blob_size: f32,
    pub(crate) xheight: f32,
    pub(crate) rows: Vec<ToRow>,
}

/// `Textord::find_components` for a page that is one block: outlines,
/// blobs, then `filter_blobs`.
pub(crate) fn find_components(black: &[bool], width: usize, height: usize) -> ToBlock {
    let outlines = extract_outlines(black, width, height);
    let (good, bad) = outlines_to_blobs(outlines, width as i32, height as i32);
    let mut block = ToBlock {
        blobs: good.into_iter().map(BlobBox::new).collect(),
        noise: bad.into_iter().map(BlobBox::new).collect(),
        ..ToBlock::default()
    };
    filter_blobs(&mut block);
    block
}

/// `Textord::filter_blobs` for one block.
fn filter_blobs(block: &mut ToBlock) {
    let mut line_size = filter_noise_blobs(block);
    if line_size == 0.0 {
        line_size = 1.0;
    }
    block.line_spacing = (f64::from(line_size)
        * (DESCENDER_FRACTION + XHEIGHT_FRACTION + 2.0 * ASCENDER_FRACTION)
        / XHEIGHT_FRACTION) as f32;
    block.line_size = (f64::from(line_size) * MIN_LINESIZE) as f32;
    block.max_blob_size = (f64::from(block.line_size) * EXCESS_BLOBSIZE) as f32;
}

/// `Textord::filter_noise_blobs`: moves noise, small and large blobs to
/// their lists, keeping Tesseract's list order, and returns the initial
/// x-height estimate.
fn filter_noise_blobs(block: &mut ToBlock) -> f32 {
    let src = std::mem::take(&mut block.blobs);
    let noise = &mut block.noise;
    let small = &mut block.small;
    let large = &mut block.large;
    // The noise iterator starts on the first rejected blob, so new noise
    // goes in after it.
    let mut noise_pos = usize::from(!noise.is_empty());
    let mut kept = Vec::with_capacity(src.len());
    for b in src {
        let h = b.bbox.height();
        if h < MAX_NOISE_SIZE {
            noise.insert(noise_pos, b);
            noise_pos += 1;
        } else if f64::from(b.area) >= f64::from(h * b.bbox.width()) * NOISE_AREA_RATIO {
            small.push(b);
        } else {
            kept.push(b);
        }
    }
    let mut src = kept;
    let mut size_stats = Stats::new(0, MAX_NEAREST_DIST - 1);
    for b in &src {
        size_stats.add(b.bbox.height(), 1);
    }
    let mut initial_x = size_stats.ile(INITIAL_X_ILE) as f32;
    let max_y = (f64::from(initial_x)
        * (DESCENDER_FRACTION + XHEIGHT_FRACTION + 2.0 * ASCENDER_FRACTION)
        / XHEIGHT_FRACTION)
        .ceil() as f32;
    let min_y = (initial_x / 2.0).floor();
    let max_x = (f64::from(initial_x) * WIDTH_LIMIT).ceil() as f32;
    // Small blobs of a plausible height go back in after the first blob.
    let mut src_pos = usize::from(!src.is_empty());
    let mut moved_any = false;
    let mut small_kept = Vec::with_capacity(small.len());
    for b in std::mem::take(small) {
        let h = b.bbox.height() as f32;
        if h > max_y {
            large.push(b);
        } else if h >= min_y {
            src.insert(src_pos, b);
            src_pos += 1;
            moved_any = true;
        } else {
            small_kept.push(b);
        }
    }
    *small = small_kept;
    // The next pass starts where the source iterator stopped.
    let start = if moved_any { src_pos - 1 } else { 0 };
    let mut small_pos = usize::from(!small.is_empty());
    size_stats.clear();
    let n = src.len();
    let mut remove = vec![false; n];
    for k in 0..n {
        let idx = (start + k) % n;
        let b = &src[idx];
        let (h, w) = (b.bbox.height() as f32, b.bbox.width() as f32);
        if h < min_y {
            small.insert(small_pos, b.clone());
            small_pos += 1;
            remove[idx] = true;
        } else if h > max_y || w > max_x {
            large.push(b.clone());
            remove[idx] = true;
        } else {
            size_stats.add(b.bbox.height(), 1);
        }
    }
    block.blobs = src
        .into_iter()
        .zip(remove)
        .filter_map(|(b, r)| (!r).then_some(b))
        .collect();
    let max_height = (f64::from(size_stats.ile(INITIAL_ASC_ILE) as f32) * XHEIGHT_CAP_RATIO) as f32;
    if max_height > initial_x {
        initial_x = max_height;
    }
    initial_x
}

/// `textord_noise_sizelimit`.
const NOISE_SIZE_LIMIT: f64 = 0.5;
/// `textord_noise_sizefraction`.
const NOISE_SIZE_FRACTION: i32 = 10;
/// `textord_noise_translimit`.
const NOISE_TRANS_LIMIT: i32 = 16;
/// `textord_noise_normratio`.
const NOISE_NORM_RATIO: f64 = 2.0;
/// `textord_noise_syfract`.
const NOISE_SY_FRACT: f64 = 0.2;
/// `textord_noise_sxfract`.
const NOISE_SX_FRACT: f64 = 0.4;
/// `textord_noise_hfract`.
const NOISE_H_FRACT: f64 = 1.0 / 64.0;
/// `textord_noise_sncount`.
const NOISE_SN_COUNT: i32 = 1;
/// `textord_blshift_maxshift`.
const BLSHIFT_MAX_SHIFT: f64 = 0.0;
/// `textord_blshift_xfraction`.
const BLSHIFT_X_FRACTION: f64 = 9.99;

/// `Textord::cleanup_blocks` for the rows of the page's block.
pub(crate) fn cleanup_rows(rows: Vec<Row>) -> Vec<Row> {
    let mut out = Vec::with_capacity(rows.len());
    for mut row in rows {
        clean_small_noise_from_words(&mut row);
        if row.words.is_empty() || clean_noise_from_row(&row) {
            continue;
        }
        clean_noise_from_words(&mut row);
        tweak_row_baseline(&mut row);
        out.push(row);
    }
    out
}

/// `C_OUTLINE::RemoveSmallRecursive` over a list.
fn remove_small(list: &mut Vec<Outline>, min_size: i32) {
    list.retain(|o| !(o.bbox.width() < min_size || o.bbox.height() < min_size));
    for o in list.iter_mut() {
        remove_small(&mut o.children, min_size);
    }
}

/// `Textord::clean_small_noise_from_words`.
fn clean_small_noise_from_words(row: &mut Row) {
    let mut i = 0;
    while i < row.words.len() {
        let word = &mut row.words[i];
        let min_size = (NOISE_H_FRACT * f64::from(word.bounding_box().height()) + 0.5) as i32;
        for b in &mut word.blobs {
            remove_small(&mut b.outlines, min_size);
        }
        word.blobs.retain(|b| !b.outlines.is_empty());
        if word.blobs.is_empty() {
            if let Some(next) = row.words.get_mut(i + 1) {
                next.fuzzy_non = false;
            }
            row.words.remove(i);
        } else {
            i += 1;
        }
    }
}

/// The outline and blob counts `clean_noise_from_row` and
/// `clean_noise_from_words` share: small outlines (dots), normal-size
/// outlines with holes, and blobs with few transitions.
fn noise_counts(word: &Werd, xheight: f32, first_word: bool) -> (i32, i32, i32) {
    let (mut dots, mut supers, mut norms) = (0, 0, 0);
    let xh = f64::from(xheight);
    for (bi, blob) in word.blobs.iter().enumerate() {
        for o in &blob.outlines {
            let b = o.bbox;
            let size = b.width().max(b.height());
            if f64::from(size) < NOISE_SIZE_LIMIT * xh {
                dots += 1;
            }
            if !o.children.is_empty()
                && f64::from(b.height()) < (1.0 + NOISE_SY_FRACT) * xh
                && f64::from(b.height()) > (1.0 - NOISE_SY_FRACT) * xh
                && f64::from(b.width()) < (1.0 + NOISE_SX_FRACT) * xh
                && f64::from(b.width()) > (1.0 - NOISE_SX_FRACT) * xh
            {
                supers += 1;
            }
        }
        let b = blob.bounding_box();
        let size = b.width().max(b.height());
        if f64::from(size) >= NOISE_SIZE_LIMIT * xh && (size as f32) < xheight * 2.0 {
            let threshold = size / NOISE_SIZE_FRACTION;
            if blob.count_transitions(threshold) < NOISE_TRANS_LIMIT {
                norms += 1;
            }
        } else if b.height() as f32 > xheight * 2.0 && (!first_word || bi > 0) {
            dots += 2;
        }
    }
    (dots, supers, norms)
}

/// `Textord::clean_noise_from_row`: whether the row is mostly noise.
fn clean_noise_from_row(row: &Row) -> bool {
    let (mut dots, mut supers, mut norms) = (0, 0, 0);
    for (wi, w) in row.words.iter().enumerate() {
        let (d, s, n) = noise_counts(w, row.xheight, wi == 0);
        dots += d;
        supers += s;
        norms += n;
    }
    let rejected = f64::from(dots) > f64::from(norms) * NOISE_NORM_RATIO && dots > 2;
    supers < NOISE_SN_COUNT && rejected
}

/// `Textord::clean_noise_from_words`: words that are mostly noise move
/// their small outlines to the reject list (`WERD::CleanNoise`).
fn clean_noise_from_words(row: &mut Row) {
    let mut dud = Vec::with_capacity(row.words.len());
    let (mut dud_words, mut ok_words) = (0, 0);
    for (wi, w) in row.words.iter().enumerate() {
        let (dots, supers, norms) = noise_counts(w, row.xheight, wi == 0);
        let norm_count = supers + norms;
        let d = if dots > 2 {
            if f64::from(dots) > f64::from(norm_count) * NOISE_NORM_RATIO * 2.0 {
                2
            } else if f64::from(dots) > f64::from(norm_count) * NOISE_NORM_RATIO {
                1
            } else {
                0
            }
        } else {
            0
        };
        if d == 2 {
            dud_words += 1;
        } else {
            ok_words += 1;
        }
        dud.push(d);
    }
    let threshold = (NOISE_SIZE_LIMIT * f64::from(row.xheight)) as f32;
    for (w, d) in row.words.iter_mut().zip(dud) {
        if d == 2 || (d == 1 && dud_words > ok_words) {
            clean_noise(w, threshold);
        }
    }
}

/// `WERD::CleanNoise`.
fn clean_noise(word: &mut Werd, size_threshold: f32) {
    let mut kept = Vec::with_capacity(word.blobs.len());
    for mut blob in std::mem::take(&mut word.blobs) {
        let mut outlines = Vec::with_capacity(blob.outlines.len());
        for o in blob.outlines {
            let size = o.bbox.width().max(o.bbox.height());
            if (size as f32) < size_threshold {
                word.rej.push(CBlob { outlines: vec![o] });
            } else {
                outlines.push(o);
            }
        }
        blob.outlines = outlines;
        if !blob.outlines.is_empty() {
            kept.push(blob);
        }
    }
    word.blobs = kept;
}

/// `tweak_row_baseline` with `textord_blshift_maxshift` 0: no blob moves
/// the baseline, but the spline is rebuilt at the blobs' segments.
fn tweak_row_baseline(row: &mut Row) {
    let blob_count: usize = row.words.iter().map(|w| w.blobs.len()).sum();
    if blob_count == 0 {
        return;
    }
    let bl = &row.baseline;
    let segs = bl.segments();
    let mut xstarts = vec![0i32; blob_count + segs + 1];
    let mut coeffs = vec![0.0f64; (blob_count + segs) * 3];
    let (mut src, mut dest) = (0usize, 0usize);
    xstarts[0] = bl.xcoords[0];
    let put = |coeffs: &mut [f64], dest: usize, src: usize| {
        let q = bl.quads[src];
        coeffs[dest * 3] = q.a;
        coeffs[dest * 3 + 1] = f64::from(q.b);
        coeffs[dest * 3 + 2] = f64::from(q.c);
    };
    for w in &row.words {
        for b in &w.blobs {
            let bb = b.bounding_box();
            let x_centre = (f64::from(bb.left + bb.right) / 2.0) as f32;
            let mut ydiff = (f64::from(bb.bottom) - f64::from(row.base_line(x_centre))) as f32;
            if ydiff < 0.0 {
                ydiff = -ydiff / row.xheight;
            } else {
                ydiff /= row.xheight;
            }
            if f64::from(ydiff) < BLSHIFT_MAX_SHIFT
                && f64::from(bb.height() as f32 / row.xheight) > BLSHIFT_X_FRACTION
            {
                if xstarts[dest] as f32 >= x_centre {
                    xstarts[dest] = bb.left;
                }
                coeffs[dest * 3] = 0.0;
                coeffs[dest * 3 + 1] = 0.0;
                coeffs[dest * 3 + 2] = f64::from(bb.bottom);
                dest += 1;
                xstarts[dest] = bb.right + 1;
            } else if xstarts[dest] as f32 <= x_centre {
                while bl.xcoords[src + 1] as f32 <= x_centre && src + 1 < segs {
                    if bl.xcoords[src + 1] > xstarts[dest] {
                        put(&mut coeffs, dest, src);
                        dest += 1;
                        xstarts[dest] = bl.xcoords[src + 1];
                    }
                    src += 1;
                }
                put(&mut coeffs, dest, src);
                dest += 1;
                xstarts[dest] = bl.xcoords[src + 1];
            }
        }
    }
    while src < segs && bl.xcoords[src + 1] <= xstarts[dest] {
        src += 1;
    }
    while src < segs {
        put(&mut coeffs, dest, src);
        dest += 1;
        src += 1;
        xstarts[dest] = bl.xcoords[src];
    }
    row.baseline = QSpline::from_coeffs(&xstarts[..=dest], &coeffs[..dest * 3]);
}
