//! Word spacing and word making for proportional text
//! (`src/textord/tospace.cpp` `Textord::to_spacing`, `make_prop_words`),
//! with the words and rows they produce (`WERD`, `ROW`).

use super::blob::CBlob;
use super::blobbox::BlobBox;
use super::makerow::ToRow;
use super::quspline::QSpline;
use super::rect::TBox;
use super::stats::Stats;
use super::tordmain::ToBlock;

/// `MAXSPACING`.
const MAX_SPACING: i32 = 128;
const ENOUGH_SPACE_SAMPLES_FOR_MEDIAN: i32 = 3;
const REDO_KERN_LIMIT: i32 = 10;
const FEW_SAMPLES: i32 = 40;
const SHORT_ROW: i32 = 20;
const OLD_SP_KN_TH_FACTOR: f64 = 2.0;
const NARROW_FRACTION: f64 = 0.3;
const NARROW_ASPECT_RATIO: f64 = 0.48;
const WIDE_FRACTION: f64 = 0.52;
const FUZZY_SPACE_FACTOR: f64 = 0.6;
const FUZZY_SPACE_FACTOR1: f64 = 0.5;
const FUZZY_SPACE_FACTOR2: f64 = 0.72;
const GAP_FACTOR: f64 = 0.83;
const KERN_GAP_FACTOR1: f64 = 2.0;
const KERN_GAP_FACTOR2: f64 = 1.3;
const KERN_GAP_FACTOR3: f64 = 2.5;
const IGNORE_VERY_BIG_GAPS: f64 = 3.5;
const ENOUGH_SMALL_GAPS: f64 = 0.65;
const TABLE_KN_SP_RATIO: f64 = 2.25;
const TABLE_XHT_SP_RATIO: f64 = 0.33;
const TABLE_FUZZY_KN_SP_RATIO: f64 = 3.0;
const FUZZY_KN_FRACTION: f64 = 0.5;
const FUZZY_SP_FRACTION: f64 = 0.5;
const MIN_SANE_KN_SP: f64 = 1.5;
const INIT_GUESS_KN_MULT: f64 = 2.2;
const INIT_GUESS_XHT_MULT: f64 = 0.28;
const MAX_SANE_KN_THRESH: f64 = 5.0;
const LARGE_KERNING: f64 = 0.19;
const SILLY_KN_SP_GAP: f64 = 0.2;
const PASS_WIDE_FUZZ_SP_TO_CONTEXT: f64 = 0.75;

/// `WERD`: a word of blobs.
#[derive(Debug, Clone, Default)]
pub(crate) struct Werd {
    pub(crate) blobs: Vec<CBlob>,
    /// `rej_cblobs`.
    pub(crate) rej: Vec<CBlob>,
    pub(crate) fuzzy_sp: bool,
    pub(crate) fuzzy_non: bool,
}

impl Werd {
    /// `WERD(C_BLOB_LIST*, …)`: blobs whose outlines
    /// disagree about being inverted, or with the majority of the word,
    /// go to the reject list.
    pub(crate) fn new(blobs: Vec<CBlob>) -> Self {
        let mut good = Vec::with_capacity(blobs.len());
        let mut rej = Vec::new();
        let (mut inverted, mut normal) = (0, 0);
        for b in blobs {
            let first = b.outlines.first().is_some_and(|o| o.inverse);
            if b.outlines.iter().any(|o| o.inverse != first) {
                rej.push(b);
            } else {
                if first {
                    inverted += 1;
                } else {
                    normal += 1;
                }
                good.push(b);
            }
        }
        let inverse = inverted > normal;
        let mut blobs = Vec::with_capacity(good.len());
        for b in good {
            if b.outlines.first().is_some_and(|o| o.inverse) != inverse {
                rej.push(b);
            } else {
                blobs.push(b);
            }
        }
        Self {
            blobs,
            rej,
            fuzzy_sp: false,
            fuzzy_non: false,
        }
    }

