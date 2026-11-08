//! The "old" spline baseline (`src/textord/oldbasel.cpp`), which
//! Tesseract still uses by default (`textord_old_baselines`), and
//! `make_spline_rows` from `src/textord/makerow.cpp`.

use super::blobbox::box_next_pre_chopped;
use super::detlinefit::DetLineFit;
use super::makerow::ToRow;
use super::quspline::{QSpline, Qlsq};
use super::rect::TBox;
use super::stats::Stats;
use super::tordmain::ToBlock;
use super::xheight::{compute_block_xheight, compute_row_xheight};

/// `TURNLIMIT`.
const TURN_LIMIT: i32 = 1;
/// `MINASCRISE`.
const MIN_ASC_RISE: f32 = 2.0;
/// `MAXOVERLAP`.
const MAX_OVERLAP: f64 = 0.1;
/// `MAXBADRUN`.
const MAX_BAD_RUN: i32 = 2;
/// `MAXHEIGHT`.
const MAX_HEIGHT: i32 = 300;
/// `MAXPARTS`.
const MAX_PARTS: usize = 6;
/// `SPLINESIZE`.
const SPLINE_SIZE: usize = 23;
/// `oldbl_holed_losscount`.
const HOLED_LOSSCOUNT: i32 = 10;
/// `oldbl_dot_error_size`.
const DOT_ERROR_SIZE: f64 = 1.26;
/// `textord_oldbl_jumplimit`.
const JUMP_LIMIT: f64 = 0.15;
/// `textord_spline_medianwin`.
const SPLINE_MEDIANWIN: usize = 6;

/// `Textord::make_spline_rows` with `make_baseline_spline` (straight,
/// since `textord_parallel_baselines` is set), then
/// `make_old_baselines`.
pub(crate) fn make_spline_rows(block: &mut ToBlock, gradient: f32, page: TBox) {
    block.rows.retain(|r| !r.blobs.is_empty());
    for row in &mut block.rows {
        let (first, last) = straight_spline_ends(row);
        row.baseline =
            QSpline::from_coeffs(&[first, last], &[0.0, f64::from(row.m), f64::from(row.c)]);
    }
    make_old_baselines(block, gradient, page);
}

/// The x range `segment_baseline` gives the straight spline: the first
/// unit's left edge and the right edge of the last blob (few blobs) or
/// the last unit.
fn straight_spline_ends(row: &ToRow) -> (i32, i32) {
    let blobs = &row.blobs;
    let first = blobs[0].bbox.left;
    let n = blobs.len();
    if n <= SPLINE_MEDIANWIN || n < 8 {
        return (first, blobs[n - 1].bbox.right);
    }
    let last_unit = blobs
        .iter()
        .rev()
        .find(|b| !b.joined)
        .map_or(blobs[n - 1].bbox, |b| b.bbox);
    (first, last_unit.right)
}

/// `Textord::make_old_baselines` with `correlate_lines`.
fn make_old_baselines(block: &mut ToBlock, gradient: f32, page: TBox) {
    let line_size = block.line_size;
    let mut prev: Option<QSpline> = None;
    for row in &mut block.rows {
        find_textlines(line_size, row, 2, None, page);
        if let (true, Some(p)) = (row.xheight <= 0.0, &prev) {
            find_textlines(line_size, row, 2, Some(p), page);
        }
        prev = if row.xheight > 0.0 {
            Some(row.baseline.clone())
        } else {
            None
        };
    }
    // correlate_neighbours only acts on rows with a negative x-height,
    // which compute_row_xheight never leaves.
    compute_block_xheight(block, gradient);
}

