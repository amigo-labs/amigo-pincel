//! The x-height, ascender and descender estimate of rows and blocks
//! (`src/textord/makerow.cpp` `compute_row_xheight`,
//! `compute_block_xheight`, `correct_row_xheight` and helpers).

use super::makerow::{MIN_BLOB_HEIGHT_FRACTION, ToRow};
use super::stats::Stats;
use super::tordmain::{
    ASCENDER_FRACTION, DESCENDER_FRACTION, ToBlock, XHEIGHT_CAP_RATIO, XHEIGHT_FRACTION,
};

/// `textord_min_xheight`.
const MIN_XHEIGHT: i32 = 10;
/// `textord_minxh`.
const MIN_XH: f64 = 0.25;
const ASCX_RATIO_MIN: f64 = 1.25;
const ASCX_RATIO_MAX: f64 = 1.8;
const DESCX_RATIO_MIN: f64 = 0.25;
const DESCX_RATIO_MAX: f64 = 0.6;
const XHEIGHT_MODE_FRACTION: f64 = 0.4;
const ASCHEIGHT_MODE_FRACTION: f64 = 0.08;
const DESCHEIGHT_MODE_FRACTION: f64 = 0.08;
const XHEIGHT_ERROR_MARGIN: f32 = 0.1;
/// `MAX_HEIGHT_MODES`.
const MAX_HEIGHT_MODES: usize = 12;

/// `get_min_max_xheight`.
fn min_max_xheight(line_size: i32) -> (i32, i32) {
    let min = ((f64::from(line_size) * MIN_XH).floor() as i32).max(MIN_XHEIGHT);
    let max = (f64::from(line_size) * 3.0).ceil() as i32;
    (min, max)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Invalid,
    Ascenders,
    Descenders,
    Unknown,
}

/// `get_row_category`.
fn category(row: &ToRow) -> Category {
    if row.xheight <= 0.0 {
        Category::Invalid
    } else if row.ascrise > 0.0 {
        Category::Ascenders
    } else if row.descdrop != 0.0 {
        Category::Descenders
    } else {
        Category::Unknown
    }
}

/// `within_error_margin`.
fn within_margin(test: f32, num: f32) -> bool {
    test >= num * (1.0 - XHEIGHT_ERROR_MARGIN) && test <= num * (1.0 + XHEIGHT_ERROR_MARGIN)
}

/// `fill_heights`: blob tops above the spline baseline.
fn fill_heights(row: &ToRow, min: i32, max: i32) -> (Stats, Stats) {
    let mut heights = Stats::new(min, max);
    let mut floating = Stats::new(min, max);
    for b in row.blobs.iter().filter(|b| !b.joined) {
        let xcentre = (b.bbox.left + b.bbox.right) as f32 / 2.0;
        let height = b.bbox.height() as f32;
        let top = (f64::from(b.bbox.top as f32) - row.baseline.y(f64::from(xcentre))) as f32;
        if top >= min as f32 && top <= max as f32 {
            let v = (f64::from(top) + 0.5).floor() as i32;
            heights.add(v, 1);
            if f64::from(height / top) < MIN_BLOB_HEIGHT_FRACTION {
                floating.add(v, 1);
            }
        }
    }
    (heights, floating)
}

/// `compute_height_modes`.
fn height_modes(heights: &Stats, min: i32, max: i32) -> Vec<i32> {
    let mut modes = [0i32; MAX_HEIGHT_MODES];
    let mut dest = 0usize;
    let mut least_count = i32::MAX;
    let mut least_index = 0usize;
    for v in min..=max {
        let pile = heights.pile_count(v);
        if pile <= 0 {
            continue;
        }
        if dest < MAX_HEIGHT_MODES {
            if pile < least_count {
                least_count = pile;
                least_index = dest;
            }
            modes[dest] = v;
            dest += 1;
        } else if pile >= least_count {
            while least_index < MAX_HEIGHT_MODES - 1 {
                modes[least_index] = modes[least_index + 1];
                least_index += 1;
            }
            modes[MAX_HEIGHT_MODES - 1] = v;
            if pile == least_count {
                least_index = MAX_HEIGHT_MODES - 1;
            } else {
                least_count = heights.pile_count(modes[0]);
                least_index = 0;
                for (i, &m) in modes.iter().enumerate().skip(1) {
                    let c = heights.pile_count(m);
                    if c < least_count {
                        least_count = c;
                        least_index = i;
                    }
                }
            }
        }
    }
    modes[..dest].to_vec()
}

