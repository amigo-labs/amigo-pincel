//! Rows of blobs (`src/textord/makerow.cpp`): the single row of
//! `--psm 7`, its line fits, underlines, blob association and the
//! x-height estimate.

use super::blob::CBlob;
use super::blobbox::BlobBox;
use super::cxx;
use super::detlinefit::DetLineFit;
use super::quspline::QSpline;
use super::rect::TBox;
use super::tordmain::ToBlock;

/// `TO_ROW`: a text line with its blobs and metrics.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToRow {
    pub(crate) blobs: Vec<BlobBox>,
    pub(crate) m: f32,
    pub(crate) c: f32,
    pub(crate) error: f32,
    pub(crate) para_c: f32,
    pub(crate) para_error: f32,
    pub(crate) y_origin: f32,
    pub(crate) baseline: QSpline,
    pub(crate) xheight: f32,
    pub(crate) xheight_evidence: i32,
    pub(crate) ascrise: f32,
    pub(crate) descdrop: f32,
    pub(crate) min_space: i32,
    pub(crate) max_nonspace: i32,
    pub(crate) space_threshold: i32,
    pub(crate) kern_size: f32,
    pub(crate) space_size: f32,
}

impl ToRow {
    /// `TO_ROW::set_line`.
    pub(crate) fn set_line(&mut self, m: f32, c: f32, error: f32) {
        self.m = m;
        self.c = c;
        self.error = error;
    }

    /// `TO_ROW::set_parallel_line`.
    pub(crate) fn set_parallel_line(&mut self, gradient: f32, c: f32, error: f32) {
        self.para_c = c;
        self.para_error = error;
        self.y_origin = c / (1.0 + gradient * gradient).sqrt();
    }
}

/// `blob_x_order` as a "less than".
pub(crate) fn blob_x_less(a: &BlobBox, b: &BlobBox) -> bool {
    a.bbox.left < b.bbox.left
}

/// `make_single_row` followed by `fit_lms_line` and `compute_page_skew`:
/// puts every blob of the block in one row (a lone blob with holes adds a
/// second row of its holes, `MakeRowFromSubBlobs`) and returns the page
/// gradient.
pub(crate) fn make_single_row(block: &mut ToBlock, page: TBox) -> f32 {
    let blobs = std::mem::take(&mut block.blobs);
    let small = std::mem::take(&mut block.small);
    let noise = std::mem::take(&mut block.noise);
    let large = std::mem::take(&mut block.large);
    let mut all: Vec<BlobBox> = Vec::new();
    let mut lists = vec![blobs, small, noise, large].into_iter();
    if let Some(first) = lists.next() {
        let mut first = first.into_iter();
        if let Some(b0) = first.next() {
            // add_list_after on the iterator at the first blob: the lists
            // land after that blob in reverse order.
            all.push(b0);
            let added: Vec<Vec<BlobBox>> = lists.collect();
            for l in added.into_iter().rev() {
                all.extend(l);
            }
            all.extend(first);
        } else {
            // An empty list takes the first non-empty list; the others go in
            // after it in reverse order.
            let added: Vec<Vec<BlobBox>> = lists.filter(|l| !l.is_empty()).collect();
            let mut it = added.into_iter();
            if let Some(head) = it.next() {
                all.extend(head);
                for l in it.rev() {
                    all.extend(l);
                }
            }
        }
    }
    let mut rows = Vec::new();
    if all.len() == 1 {
        if let Some((sub_row, size)) = row_from_sub_blobs(&all[0]) {
            if size > block.line_size {
                block.line_size = size;
            }
            rows.push(sub_row);
        }
    } else if all.is_empty() {
        all.push(BlobBox::new(CBlob::fake(page)));
    }
    let (main, _) = row_from_blobs(all);
    rows.insert(0, main);
    for row in &mut rows {
        fit_lms_line(row);
    }
    let gradient = page_skew(&rows);
    block.rows = rows;
    gradient
}

/// `MakeRowFromBlobs`: sorts the blobs into a new row; returns it with
/// the mean blob height.
fn row_from_blobs(mut blobs: Vec<BlobBox>) -> (ToRow, f32) {
    cxx::sort(&mut blobs, &blob_x_less);
    let mut total = 0.0f32;
    for b in &blobs {
        total += b.bbox.height() as f32;
    }
    let n = blobs.len();
    let size = if n > 0 { total / n as f32 } else { total };
    (
        ToRow {
            blobs,
            ..ToRow::default()
        },
        size,
    )
}

