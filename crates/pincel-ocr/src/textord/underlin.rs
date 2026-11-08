//! Underlines (`src/textord/underlin.cpp` `restore_underlined_blobs`):
//! the parts of an underline that run through the x-height band are
//! letters touching it, and go back into their row.

use super::blob::CBlob;
use super::blobbox::BlobBox;
use super::fpchop::split_to_blob;
use super::makerow::ToRow;
use super::outline::Outline;
use super::quspline::QSpline;
use super::stats::Stats;
use super::tordmain::ToBlock;

/// `textord_underline_offset`.
const UNDERLINE_OFFSET: f64 = 0.1;
/// `textord_fp_chop_error`.
const FP_CHOP_ERROR: i32 = 2;

/// `restore_underlined_blobs`.
pub(crate) fn restore_underlined_blobs(block: &mut ToBlock) {
    if block.rows.is_empty() {
        return;
    }
    let pitch_error = FP_CHOP_ERROR as f32 + 0.5;
    let mut residual: Vec<BlobBox> = Vec::new();
    let (mut left, mut right): (Vec<Outline>, Vec<Outline>) = (Vec::new(), Vec::new());
    for u_line in std::mem::take(&mut block.underlines) {
        let blob_box = u_line.bbox;
        let Some(r) = most_overlapping_row(&block.rows, &u_line) else {
            return;
        };
        let row = &block.rows[r];
        let cells = find_underlined_blobs(
            &u_line,
            &row.baseline,
            row.xheight,
            (f64::from(row.xheight) * UNDERLINE_OFFSET) as f32,
        );
        let mut u_outlines = u_line.cblob.map(|c| c.outlines);
        for (x, y) in cells {
            if y - x > FP_CHOP_ERROR + 1 {
                split_to_blob(u_outlines.take(), x, pitch_error, &mut left, &mut right);
                if !left.is_empty() {
                    residual.push(BlobBox::new(CBlob::from_outlines(std::mem::take(
                        &mut left,
                    ))));
                }
                split_to_blob(None, y, pitch_error, &mut left, &mut right);
                if !left.is_empty() {
                    let b = BlobBox::new(CBlob::from_outlines(std::mem::take(&mut left)));
                    insert_blob(&mut block.rows[r], b);
                }
            }
        }
        if !right.is_empty() {
            split_to_blob(None, blob_box.right, pitch_error, &mut left, &mut right);
            if !left.is_empty() {
                residual.push(BlobBox::new(CBlob::from_outlines(std::mem::take(
                    &mut left,
                ))));
            }
        }
    }
    block.underlines = residual;
}

/// `TO_ROW::insert_blob`: after the blobs whose left edge is not greater.
fn insert_blob(row: &mut ToRow, blob: BlobBox) {
    let pos = row
        .blobs
        .iter()
        .position(|b| b.bbox.left > blob.bbox.left)
        .unwrap_or(row.blobs.len());
    row.blobs.insert(pos, blob);
}

/// `most_overlapping_row`: the row whose band the blob overlaps most.
fn most_overlapping_row(rows: &[ToRow], blob: &BlobBox) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let bb = blob.bbox;
    let x = f64::from(((bb.left + bb.right) / 2) as i16);
    let mut best: Option<usize> = None;
    let mut bestover = -i32::MAX as f32;
    let mut i = 0usize;
    let base = |r: &ToRow| r.baseline.y(x) as f32;
    while i < rows.len() && base(&rows[i]) + rows[i].descdrop > bb.top as f32 {
        best = Some(i);
        bestover = bb.top as f32 - base(&rows[i]) + rows[i].descdrop;
        i += 1;
    }
    while i < rows.len() && base(&rows[i]) + rows[i].xheight + rows[i].ascrise >= bb.bottom as f32 {
        let r = &rows[i];
        let mut overlap = base(r) + r.xheight + r.ascrise;
        if (bb.top as f32) < overlap {
            overlap = bb.top as f32;
        }
        if bb.bottom as f32 > base(r) + r.descdrop {
            overlap -= bb.bottom as f32;
        } else {
            overlap -= base(r) + r.descdrop;
        }
        if overlap > bestover {
            bestover = overlap;
            best = Some(i);
        }
        i += 1;
    }
    // The iterator wraps back to the first row when it runs off the end.
    let r = &rows[i % rows.len()];
    if bestover < 0.0 && base(r) + r.xheight + r.ascrise - bb.bottom as f32 > bestover {
        best = Some(i % rows.len());
    }
    best
}

/// `find_underlined_blobs`: the x ranges where the underline has ink in
/// the x-height band.
fn find_underlined_blobs(
    u_line: &BlobBox,
    baseline: &QSpline,
    xheight: f32,
    offset: f32,
) -> Vec<(i32, i32)> {
    let bb = u_line.bbox;
    let mut middle = Stats::new(bb.left, bb.right);
    if let Some(c) = &u_line.cblob {
        for o in &c.outlines {
            middle_projection(o, baseline, xheight, offset, &mut middle);
        }
    }
    let mut cells = Vec::new();
    let mut x = bb.left;
    while x < bb.right {
        if middle.pile_count(x) > 0 {
            let mut y = x + 1;
            while y < bb.right && middle.pile_count(y) > 0 {
                y += 1;
            }
            cells.push((x, y));
            x = y;
        }
        x += 1;
    }
    cells
}

/// The middle band of `vertical_cunderline_projection`.
fn middle_projection(
    o: &Outline,
    baseline: &QSpline,
    xheight: f32,
    offset: f32,
    middle: &mut Stats,
) {
    let mut pos = o.start;
    let band = |x: i32| {
        let y = baseline.y(f64::from(x));
        (
            i32::from((y + f64::from(offset) + 0.5).floor() as i16),
            i32::from((y + f64::from(offset) + f64::from(xheight) + 0.5).floor() as i16),
        )
    };
    for i in 0..o.steps.len() {
        let st = o.step(i);
        if st.0 > 0 {
            let (lower, upper) = band(pos.0);
            if pos.1 >= lower {
                if pos.1 >= upper {
                    middle.add(pos.0, lower - upper);
                } else {
                    middle.add(pos.0, lower - pos.1);
                }
            }
        } else if st.0 < 0 {
            let (lower, upper) = band(pos.0 - 1);
            if pos.1 >= lower {
                if pos.1 >= upper {
                    middle.add(pos.0 - 1, upper - lower);
                } else {
                    middle.add(pos.0 - 1, pos.1 - lower);
                }
            }
        }
        pos = (pos.0 + st.0, pos.1 + st.1);
    }
    for c in &o.children {
        middle_projection(c, baseline, xheight, offset, middle);
    }
}