/// `compute_xheight_from_modes`: returns `(evidence, xheight, ascrise)`;
/// `xheight` and `ascrise` start from the given values.
fn xheight_from_modes(
    heights: &mut Stats,
    floating: &Stats,
    min: i32,
    max: i32,
    mut xheight: f32,
    mut ascrise: f32,
) -> (i32, f32, f32) {
    let mut blob_index = heights.mode();
    let blob_count = heights.pile_count(blob_index);
    if blob_count == 0 {
        return (0, xheight, ascrise);
    }
    let modes = height_modes(heights, min, max);
    let mut in_best_pile = false;
    let mut prev_size = -i32::MAX;
    let mut best_count = 0;
    for x in 0..modes.len().saturating_sub(1) {
        if modes[x] != prev_size + 1 {
            in_best_pile = false;
        }
        let modes_x_count = heights.pile_count(modes[x]) - floating.pile_count(modes[x]);
        if f64::from(modes_x_count) >= f64::from(blob_count) * XHEIGHT_MODE_FRACTION
            && (in_best_pile || modes_x_count > best_count)
        {
            for asc in x + 1..modes.len() {
                let ratio = modes[asc] as f32 / modes[x] as f32;
                if ASCX_RATIO_MIN < f64::from(ratio)
                    && f64::from(ratio) < ASCX_RATIO_MAX
                    && f64::from(heights.pile_count(modes[asc]))
                        >= f64::from(blob_count) * ASCHEIGHT_MODE_FRACTION
                {
                    if modes_x_count > best_count {
                        in_best_pile = true;
                        best_count = modes_x_count;
                    }
                    prev_size = modes[x];
                    xheight = modes[x] as f32;
                    ascrise = (modes[asc] - modes[x]) as f32;
                }
            }
        }
    }
    if xheight == 0.0 {
        if floating.total() > 0 {
            for x in min..max {
                heights.add(x, -floating.pile_count(x));
            }
            blob_index = heights.mode();
            for x in min..max {
                heights.add(x, floating.pile_count(x));
            }
        }
        xheight = blob_index as f32;
        ascrise = 0.0;
        best_count = heights.pile_count(blob_index);
    }
    (best_count, xheight, ascrise)
}

/// `compute_row_descdrop`.
fn row_descdrop(row: &ToRow, gradient: f32, evidence: i32, asc: &Stats) -> f32 {
    let xheight = row.xheight;
    let mut i_min = asc.min_bucket();
    if f64::from(i_min as f32 / xheight) < ASCX_RATIO_MIN {
        i_min = (f64::from(xheight) * ASCX_RATIO_MIN + 0.5).floor() as i32;
    }
    let mut i_max = asc.max_bucket();
    if f64::from(i_max as f32 / xheight) > ASCX_RATIO_MAX {
        i_max = (f64::from(xheight) * ASCX_RATIO_MAX).floor() as i32;
    }
    let num_potential_asc: i32 = (i_min..=i_max).map(|i| asc.pile_count(i)).sum();
    let min = (f64::from(xheight) * DESCX_RATIO_MIN + 0.5).floor() as i32;
    let max = (f64::from(xheight) * DESCX_RATIO_MAX).floor() as i32;
    let mut heights = Stats::new(min, max);
    for b in row.blobs.iter().filter(|b| !b.joined) {
        let xcentre = (b.bbox.left + b.bbox.right) as f32 / 2.0;
        let height = gradient * xcentre + row.para_c - b.bbox.bottom as f32;
        if height >= min as f32 && height <= max as f32 {
            heights.add((f64::from(height) + 0.5).floor() as i32, 1);
        }
    }
    let index = heights.mode();
    let mut count = heights.pile_count(index);
    let total_fraction = (DESCHEIGHT_MODE_FRACTION + ASCHEIGHT_MODE_FRACTION) as f32;
    if ((count + num_potential_asc) as f32) < evidence as f32 * total_fraction {
        count = 0;
    }
    if count > 0 { -(index as f32) } else { 0.0 }
}

/// `Textord::compute_row_xheight`.
pub(crate) fn compute_row_xheight(row: &mut ToRow, gradient: f32, line_size: i32) {
    let (min, max) = min_max_xheight(line_size);
    let (mut heights, floating) = fill_heights(row, min, max);
    let (evidence, xheight, ascrise) =
        xheight_from_modes(&mut heights, &floating, min, max, 0.0, 0.0);
    row.xheight = xheight;
    row.ascrise = ascrise;
    row.xheight_evidence = evidence;
    row.descdrop = 0.0;
    if row.xheight > 0.0 {
        row.descdrop = row_descdrop(row, gradient, evidence, &heights);
    }
}