    /// `WERD::true_bounding_box`.
    pub(crate) fn true_bounding_box(&self) -> TBox {
        self.blobs
            .iter()
            .fold(TBox::default(), |b, c| b.union(&c.bounding_box()))
    }

    /// `WERD::restricted_bounding_box`.
    pub(crate) fn restricted_bounding_box(&self, upper_dots: bool, lower_dots: bool) -> TBox {
        let mut bbox = self.true_bounding_box();
        let (bottom, top) = (bbox.bottom, bbox.top);
        for r in &self.rej {
            let d = r.bounding_box();
            if (upper_dots || d.bottom <= top) && (lower_dots || d.top >= bottom) {
                bbox = bbox.union(&d);
            }
        }
        bbox
    }

    /// `WERD::bounding_box`.
    pub(crate) fn bounding_box(&self) -> TBox {
        self.restricted_bounding_box(true, true)
    }
}

/// `ROW`: a text line of words with its baseline and size.
#[derive(Debug, Clone, Default)]
pub(crate) struct Row {
    pub(crate) words: Vec<Werd>,
    pub(crate) baseline: QSpline,
    pub(crate) xheight: f32,
    pub(crate) ascrise: f32,
    pub(crate) descdrop: f32,
}

impl Row {
    /// `ROW::base_line`.
    pub(crate) fn base_line(&self, x: f32) -> f32 {
        self.baseline.y(f64::from(x)) as f32
    }
}

/// `make_words` for a page of proportional rows: `Textord::to_spacing`
/// then `make_prop_words` per row.
pub(crate) fn make_words(block: &mut ToBlock) -> Vec<Row> {
    to_spacing(block);
    block
        .rows
        .iter_mut()
        .filter(|r| !r.blobs.is_empty())
        .map(make_prop_words)
        .collect()
}

/// `box_next`: the box of the blob at `*i` with the chopped pieces after
/// it; `*i` moves to the next real blob.
fn box_next(blobs: &[BlobBox], i: &mut usize) -> TBox {
    let mut result = blobs[*i].bbox;
    loop {
        *i = (*i + 1) % blobs.len();
        let b = &blobs[*i];
        if b.cblob.is_none() {
            result = result.union(&b.bbox);
        }
        if b.cblob.is_some() && !b.joined {
            break;
        }
    }
    result
}

/// `find_cblob_hlimits`.
fn cblob_hlimits(cblob: &CBlob, bottom: f32, top: f32) -> (f32, f32) {
    let (mut xmin, mut xmax) = (i32::MAX as f32, -i32::MAX as f32);
    for o in &cblob.outlines {
        let mut pos = o.start;
        for s in 0..o.steps.len() {
            let y = pos.1 as f32;
            if y >= bottom && y <= top {
                let x = pos.0 as f32;
                if x < xmin {
                    xmin = x;
                }
                if x > xmax {
                    xmax = x;
                }
            }
            let st = o.step(s);
            pos = (pos.0 + st.0, pos.1 + st.1);
        }
    }
    (xmin, xmax)
}

/// `Textord::reduced_box_for_blob`: the blob's box from the left edge of
/// its part above the baseline to the right edge of its part below the
/// x-height; also returns the left edge above the x-height.
fn reduced_box_for_blob(blob: &BlobBox, row: &ToRow) -> (TBox, i32) {
    let bbox = blob.bbox;
    let Some(cblob) = &blob.cblob else {
        return (TBox::default(), i32::from(i16::MAX));
    };
    let centre = (f64::from(bbox.left + bbox.right) / 2.0) as f32;
    let baseline = row.baseline.y(f64::from(centre)) as f32;
    let (left, right) = cblob_hlimits(
        cblob,
        (f64::from(baseline) + 1.1 * f64::from(row.xheight)) as f32,
        f32::from(i16::MAX),
    );
    let left_above_xht = if left > right {
        i32::from(i16::MAX)
    } else {
        i32::from(left.floor() as i16)
    };
    let (left_limit, junk) = cblob_hlimits(cblob, baseline, f32::from(i16::MAX));
    if left_limit > junk {
        return (TBox::default(), left_above_xht);
    }
    let (junk, right_limit) = cblob_hlimits(cblob, -f32::from(i16::MAX), baseline + row.xheight);
    if junk > right_limit {
        return (TBox::default(), left_above_xht);
    }
    (
        TBox::from_corners(
            (i32::from(left_limit.floor() as i16), bbox.bottom),
            (i32::from(right_limit.ceil() as i16), bbox.top),
        ),
        left_above_xht,
    )
}