/// `MakeRowFromSubBlobs`: a row of the outlines inside a lone blob.
fn row_from_sub_blobs(blob: &BlobBox) -> Option<(ToRow, f32)> {
    let outline = blob.cblob.as_ref()?.outlines.first()?;
    if outline.children.is_empty() {
        return None;
    }
    let blobs = outline
        .children
        .iter()
        .map(|c| BlobBox::new(CBlob::from_outline(c.clone())))
        .collect();
    Some(row_from_blobs(blobs))
}

/// `compute_page_skew`: the median gradient, each row weighted by its
/// blob count over the square of its rounded-up fit error.
fn page_skew(rows: &[ToRow]) -> f32 {
    let mut gradients: Vec<f32> = Vec::new();
    for row in rows {
        let mut err = row.error.ceil() as i32;
        if err <= 0 {
            err = 1;
        }
        let count = row.blobs.len() as i32 / err / err;
        for _ in 0..count {
            gradients.push(row.m);
        }
    }
    if gradients.is_empty() {
        gradients = rows.iter().map(|r| r.m).collect();
    }
    if gradients.is_empty() {
        return 0.0;
    }
    let index = (gradients.len() as f64 * 0.5) as usize;
    cxx::nth_element(&mut gradients, index, &|a, b| a < b);
    gradients[index]
}
/// `fit_lms_line`.
fn fit_lms_line(row: &mut ToRow) {
    let mut lms = DetLineFit::default();
    for b in &row.blobs {
        lms.add((b.bbox.x_middle(), b.bbox.bottom), 0);
    }
    let (error, m, c) = lms.fit_mc();
    row.set_line(m, c, error as f32);
}

/// `textord_underline_width`.
const UNDERLINE_WIDTH: f64 = 2.0;
/// `textord_min_blob_height_fraction`.
pub(crate) const MIN_BLOB_HEIGHT_FRACTION: f64 = 0.75;
/// `textord_max_blob_overlaps`.
const MAX_BLOB_OVERLAPS: i32 = 4;
/// `textord_underline_threshold`.
const UNDERLINE_THRESHOLD: f64 = 0.5;
/// `textord_chop_width`.
const CHOP_WIDTH: f64 = 1.5;

/// `CountOverlaps`.
fn count_overlaps(b: &TBox, min_height: i32, blobs: &[BlobBox]) -> i32 {
    blobs
        .iter()
        .filter(|o| o.bbox.height() >= min_height && b.major_overlap(&o.bbox))
        .count() as i32
}

/// `test_underline` with `horizontal_cblob_projection`. Tesseract tests
/// the blob after rotating it by the skew; the rotation is left out here
/// (exact for an unskewed line).
fn test_underline(cblob: &CBlob, baseline: i32, xheight: i32) -> bool {
    let bbox = cblob.bounding_box();
    let width = bbox.width();
    let mut projection = super::stats::Stats::new(bbox.bottom, bbox.top);
    // The rotated copy keeps only the top-level outlines.
    for o in &cblob.outlines {
        let mut pos = o.start;
        for s in 0..o.steps.len() {
            let st = o.step(s);
            if st.1 > 0 {
                projection.add(pos.1, pos.0);
            } else if st.1 < 0 {
                projection.add(pos.1 - 1, -pos.0);
            }
            pos = (pos.0 + st.0, pos.1 + st.1);
        }
    }
    let mut desc_occ = 0;
    for occ in bbox.bottom..baseline {
        if occ <= bbox.top && projection.pile_count(occ) > desc_occ {
            desc_occ = projection.pile_count(occ);
        }
    }
    let mut x_occ = 0;
    for occ in baseline..=baseline + xheight {
        if occ >= bbox.bottom && occ <= bbox.top && projection.pile_count(occ) > x_occ {
            x_occ = projection.pile_count(occ);
        }
    }
    let mut asc_occ = 0;
    for occ in baseline + xheight + 1..=bbox.top {
        if occ >= bbox.bottom && projection.pile_count(occ) > asc_occ {
            asc_occ = projection.pile_count(occ);
        }
    }
    if desc_occ > x_occ + x_occ && f64::from(desc_occ) > f64::from(width) * UNDERLINE_THRESHOLD {
        return true;
    }
    asc_occ > x_occ + x_occ && f64::from(asc_occ) > f64::from(width) * UNDERLINE_THRESHOLD
}