/// `Textord::compute_block_xheight`: the block's estimate from its rows,
/// then `correct_row_xheight` on each row.
pub(crate) fn compute_block_xheight(block: &mut ToBlock, gradient: f32) {
    if block.rows.is_empty() {
        return;
    }
    let asc_frac = (ASCENDER_FRACTION / XHEIGHT_FRACTION) as f32;
    let desc_frac = (DESCENDER_FRACTION / XHEIGHT_FRACTION) as f32;
    let line_size = block.line_size as i32;
    let (min, max) = min_max_xheight(line_size);
    let mut asc_xheights = Stats::new(min, max);
    let mut asc_ascrise = Stats::new(
        (min as f32 * asc_frac) as i32,
        (max as f32 * asc_frac) as i32,
    );
    let min_desc = (min as f32 * desc_frac) as i32;
    let max_desc = (max as f32 * desc_frac) as i32;
    let mut asc_descdrop = Stats::new(min_desc, max_desc);
    let mut desc_xheights = Stats::new(min, max);
    let mut desc_descdrop = Stats::new(min_desc, max_desc);
    let mut cap_xheights = Stats::new(min, max);
    let mut cap_floating = Stats::new(min, max);
    for row in &mut block.rows {
        if row.xheight <= 0.0 {
            compute_row_xheight(row, gradient, line_size);
        }
        match category(row) {
            Category::Ascenders => {
                asc_xheights.add(row.xheight as i32, row.xheight_evidence);
                asc_ascrise.add(row.ascrise as i32, row.xheight_evidence);
                asc_descdrop.add((-row.descdrop) as i32, row.xheight_evidence);
            }
            Category::Descenders => {
                desc_xheights.add(row.xheight as i32, row.xheight_evidence);
                desc_descdrop.add((-row.descdrop) as i32, row.xheight_evidence);
            }
            Category::Unknown => {
                let (h, f) = fill_heights(row, min, max);
                for v in min..=max {
                    cap_xheights.add(v, h.pile_count(v));
                    cap_floating.add(v, f.pile_count(v));
                }
            }
            Category::Invalid => {}
        }
    }
    let (mut xheight, mut ascrise, mut descdrop) = (0.0f32, 0.0f32, 0.0f32);
    if asc_xheights.total() > 0 {
        xheight = asc_xheights.median() as f32;
        ascrise = asc_ascrise.median() as f32;
        descdrop = -(asc_descdrop.median() as f32);
    } else if desc_xheights.total() > 0 {
        xheight = desc_xheights.median() as f32;
        descdrop = -(desc_descdrop.median() as f32);
    } else if cap_xheights.total() > 0 {
        let (_, xh, asc) =
            xheight_from_modes(&mut cap_xheights, &cap_floating, min, max, xheight, ascrise);
        xheight = xh;
        ascrise = asc;
        if ascrise == 0.0 {
            xheight = (cap_xheights.median() * XHEIGHT_CAP_RATIO) as f32;
        }
    } else {
        xheight = (f64::from(block.line_size) * XHEIGHT_FRACTION) as f32;
    }
    let mut corrected = false;
    if xheight < MIN_XHEIGHT as f32 {
        xheight = MIN_XHEIGHT as f32;
        corrected = true;
    }
    if corrected || ascrise <= 0.0 {
        ascrise = xheight * asc_frac;
    }
    if corrected || descdrop >= 0.0 {
        descdrop = -(xheight * desc_frac);
    }
    block.xheight = xheight;
    for row in &mut block.rows {
        correct_row_xheight(row, xheight, ascrise, descdrop);
    }
}

/// `correct_row_xheight`.
fn correct_row_xheight(row: &mut ToRow, xheight: f32, ascrise: f32, descdrop: f32) {
    let cat = category(row);
    let normal = within_margin(row.xheight, xheight);
    let cap = within_margin(row.xheight, xheight + ascrise);
    match cat {
        Category::Ascenders => {
            if row.descdrop >= 0.0 {
                row.descdrop = row.xheight * (descdrop / xheight);
            }
        }
        Category::Invalid => {
            row.xheight = xheight;
            row.ascrise = ascrise;
            row.descdrop = descdrop;
        }
        Category::Descenders if normal || cap => {
            row.xheight = xheight;
            row.ascrise = ascrise;
            row.descdrop = descdrop;
        }
        Category::Unknown if normal => {
            row.xheight = xheight;
            row.ascrise = ascrise;
            row.descdrop = descdrop;
        }
        Category::Descenders => {
            row.ascrise = row.xheight * (ascrise / xheight);
        }
        Category::Unknown => {
            if cap {
                row.xheight = xheight;
                row.ascrise = ascrise;
                row.descdrop = descdrop;
            } else {
                row.ascrise = row.xheight * (ascrise / (xheight + ascrise));
                row.xheight -= row.ascrise;
                row.descdrop = row.xheight * (descdrop / xheight);
            }
        }
    }
}