/// `Textord::reduced_box_next`, caching the result on the head blob.
fn reduced_box_next(row: &mut ToRow, i: &mut usize) -> TBox {
    let n = row.blobs.len();
    if let Some(r) = row.blobs[*i].reduced {
        loop {
            *i = (*i + 1) % n;
            let b = &row.blobs[*i];
            if b.cblob.is_some() && !b.joined {
                break;
            }
        }
        return r;
    }
    let head = *i;
    let mut full = row.blobs[head].bbox;
    let (mut reduced, mut left_above) = reduced_box_for_blob(&row.blobs[head], row);
    loop {
        *i = (*i + 1) % n;
        let b = &row.blobs[*i];
        if b.cblob.is_none() {
            full = full.union(&b.bbox);
        } else if b.joined {
            let (r, l) = reduced_box_for_blob(b, row);
            reduced = reduced.union(&r);
            left_above = left_above.min(l);
        }
        if b.cblob.is_some() && !b.joined {
            break;
        }
    }
    if !(reduced.width() > 0
        && reduced.left < left_above
        && f64::from(reduced.height()) > 0.7 * f64::from(row.xheight))
    {
        reduced = full;
    }
    row.blobs[head].reduced = Some(reduced);
    reduced
}

/// `Textord::narrow_blob`.
fn narrow_blob(row: &ToRow, b: &TBox) -> bool {
    f64::from(b.width()) <= NARROW_FRACTION * f64::from(row.xheight)
        || f64::from(b.width() as f32 / b.height() as f32) <= NARROW_ASPECT_RATIO
}

/// `Textord::wide_blob`.
fn wide_blob(row: &ToRow, b: &TBox) -> bool {
    f64::from(b.width()) >= WIDE_FRACTION * f64::from(row.xheight)
}

/// `Textord::suspected_punct_blob`.
fn suspected_punct_blob(row: &ToRow, b: &TBox) -> bool {
    let centre = (f64::from(b.right + b.left) / 2.0) as f32;
    let baseline = row.baseline.y(f64::from(centre)) as f32;
    let half = f64::from(baseline) + f64::from(row.xheight) / 2.0;
    f64::from(b.height()) <= 0.66 * f64::from(row.xheight)
        || f64::from(b.top) < half
        || f64::from(b.bottom) > half
}

/// `Textord::ignore_big_gap` for a page of fewer than three rows (an
/// empty gap map).
fn ignore_big_gap(row: &ToRow, left: i16, right: i16) -> bool {
    let gap = right.wrapping_sub(left).wrapping_add(1);
    f64::from(gap) > IGNORE_VERY_BIG_GAPS * f64::from(row.xheight)
}

/// The row's units in order, as `reduced_box_next` walks them.
fn reduced_units(row: &mut ToRow) -> Vec<TBox> {
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        out.push(reduced_box_next(row, &mut i));
        if i == 0 {
            break;
        }
    }
    out
}

/// `Textord::to_spacing`: block and row gap statistics, which set each
/// row's space threshold and fuzzy limits.
fn to_spacing(block: &mut ToBlock) {
    let (block_space, block_non_space) = block_spacing_stats(block);
    for row in &mut block.rows {
        row_spacing_stats(row, block_space, block_non_space);
    }
}