/// `Textord::find_textlines`: fits the row's baseline spline and
/// estimates its x-height.
fn find_textlines(
    line_size: f32,
    row: &mut ToRow,
    degree: i32,
    spline: Option<&QSpline>,
    page: TBox,
) {
    let blobcount_all = row.blobs.len();
    let mut blobcoords = vec![TBox::default(); blobcount_all];
    let (lineheight, holed, blobcount) = get_blob_coords(row, line_size as i32, &mut blobcoords);
    let mut jumplimit = (f64::from(lineheight) * JUMP_LIMIT) as f32;
    if jumplimit < MIN_ASC_RISE {
        jumplimit = MIN_ASC_RISE;
    }
    let coords = &blobcoords[..blobcount];
    if holed {
        make_holed_baseline(coords, spline, &mut row.baseline, row.m);
    } else {
        make_first_baseline(coords, spline, &mut row.baseline);
    }
    if blobcount > 1 {
        let mut partids = vec![0u8; blobcount];
        let mut partsizes = [0i32; MAX_PARTS];
        let mut ydiffs = vec![0.0f32; blobcount];
        let (bestpart, partcount) = partition_line(
            coords,
            &mut partids,
            &mut partsizes,
            &row.baseline,
            jumplimit,
            &mut ydiffs,
        );
        let (xcoords, ycoords) = partition_coords(coords, &partids, bestpart);
        if xcoords.is_empty() {
            row.xheight = -1.0;
            row.descdrop = 0.0;
            row.ascrise = 0.0;
            row.baseline
                .extrapolate(f64::from(row.m), page.left, page.right);
            compute_row_xheight(row, row.m, line_size as i32);
            return;
        }
        let mut xstarts = [0i32; SPLINE_SIZE + 1];
        let mut segments = segment_spline(&xcoords, &ycoords, degree, &mut xstarts);
        if !holed {
            loop {
                row.baseline = QSpline::fit(&xstarts, segments, &xcoords, &ycoords, degree);
                if !split_stepped_spline(
                    &row.baseline,
                    jumplimit / 2.0,
                    &xcoords,
                    &mut xstarts,
                    &mut segments,
                ) {
                    break;
                }
            }
        }
        find_lesser_parts(row, coords, &partids, &partsizes, partcount, bestpart);
    } else {
        row.xheight = -1.0;
        row.descdrop = 0.0;
        row.ascrise = 0.0;
    }
    row.baseline
        .extrapolate(f64::from(row.m), page.left, page.right);
    compute_row_xheight(row, row.m, line_size as i32);
}

/// `get_blob_coords`: the boxes of the row's units, dropping small
/// non-dot units; returns the x-height guess, whether many units were
/// dropped in a row and the number kept.
fn get_blob_coords(row: &ToRow, lineheight: i32, coords: &mut [TBox]) -> (i32, bool, usize) {
    let blobs = &row.blobs;
    if blobs.is_empty() {
        return (0, false, 0);
    }
    let mut heightstat = Stats::new(0, MAX_HEIGHT - 1);
    let (mut maxloss, mut loss) = (0, 0);
    let mut index = 0usize;
    let mut it = 0usize;
    let limit = f64::from(lineheight) * 0.25;
    loop {
        coords[index] = box_next_pre_chopped(blobs, &mut it);
        let cycled = it == 0;
        let b = coords[index];
        if f64::from(b.height()) > limit {
            heightstat.add(b.height(), 1);
        }
        // Kept: the first and last units, tall units and dots.
        let dot = f64::from(b.height()) < f64::from(b.width()) * DOT_ERROR_SIZE
            && f64::from(b.width()) < f64::from(b.height()) * DOT_ERROR_SIZE;
        if index == 0 || f64::from(b.height()) > limit || cycled || dot {
            index += 1;
            loss = 0;
        } else {
            loss += 1;
            if loss > maxloss {
                maxloss = loss;
            }
        }
        if cycled {
            break;
        }
    }
    let holed = maxloss > HOLED_LOSSCOUNT;
    let height = if heightstat.total() > 1 {
        heightstat.ile(0.25) as i32
    } else {
        coords[0].height()
    };
    (height, holed, index)
}

/// `make_first_baseline`: with `textord_oldbl_paradef` the straight
/// baseline stays unless a usable previous spline is given.
fn make_first_baseline(coords: &[TBox], spline: Option<&QSpline>, baseline: &mut QSpline) {
    let left = coords[0].left;
    let right = coords[coords.len() - 1].right;
    let Some(spline) = spline else { return };
    let segs = spline.segments();
    if segs < 3
        || f64::from(spline.xcoords[1]) > f64::from(left) + MAX_OVERLAP * f64::from(right - left)
        || f64::from(spline.xcoords[segs - 1])
            < f64::from(right) - MAX_OVERLAP * f64::from(right - left)
    {
        return;
    }
    *baseline = spline.clone();
    let dy = (f64::from(coords[0].bottom) - spline.y(f64::from(coords[0].right))) as i16;
    baseline.shift(0, i32::from(dy));
}