/// `separate_underlines`: wide blobs that are underlines (or overlap many
/// blobs) leave the rows.
pub(crate) fn separate_underlines(block: &mut ToBlock) {
    let line_size = f64::from(block.line_size);
    let min_blob_height = (MIN_BLOB_HEIGHT_FRACTION * line_size + 0.5) as i32;
    let xheight = (line_size
        * (super::tordmain::XHEIGHT_FRACTION + super::tordmain::ASCENDER_FRACTION / 2.0))
        as i16;
    for r in 0..block.rows.len() {
        let baseline = block.rows[r].y_origin as i16;
        let mut i = 0;
        while i < block.rows[r].blobs.len() {
            let b = &block.rows[r].blobs[i];
            if f64::from(b.bbox.width()) > line_size * UNDERLINE_WIDTH {
                let under = b
                    .cblob
                    .as_ref()
                    .is_some_and(|c| test_underline(c, i32::from(baseline), i32::from(xheight)));
                if under {
                    let b = block.rows[r].blobs.remove(i);
                    block.underlines.push(b);
                    continue;
                }
                let bbox = b.bbox;
                if count_overlaps(&bbox, min_blob_height, &block.rows[r].blobs) > MAX_BLOB_OVERLAPS
                {
                    let b = block.rows[r].blobs.remove(i);
                    block.large.push(b);
                    continue;
                }
            }
            i += 1;
        }
    }
}

/// `find_cblob_vlimits`: the y range of the top-level outline points in
/// `[left, right]`.
fn cblob_vlimits(cblob: &CBlob, left: f32, right: f32) -> (f32, f32) {
    let (mut ymin, mut ymax) = (i32::MAX as f32, -i32::MAX as f32);
    for o in &cblob.outlines {
        let mut pos = o.start;
        for s in 0..o.steps.len() {
            let x = pos.0 as f32;
            if x >= left && x <= right {
                let y = pos.1 as f32;
                if y < ymin {
                    ymin = y;
                }
                if y > ymax {
                    ymax = y;
                }
            }
            let st = o.step(s);
            pos = (pos.0 + st.0, pos.1 + st.1);
        }
    }
    (ymin, ymax)
}

/// `BLOBNBOX::chop`: splits the merged blob `blobs[start]` (with the
/// joined blobs up to `end`) into pieces about `xheight` wide; returns the
/// pieces after the first, left to right.
fn chop(blobs: &mut [BlobBox], start: usize, end: usize, xheight: f32) -> Vec<BlobBox> {
    let bbox = blobs[start].bbox;
    let count = (bbox.width() as f32 / xheight).floor() as i16;
    let mut pieces = Vec::new();
    if count <= 1 || blobs[start].cblob.is_none() {
        return pieces;
    }
    let width = (bbox.width() + 1) as f32 / f32::from(count);
    let mut right = bbox.right as f32;
    for index in (0..count).rev() {
        let (mut ymin, mut ymax) = (i32::MAX as f32, -i32::MAX as f32);
        for b in &blobs[start..=end] {
            if let Some(c) = &b.cblob {
                let (lo, hi) = cblob_vlimits(c, right - width, right);
                if lo < ymin {
                    ymin = lo;
                }
                if hi > ymax {
                    ymax = hi;
                }
            }
        }
        if ymin < ymax {
            let mut left = (right - width).floor() as i16 as i32;
            if left < bbox.left {
                left = bbox.left;
            }
            let piece = TBox::from_corners(
                (left, i32::from(ymin.floor() as i16)),
                (
                    i32::from(right.ceil() as i16),
                    i32::from(ymax.ceil() as i16),
                ),
            );
            if index == 0 {
                blobs[start].bbox = piece;
            } else {
                pieces.insert(0, BlobBox::fake(piece));
            }
        }
        right -= width;
    }
    pieces
}

/// `pre_associate_blobs`: merges blobs that overlap in x (`BLOBNBOX::merge`
/// marks the later ones joined) and chops wide results.
pub(crate) fn pre_associate_blobs(block: &mut ToBlock) {
    let xheight =
        (f64::from(block.line_size) * super::tordmain::XHEIGHT_FRACTION * CHOP_WIDTH) as f32;
    for row in &mut block.rows {
        let blobs = &mut row.blobs;
        let mut i = 0;
        while i < blobs.len() {
            let start = i;
            let mut bbox = blobs[i].bbox;
            while i + 1 < blobs.len() && bbox.major_x_overlap(&blobs[i + 1].bbox) {
                bbox = bbox.union(&blobs[i + 1].bbox);
                blobs[start].bbox = bbox;
                blobs[i + 1].joined = true;
                i += 1;
            }
            let pieces = chop(blobs, start, i, xheight);
            blobs.splice(i + 1..i + 1, pieces);
            i += 1;
        }
    }
}