/// `Textord::block_spacing_stats`: returns the block's space and
/// non-space gap widths.
fn block_spacing_stats(block: &mut ToBlock) -> (i16, i16) {
    let mut centre_to_centre = Stats::new(0, MAX_SPACING - 1);
    let mut all_gaps = Stats::new(0, MAX_SPACING - 1);
    let mut space_gaps = Stats::new(0, MAX_SPACING - 1);
    let mut minwidth = MAX_SPACING as i16;
    for row in &mut block.rows {
        if row.blobs.is_empty() {
            continue;
        }
        let units = reduced_units(row);
        let mut prev = units[0];
        if (prev.width() as i16) < minwidth {
            minwidth = prev.width() as i16;
        }
        for b in &units[1..] {
            if (b.width() as i16) < minwidth {
                minwidth = b.width() as i16;
            }
            let (left, right) = (prev.right as i16, b.left as i16);
            let gap = right.wrapping_sub(left);
            if !ignore_big_gap(row, left, right) {
                all_gaps.add(i32::from(gap), 1);
                let c = (i32::from(right) + b.right - (prev.left + i32::from(left))) / 2;
                centre_to_centre.add(i32::from(c as i16), 1);
            }
            prev = *b;
        }
    }
    if all_gaps.total() <= 1 {
        return (-1, minwidth);
    }
    let block_non_space = all_gaps.median().floor() as i16;
    for row in &mut block.rows {
        if row.blobs.is_empty() {
            continue;
        }
        let threshold = (INIT_GUESS_KN_MULT * f64::from(block_non_space))
            .max(INIT_GUESS_XHT_MULT * f64::from(row.xheight)) as f32;
        let units = reduced_units(row);
        let mut prev = units[0];
        for b in &units[1..] {
            let (left, right) = (prev.right as i16, b.left as i16);
            let gap = right.wrapping_sub(left);
            if f32::from(gap) > threshold && !ignore_big_gap(row, left, right) {
                let xh = f64::from(row.xheight);
                let g = f64::from(gap);
                if g > FUZZY_SPACE_FACTOR2 * xh
                    || (g > FUZZY_SPACE_FACTOR1 * xh
                        && !narrow_blob(row, &prev)
                        && !narrow_blob(row, b))
                    || (wide_blob(row, &prev) && wide_blob(row, b))
                {
                    space_gaps.add(i32::from(gap), 1);
                }
            }
            prev = *b;
        }
    }
    let block_space = if space_gaps.total() <= 2 {
        -1
    } else {
        (space_gaps.median().floor() as i16).max(3i16.wrapping_mul(block_non_space))
    };
    (block_space, block_non_space)
}