/// `make_holed_baseline`: a straight fit of the given gradient.
fn make_holed_baseline(
    coords: &[TBox],
    spline: Option<&QSpline>,
    baseline: &mut QSpline,
    gradient: f32,
) {
    let left = coords[0].left;
    let right = coords[coords.len() - 1].right;
    let mut lms = DetLineFit::default();
    for b in coords {
        lms.add(((b.left + b.right) / 2, b.bottom), 0);
    }
    let (_, c) = lms.constrained_fit_m(f64::from(gradient));
    *baseline = QSpline::from_coeffs(&[left, right], &[0.0, f64::from(gradient), f64::from(c)]);
    if let Some(spline) = spline {
        let segs = spline.segments();
        if segs >= 3
            && f64::from(spline.xcoords[1])
                <= f64::from(left) + MAX_OVERLAP * f64::from(right - left)
            && f64::from(spline.xcoords[segs - 1])
                >= f64::from(right) - MAX_OVERLAP * f64::from(right - left)
        {
            *baseline = spline.clone();
            let x = f64::from(left + right) / 2.0;
            let dy = (f64::from(gradient) * x + f64::from(c) - spline.y(x)) as i16;
            baseline.shift(0, i32::from(dy));
        }
    }
}

/// `partition_line`: groups the units by their offset from the spline;
/// returns the biggest group and the number of groups.
fn partition_line(
    coords: &[TBox],
    partids: &mut [u8],
    partsizes: &mut [i32; MAX_PARTS],
    spline: &QSpline,
    jumplimit: f32,
    ydiffs: &mut [f32],
) -> (usize, usize) {
    let n = coords.len();
    let startx = get_ydiffs(coords, spline, ydiffs);
    let mut numparts = 1usize;
    let mut partdiffs = [0.0f32; MAX_PARTS];
    let mut state = PartitionState::default();
    let mut bestpart: Option<usize> = None;
    for i in startx..n {
        let p = choose_partition(
            ydiffs[i],
            &mut partdiffs,
            bestpart,
            jumplimit,
            &mut state,
            &mut numparts,
        );
        bestpart = Some(p);
        partids[i] = p as u8;
        partsizes[p] += 1;
    }
    bestpart = None;
    state = PartitionState::default();
    partsizes[0] -= 1;
    for i in (0..=startx).rev() {
        let p = choose_partition(
            ydiffs[i],
            &mut partdiffs,
            bestpart,
            jumplimit,
            &mut state,
            &mut numparts,
        );
        bestpart = Some(p);
        partids[i] = p as u8;
        partsizes[p] += 1;
    }
    let mut biggest = 0usize;
    for p in 1..numparts {
        if partsizes[p] >= partsizes[biggest] {
            biggest = p;
        }
    }
    merge_oldbl_parts(coords, partids, partsizes, biggest, jumplimit);
    (biggest, numparts)
}

/// `drift` and `lastdelta` of `choose_partition`.
#[derive(Debug, Default, Clone, Copy)]
struct PartitionState {
    drift: f32,
    last_delta: f32,
}

/// `choose_partition`.
fn choose_partition(
    diff: f32,
    partdiffs: &mut [f32; MAX_PARTS],
    lastpart: Option<usize>,
    jumplimit: f32,
    state: &mut PartitionState,
    partcount: &mut usize,
) -> usize {
    let lastpart = match lastpart {
        Some(p) => p,
        None => {
            partdiffs[0] = diff;
            state.drift = 0.0;
            state.last_delta = 0.0;
            0
        }
    };
    let mut delta = diff - partdiffs[lastpart] - state.drift;
    let bestpart;
    if delta.abs() > jumplimit / 2.0 {
        let mut bestdelta = diff - partdiffs[0] - state.drift;
        let mut best = 0usize;
        for (p, &pd) in partdiffs.iter().enumerate().take(*partcount).skip(1) {
            let d = diff - pd - state.drift;
            if d.abs() < bestdelta.abs() {
                bestdelta = d;
                best = p;
            }
        }
        delta = bestdelta;
        if bestdelta.abs() > jumplimit && *partcount < MAX_PARTS {
            best = *partcount;
            *partcount += 1;
            partdiffs[best] = diff - state.drift;
            delta = 0.0;
        }
        bestpart = best;
    } else {
        bestpart = lastpart;
    }
    if bestpart == lastpart
        && ((delta - state.last_delta).abs() < jumplimit / 2.0 || delta.abs() < jumplimit / 2.0)
    {
        state.drift = (3.0 * state.drift + delta) / 3.0;
    }
    state.last_delta = delta;
    bestpart
}