/// `Textord::row_spacing_stats`.
fn row_spacing_stats(row: &mut ToRow, block_space_in: i16, block_non_space: i16) {
    let mut all_gaps = Stats::new(0, MAX_SPACING - 1);
    let mut cert_space = Stats::new(0, MAX_SPACING - 1);
    let mut all_space = Stats::new(0, MAX_SPACING - 1);
    let mut small_gaps = Stats::new(0, MAX_SPACING - 1);
    let good_block_space = block_space_in > 0;
    let block_space = if good_block_space {
        block_space_in
    } else {
        (row.xheight / 2.0).floor() as i16
    };
    let mut large_gap_count = 0;
    if !row.blobs.is_empty() {
        let real_space_threshold = (i32::from(block_space) + i32::from(block_non_space)) / 2;
        let units = reduced_units(row);
        let mut prev = units[0];
        for b in &units[1..] {
            let (left, right) = (prev.right as i16, b.left as i16);
            let gap = right.wrapping_sub(left);
            if ignore_big_gap(row, left, right) {
                large_gap_count += 1;
            } else {
                if i32::from(gap) >= real_space_threshold {
                    let xh = f64::from(row.xheight);
                    let g = f64::from(gap);
                    if g > FUZZY_SPACE_FACTOR2 * xh
                        || (g > FUZZY_SPACE_FACTOR1 * xh
                            && !narrow_blob(row, &prev)
                            && !narrow_blob(row, b))
                        || (wide_blob(row, &prev) && wide_blob(row, b))
                    {
                        cert_space.add(i32::from(gap), 1);
                    }
                    all_space.add(i32::from(gap), 1);
                } else {
                    small_gaps.add(i32::from(gap), 1);
                }
                all_gaps.add(i32::from(gap), 1);
            }
            prev = *b;
        }
    }
    let suspected_table =
        large_gap_count > 1 || (large_gap_count > 0 && all_gaps.total() <= FEW_SAMPLES);
    if cert_space.total() >= ENOUGH_SPACE_SAMPLES_FOR_MEDIAN
        || ((suspected_table || all_gaps.total() <= SHORT_ROW) && cert_space.total() > 0)
    {
        old_to_method(row, &all_gaps, &cert_space, block_space, block_non_space);
    } else if !isolated_row_stats(row, &all_gaps, suspected_table) {
        if good_block_space {
            row.space_size = f32::from(block_space);
            row.kern_size = if all_gaps.total() > REDO_KERN_LIMIT {
                all_gaps.median() as f32
            } else {
                f32::from(block_non_space)
            };
            row.space_threshold =
                (f64::from(row.space_size + row.kern_size) / OLD_SP_KN_TH_FACTOR).floor() as i32;
        } else {
            old_to_method(row, &all_gaps, &all_space, block_space, block_non_space);
        }
    }
    // tosp_sanity_method 1.
    if f64::from(row.space_size) < MIN_SANE_KN_SP * f64::from(row.kern_size.max(2.5))
        || f64::from(row.space_size - row.kern_size) < SILLY_KN_SP_GAP * f64::from(row.xheight)
    {
        let sane_space = if good_block_space
            && f64::from(block_space) >= MIN_SANE_KN_SP * f64::from(row.kern_size)
        {
            f32::from(block_space)
        } else {
            ((MIN_SANE_KN_SP as f32) * row.kern_size.max(2.5)).max(row.xheight / 2.0)
        };
        row.space_size = sane_space;
        row.space_threshold =
            (f64::from(row.space_size + row.kern_size) / OLD_SP_KN_TH_FACTOR).floor() as i32;
    }
    let sane_threshold = (MAX_SANE_KN_THRESH * f64::from(row.kern_size.max(2.5))).floor() as i32;
    if row.space_threshold > sane_threshold {
        row.space_threshold = sane_threshold;
        if row.space_size <= sane_threshold as f32 {
            row.space_size = row.space_threshold as f32 + 1.0;
        }
    }
    if suspected_table {
        let sane_space = (TABLE_KN_SP_RATIO * f64::from(row.kern_size))
            .max(TABLE_XHT_SP_RATIO * f64::from(row.xheight)) as f32;
        let sane_threshold = (f64::from(sane_space + row.kern_size) / 2.0).floor() as i32;
        if row.space_size < sane_space || row.space_threshold < sane_threshold {
            row.space_threshold = sane_space as i32;
            row.space_size = (row.space_threshold as f32 + 1.0).max(row.xheight);
        }
    }
    row.min_space =
        ((FUZZY_SPACE_FACTOR * f64::from(row.xheight)).ceil() as i32).min(row.space_size as i32);
    if row.min_space <= row.space_threshold {
        row.min_space = row.space_threshold + 1;
    }
    let max_max_nonspace = ((row.space_threshold as f32 + row.kern_size) / 2.0) as i32;
    row.max_nonspace = max_max_nonspace;
    let mut max = 0;
    for index in 0..=max_max_nonspace {
        if all_gaps.pile_count(index) > max {
            max = all_gaps.pile_count(index);
        }
        if index as f32 > row.kern_size
            && f64::from(all_gaps.pile_count(index)) < 0.1 * f64::from(max)
        {
            row.max_nonspace = index;
            break;
        }
    }
    if row.space_size > row.space_threshold as f32 {
        row.min_space = row.min_space.max(
            (f64::from(row.space_threshold)
                + FUZZY_SP_FRACTION * f64::from(row.space_size - row.space_threshold as f32))
            .ceil() as i32,
        );
    }
    row.min_space = row
        .min_space
        .max((TABLE_FUZZY_KN_SP_RATIO * f64::from(row.kern_size)).ceil() as i32);
    if row.kern_size < row.space_threshold as f32 {
        row.max_nonspace = (0.5
            + f64::from(row.kern_size)
            + FUZZY_KN_FRACTION * f64::from(row.space_threshold as f32 - row.kern_size))
        .floor() as i32;
    }
    if row.max_nonspace > row.space_threshold {
        row.max_nonspace = row.space_threshold;
    }
}

/// `Textord::old_to_method` with the default parameters.
fn old_to_method(
    row: &mut ToRow,
    all_gaps: &Stats,
    space_gaps: &Stats,
    block_space: i16,
    block_non_space: i16,
) {
    if space_gaps.total() >= ENOUGH_SPACE_SAMPLES_FOR_MEDIAN {
        row.space_size = space_gaps.median() as f32;
        if f64::from(row.space_size) > f64::from(block_space) * 1.5 {
            row.space_size = f32::from(block_space);
        }
        if row.space_size < f32::from(block_non_space * 2 + 1) {
            row.space_size = f32::from(block_non_space * 2 + 1);
        }
    } else if space_gaps.total() >= 1 {
        row.space_size = space_gaps.mean() as f32;
        if f64::from(row.space_size) > f64::from(block_space) * 1.5 {
            row.space_size = f32::from(block_space);
        }
        if row.space_size < f32::from(block_non_space * 3 + 1) {
            row.space_size = f32::from(block_non_space * 3 + 1);
        }
    } else {
        row.space_size = f32::from(block_space);
    }
    row.kern_size = if all_gaps.total() > REDO_KERN_LIMIT {
        all_gaps.median() as f32
    } else {
        f32::from(block_non_space)
    };
    row.space_threshold = ((row.space_size + row.kern_size) / 2.0).floor() as i32;
}

/// `Textord::isolated_row_stats`: spacing from the row's own gaps when
/// they separate clearly.
fn isolated_row_stats(row: &mut ToRow, all_gaps: &Stats, suspected_table: bool) -> bool {
    let kern_estimate = all_gaps.median() as f32;
    let crude = (INIT_GUESS_KN_MULT * f64::from(kern_estimate))
        .max(INIT_GUESS_XHT_MULT * f64::from(row.xheight)) as f32;
    let small_count = stats_count_under(all_gaps, crude.ceil() as i16);
    let total = all_gaps.total() as i16;
    if i32::from(total) <= REDO_KERN_LIMIT
        || f64::from(f32::from(small_count) / f32::from(total)) < ENOUGH_SMALL_GAPS
        || total - small_count < 1
    {
        return false;
    }
    let mut cert_space = Stats::new(0, MAX_SPACING - 1);
    let mut all_space = Stats::new(0, MAX_SPACING - 1);
    let units = reduced_units(row);
    let mut prev = units[0];
    for b in &units[1..] {
        let (left, right) = (prev.right as i16, b.left as i16);
        let gap = right.wrapping_sub(left);
        if !ignore_big_gap(row, left, right) && f32::from(gap) > crude {
            let xh = f64::from(row.xheight);
            let g = f64::from(gap);
            if g > FUZZY_SPACE_FACTOR2 * xh
                || (g > FUZZY_SPACE_FACTOR1 * xh
                    && !narrow_blob(row, &prev)
                    && !narrow_blob(row, b))
                || (wide_blob(row, &prev) && wide_blob(row, b))
            {
                cert_space.add(i32::from(gap), 1);
            }
            all_space.add(i32::from(gap), 1);
        }
        prev = *b;
    }
    row.space_size = if cert_space.total() >= ENOUGH_SPACE_SAMPLES_FOR_MEDIAN {
        cert_space.median() as f32
    } else if suspected_table && cert_space.total() > 0 {
        cert_space.mean() as f32
    } else if all_space.total() >= ENOUGH_SPACE_SAMPLES_FOR_MEDIAN {
        all_space.median() as f32
    } else {
        all_space.mean() as f32
    };
    row.kern_size = all_gaps.median() as f32;
    row.space_threshold = ((row.space_size + row.kern_size) / 2.0).floor() as i32;
    if row.kern_size >= row.space_threshold as f32
        || row.space_threshold as f32 >= row.space_size
        || row.space_threshold <= 0
    {
        row.kern_size = 0.0;
        row.space_threshold = 0;
        row.space_size = 0.0;
        return false;
    }
    true
}