/// `get_ydiffs`: the offsets of the units from the spline; returns the
/// middle of the best-behaved window of three.
fn get_ydiffs(coords: &[TBox], spline: &QSpline, ydiffs: &mut [f32]) -> usize {
    let mut diffsum = 0.0f32;
    let mut bestindex = 0usize;
    let mut bestsum = i32::MAX as f32;
    let mut drift = 0.0f32;
    let mut lastx = coords[0].left;
    for (i, b) in coords.iter().enumerate() {
        let xcentre = (b.left + b.right) >> 1;
        drift = (f64::from(drift) + spline.step(f64::from(lastx), f64::from(xcentre))) as f32;
        lastx = xcentre;
        let mut diff = b.bottom as f32;
        diff = (f64::from(diff) - spline.y(f64::from(xcentre))) as f32;
        diff += drift;
        ydiffs[i] = diff;
        if i > 2 {
            diffsum -= ydiffs[i - 3].abs();
        }
        diffsum += diff.abs();
        if i >= 2 && diffsum < bestsum {
            bestsum = diffsum;
            bestindex = i - 1;
        }
    }
    bestindex
}

/// `merge_oldbl_parts`: short runs off the main partition rejoin it when
/// they fit their neighbours.
fn merge_oldbl_parts(
    coords: &[TBox],
    partids: &mut [u8],
    partsizes: &mut [i32; MAX_PARTS],
    biggest: usize,
    jumplimit: f32,
) {
    let n = coords.len();
    let biggest_id = biggest as u8;
    let mut prevpart = biggest_id;
    let mut runlength = 0i32;
    let mut startx = 0usize;
    let centre = |b: &TBox| ((f64::from(b.left + b.right) / 2.0) as f32, b.bottom as f32);
    for blobindex in 0..n {
        if partids[blobindex] != prevpart {
            if prevpart != biggest_id && runlength > MAX_BAD_RUN {
                let mut q = Qlsq::default();
                for b in &coords[startx..blobindex] {
                    let (x, y) = centre(b);
                    q.add(f64::from(x), f64::from(y));
                }
                let (_, qb, qc) = q.fit(1);
                let (m, c) = (qb as f32, qc as f32);
                let mut found = false;
                let mut close = false;
                let mut t = 1usize;
                while !found && (startx >= t || blobindex + t <= n) {
                    if startx >= t && partids[startx - t] == biggest_id {
                        found = true;
                        let (x, y) = centre(&coords[startx - t]);
                        let diff = m * x + c - y;
                        if diff < jumplimit && -diff < jumplimit {
                            close = true;
                        }
                    }
                    if blobindex + t <= n && partids[blobindex + t - 1] == biggest_id {
                        found = true;
                        let (x, y) = centre(&coords[blobindex + t - 1]);
                        let diff = m * x + c - y;
                        if diff < jumplimit && -diff < jumplimit {
                            close = true;
                        }
                    }
                    t += 1;
                }
                if close {
                    partsizes[usize::from(prevpart)] -= runlength;
                    for id in &mut partids[startx..blobindex] {
                        *id = biggest_id;
                    }
                }
            }
            prevpart = partids[blobindex];
            runlength = 1;
            startx = blobindex;
        } else {
            runlength += 1;
        }
    }
}

/// `partition_coords`: the centres and bottoms of the units in `part`.
fn partition_coords(coords: &[TBox], partids: &[u8], part: usize) -> (Vec<i32>, Vec<i32>) {
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (b, &id) in coords.iter().zip(partids) {
        if usize::from(id) == part {
            xs.push((b.left + b.right) >> 1);
            ys.push(b.bottom);
        }
    }
    (xs, ys)
}

/// `segment_spline`: segment boundaries halfway between the turning
/// points of the bottoms; returns the number of segments.
fn segment_spline(
    xcoords: &[i32],
    ycoords: &[i32],
    degree: i32,
    xstarts: &mut [i32; SPLINE_SIZE + 1],
) -> usize {
    let mut pointcount = xcoords.len();
    xstarts[0] = xcoords[0] - 1;
    let max_x = xcoords[pointcount - 1] + 1;
    if degree < 2 {
        pointcount = 0;
    }
    let mut turnpoints = [0usize; SPLINE_SIZE];
    let mut turncount = 0usize;
    if pointcount > 3 {
        let mut ptindex = 1usize;
        let (mut lastmax, mut lastmin) = (0usize, 0usize);
        while ptindex < pointcount - 1 && turncount < SPLINE_SIZE - 1 {
            if ycoords[ptindex - 1] > ycoords[ptindex] && ycoords[ptindex] <= ycoords[ptindex + 1] {
                if ycoords[ptindex] < ycoords[lastmax] - TURN_LIMIT {
                    if turncount == 0 || turnpoints[turncount - 1] != lastmax {
                        turnpoints[turncount] = lastmax;
                        turncount += 1;
                    }
                    lastmin = ptindex;
                } else if ycoords[ptindex] < ycoords[lastmin] {
                    lastmin = ptindex;
                }
            }
            if ycoords[ptindex - 1] < ycoords[ptindex] && ycoords[ptindex] >= ycoords[ptindex + 1] {
                if ycoords[ptindex] > ycoords[lastmin] + TURN_LIMIT {
                    if turncount == 0 || turnpoints[turncount - 1] != lastmin {
                        turnpoints[turncount] = lastmin;
                        turncount += 1;
                    }
                    lastmax = ptindex;
                } else if ycoords[ptindex] > ycoords[lastmax] {
                    lastmax = ptindex;
                }
            }
            ptindex += 1;
        }
        if ycoords[ptindex] < ycoords[lastmax] - TURN_LIMIT
            && (turncount == 0 || turnpoints[turncount - 1] != lastmax)
        {
            if turncount < SPLINE_SIZE - 1 {
                turnpoints[turncount] = lastmax;
                turncount += 1;
            }
            if turncount < SPLINE_SIZE - 1 {
                turnpoints[turncount] = ptindex;
                turncount += 1;
            }
        } else if ycoords[ptindex] > ycoords[lastmin] + TURN_LIMIT
            && (turncount == 0 || turnpoints[turncount - 1] != lastmin)
        {
            if turncount < SPLINE_SIZE - 1 {
                turnpoints[turncount] = lastmin;
                turncount += 1;
            }
            if turncount < SPLINE_SIZE - 1 {
                turnpoints[turncount] = ptindex;
                turncount += 1;
            }
        } else if turncount > 0
            && turnpoints[turncount - 1] == lastmin
            && turncount < SPLINE_SIZE - 1
        {
            turnpoints[turncount] = if ycoords[ptindex] > ycoords[lastmax] {
                ptindex
            } else {
                lastmax
            };
            turncount += 1;
        } else if turncount > 0
            && turnpoints[turncount - 1] == lastmax
            && turncount < SPLINE_SIZE - 1
        {
            turnpoints[turncount] = if ycoords[ptindex] < ycoords[lastmin] {
                ptindex
            } else {
                lastmin
            };
            turncount += 1;
        }
    }
    let mut segment = 1usize;
    while segment < turncount {
        let a = turnpoints[segment - 1];
        let b = turnpoints[segment];
        let centre = (ycoords[a] + ycoords[b]) / 2;
        let mut ptindex = a + 1;
        if ycoords[a] < ycoords[b] {
            while ptindex < b && ycoords[ptindex + 1] <= centre {
                ptindex += 1;
            }
        } else {
            while ptindex < b && ycoords[ptindex + 1] >= centre {
                ptindex += 1;
            }
        }
        xstarts[segment] =
            (xcoords[ptindex - 1] + xcoords[ptindex] + xcoords[a] + xcoords[b] + 2) / 4;
        segment += 1;
    }
    xstarts[segment] = max_x;
    segment
}