/// `Textord::stats_count_under`.
fn stats_count_under(stats: &Stats, threshold: i16) -> i16 {
    let mut total = 0i16;
    for index in 0..threshold {
        total = total.wrapping_add(stats.pile_count(i32::from(index)) as i16);
    }
    total
}

/// The gap `make_a_word_break` judges: the units on either side, the
/// gaps beyond them and the gap itself (real and between reduced boxes).
#[derive(Debug, Clone, Copy)]
struct GapContext {
    prev_gap: i16,
    prev_box: TBox,
    current_gap: i16,
    current_within: i16,
    next_box: TBox,
    next_gap: i16,
}

/// The gap state `make_prop_words` threads through `make_a_word_break`.
#[derive(Debug, Clone, Copy)]
struct Gap {
    fuzzy_sp: bool,
    fuzzy_non: bool,
    prev_gap_was_a_space: bool,
}

/// `Textord::make_a_word_break` with the default parameters (no old
/// method, x-height gaps on).
fn make_a_word_break(row: &ToRow, g: &GapContext, st: &mut Gap) -> bool {
    let (prev_gap, prev_box, next_box, next_gap) =
        (g.prev_gap, &g.prev_box, &g.next_box, g.next_gap);
    let real_current_gap = g.current_gap;
    let mut within_xht_current_gap = g.current_within;
    if f64::from(row.kern_size) > LARGE_KERNING * f64::from(row.xheight) {
        within_xht_current_gap = real_current_gap;
    }
    let current_gap = real_current_gap;
    if prev_box.is_null() {
        st.prev_gap_was_a_space = true;
    }
    let mut space = i32::from(current_gap) > row.space_threshold;
    st.fuzzy_sp = false;
    st.fuzzy_non = false;
    let (cur, real, within) = (
        i32::from(current_gap),
        i32::from(real_current_gap),
        i32::from(within_xht_current_gap),
    );
    if real <= row.max_nonspace && within > row.max_nonspace {
        space = true;
        st.fuzzy_non = true;
    } else if real <= row.space_threshold && within > row.space_threshold {
        space = true;
        st.fuzzy_sp = true;
    } else if real < row.min_space && within >= row.min_space {
        space = true;
    } else if cur < row.min_space && cur > row.space_threshold {
        let limit = (f64::from(row.kern_size)
            + PASS_WIDE_FUZZ_SP_TO_CONTEXT * f64::from(row.space_size - row.kern_size))
            as f32;
        let (pg, ng, cg) = (
            f64::from(prev_gap),
            f64::from(next_gap),
            f64::from(current_gap),
        );
        // Rules 1 to 4: a dubious space next to a narrow blob is a kern
        // when the gap on the blob's other side says so.
        let narrow_prev = prev_box.width() > 0 && narrow_blob(row, prev_box);
        let narrow_next = next_box.width() > 0 && narrow_blob(row, next_box);
        let next_is_space = i32::from(next_gap) > row.space_threshold;
        let to_kern = (narrow_prev && st.prev_gap_was_a_space && cg <= GAP_FACTOR * pg)
            || (narrow_prev && !st.prev_gap_was_a_space && cg * GAP_FACTOR <= pg)
            || (narrow_next && next_is_space && cg <= GAP_FACTOR * ng)
            || (narrow_next && !next_is_space && cg * GAP_FACTOR <= ng);
        if to_kern {
            if current_gap as f32 > limit {
                st.fuzzy_non = true;
            } else {
                space = false;
            }
        } else if narrow_next || narrow_prev {
            st.fuzzy_sp = true;
        }
    } else if cur > row.max_nonspace && cur <= row.space_threshold {
        let biggest = f64::from(prev_gap.max(next_gap));
        let cg = f64::from(current_gap);
        if prev_box.width() > 0
            && next_box.width() > 0
            && cg >= KERN_GAP_FACTOR1 * biggest
            && wide_blob(row, prev_box)
            && wide_blob(row, next_box)
        {
            space = true;
            st.fuzzy_sp = true;
        } else if prev_box.width() > 0
            && next_box.width() > 0
            && ((current_gap > 5
                && cg >= KERN_GAP_FACTOR2 * biggest
                && !(narrow_blob(row, prev_box) || suspected_punct_blob(row, prev_box))
                && !(narrow_blob(row, next_box) || suspected_punct_blob(row, next_box)))
                || cg >= KERN_GAP_FACTOR3 * biggest)
        {
            // Rules 8 and 9.
            space = true;
            st.fuzzy_non = true;
        }
    }
    st.prev_gap_was_a_space = space && !st.fuzzy_non;
    space
}