/// `split_stepped_spline`: adds boundaries around big steps.
fn split_stepped_spline(
    baseline: &QSpline,
    jumplimit: f32,
    xcoords: &[i32],
    xstarts: &mut [i32; SPLINE_SIZE + 1],
    segments: &mut usize,
) -> bool {
    let mut doneany = false;
    let mut startindex = 0usize;
    let mw = SPLINE_MEDIANWIN;
    let mut segment = 1usize;
    while segment + 1 < *segments {
        let step = baseline
            .step(
                f64::from(xstarts[segment - 1] + xstarts[segment]) / 2.0,
                f64::from(xstarts[segment] + xstarts[segment + 1]) / 2.0,
            )
            .abs() as f32;
        if step > jumplimit {
            let last = xcoords.len() - 1;
            while startindex < last && xcoords[startindex] < xstarts[segment - 1] {
                startindex += 1;
            }
            let mut centreindex = startindex;
            while centreindex < last && xcoords[centreindex] < xstarts[segment] {
                centreindex += 1;
            }
            let mut endindex = centreindex;
            while endindex < last && xcoords[endindex] < xstarts[segment + 1] {
                endindex += 1;
            }
            if *segments >= SPLINE_SIZE {
                // Too many segments to resegment.
            } else if endindex - startindex >= mw * 3 {
                while centreindex - startindex < mw * 3 / 2 {
                    centreindex += 1;
                }
                while endindex - centreindex < mw * 3 / 2 {
                    centreindex -= 1;
                }
                let mut leftindex = (startindex + startindex + centreindex) / 3;
                let mut rightindex = (centreindex + endindex + endindex) / 3;
                let leftcoord =
                    (f64::from(xcoords[startindex] * 2 + xcoords[centreindex]) / 3.0) as f32;
                let rightcoord =
                    (f64::from(xcoords[centreindex] + xcoords[endindex] * 2) / 3.0) as f32;
                while xcoords[leftindex] as f32 > leftcoord && leftindex - startindex > mw {
                    leftindex -= 1;
                }
                while (xcoords[leftindex] as f32) < leftcoord && centreindex - leftindex > mw / 2 {
                    leftindex += 1;
                }
                if leftindex > 0
                    && xcoords[leftindex] as f32 - leftcoord
                        > leftcoord - xcoords[leftindex - 1] as f32
                {
                    leftindex -= 1;
                }
                while xcoords[rightindex] as f32 > rightcoord && rightindex - centreindex > mw / 2 {
                    rightindex -= 1;
                }
                while (xcoords[rightindex] as f32) < rightcoord && endindex - rightindex > mw {
                    rightindex += 1;
                }
                if rightindex > 0
                    && xcoords[rightindex] as f32 - rightcoord
                        > rightcoord - xcoords[rightindex - 1] as f32
                {
                    rightindex -= 1;
                }
                // insert_spline_point
                let c1 = (xcoords[leftindex.max(1) - 1] + xcoords[leftindex]) / 2;
                let c2 = (xcoords[rightindex.max(1) - 1] + xcoords[rightindex]) / 2;
                let mut index = *segments;
                while index > segment {
                    xstarts[index + 1] = xstarts[index];
                    index -= 1;
                }
                *segments += 1;
                xstarts[segment] = c1;
                xstarts[segment + 1] = c2;
                doneany = true;
            }
        }
        segment += 1;
    }
    doneany
}

/// `find_lesser_parts`: marks the row failed when too many units in a
/// row are off the main partition and takes the most common lower
/// partition as the descender drop.
fn find_lesser_parts(
    row: &mut ToRow,
    coords: &[TBox],
    partids: &[u8],
    partsizes: &[i32; MAX_PARTS],
    partcount: usize,
    bestpart: usize,
) {
    let mut partsteps = [0.0f32; MAX_PARTS];
    let mut runlength = 0;
    let mut biggestrun = 0;
    for (b, &id) in coords.iter().zip(partids) {
        let xcentre = (b.left + b.right) >> 1;
        let id = usize::from(id);
        if id != bestpart {
            runlength += 1;
            if runlength > biggestrun {
                biggestrun = runlength;
            }
            partsteps[id] = (f64::from(partsteps[id])
                + (f64::from(b.bottom) - row.baseline.y(f64::from(xcentre))))
                as f32;
        } else {
            runlength = 0;
        }
    }
    row.xheight = if biggestrun > MAX_BAD_RUN { -1.0 } else { 1.0 };
    let mut poscount = 0;
    let mut negcount = 0;
    let mut bestneg = 0.0f32;
    for p in 0..partcount {
        if p != bestpart {
            if partsizes[p] == 0 {
                partsteps[p] = 0.0;
            } else {
                partsteps[p] /= partsizes[p] as f32;
            }
            if partsteps[p] >= MIN_ASC_RISE && partsizes[p] > poscount {
                poscount = partsizes[p];
            }
            if partsteps[p] <= -MIN_ASC_RISE && partsizes[p] > negcount {
                bestneg = partsteps[p];
                negcount = partsizes[p];
            }
        }
    }
    row.descdrop = bestneg;
}