/// `Textord::peek_at_next_gap`: the next unit's box, the gap after it and
/// that gap between reduced boxes.
fn peek_at_next_gap(row: &mut ToRow, i: usize) -> (TBox, i16, i16) {
    let mut it = i;
    let mut red_it = i;
    let next_box = box_next(&row.blobs, &mut it);
    let next_reduced = reduced_box_next(row, &mut red_it);
    if it == 0 {
        return (next_box, i16::MAX, i16::MAX);
    }
    let beyond = row.blobs[it].bbox;
    let next_gap = (beyond.left - next_box.right) as i16;
    let beyond = reduced_box_next(row, &mut red_it);
    let next_within = (beyond.left - next_reduced.right) as i16;
    (next_box, next_gap, next_within)
}

/// `Textord::make_prop_words`: cuts the row into words at the gaps
/// `make_a_word_break` calls spaces.
fn make_prop_words(row: &mut ToRow) -> Row {
    let n = row.blobs.len();
    let mut words: Vec<Werd> = Vec::new();
    let mut cblobs: Vec<CBlob> = Vec::new();
    let mut prev_fuzzy_sp = false;
    let mut prev_fuzzy_non = false;
    let mut st = Gap {
        fuzzy_sp: false,
        fuzzy_non: false,
        prev_gap_was_a_space: false,
    };
    let mut prev_gap;
    let mut current_gap = i16::MAX;
    let mut next_gap;
    let mut current_within;
    let mut next_within;
    let mut prev_box;
    let mut next_box;
    (next_box, next_gap, next_within) = peek_at_next_gap(row, 0);
    let mut i = 0usize;
    loop {
        let blob = &mut row.blobs[i];
        if blob.joined {
            if let Some(c) = blob.cblob.take()
                && let Some(last) = cblobs.last_mut()
            {
                last.outlines.extend(c.outlines);
            }
        } else if let Some(c) = &blob.cblob {
            cblobs.push(c.clone());
        }
        i = (i + 1) % n;
        let at_first = i == 0;
        let b = &row.blobs[i];
        if !b.joined && b.cblob.is_some() {
            prev_gap = current_gap;
            prev_box = next_box;
            current_gap = next_gap;
            current_within = next_within;
            (next_box, next_gap, next_within) = peek_at_next_gap(row, i);
            let ctx = GapContext {
                prev_gap,
                prev_box,
                current_gap,
                current_within,
                next_box,
                next_gap,
            };
            let brk = make_a_word_break(row, &ctx, &mut st);
            if brk || at_first {
                let mut word = Werd::new(std::mem::take(&mut cblobs));
                if prev_fuzzy_sp {
                    word.fuzzy_sp = true;
                } else if prev_fuzzy_non {
                    word.fuzzy_non = true;
                }
                words.push(word);
                if !at_first {
                    prev_fuzzy_sp = st.fuzzy_sp;
                    prev_fuzzy_non = st.fuzzy_non;
                }
            }
        }
        if at_first {
            break;
        }
    }
    Row {
        words,
        baseline: row.baseline.clone(),
        xheight: row.xheight,
        ascrise: row.ascrise,
        descdrop: row.descdrop,
    }
}
