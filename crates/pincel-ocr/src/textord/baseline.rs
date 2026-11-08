//! Straight baselines (`src/textord/baselinedetect.cpp`
//! `BaselineRow`, `BaselineBlock`, `BaselineDetect::ComputeStraightBaselines`)
//! for a block of one row.

use super::detlinefit::{DetLineFit, Llsq, cross_f, int_cast_rounded};
use super::makerow::ToRow;
use super::stats::Stats;
use super::tordmain::ToBlock;
use std::f64::consts::PI;

/// `kMaxDisplacementsModes`.
const MAX_DISPLACEMENT_MODES: usize = 3;
/// `kNumSkipPoints`.
const NUM_SKIP_POINTS: usize = 3;
/// `kMaxSkewDeviation`.
const MAX_SKEW_DEVIATION: f64 = 1.0 / 64.0;
/// `kOffsetQuantizationFactor`.
const OFFSET_QUANTIZATION_FACTOR: f64 = 3.0 / 64.0;
/// `kFitHalfrangeFactor`.
const FIT_HALFRANGE_FACTOR: f64 = 6.0 / 64.0;
/// `kMaxBaselineError`.
const MAX_BASELINE_ERROR: f64 = 3.0 / 64.0;
/// `kMaxBlobSizeMultiple`.
const MAX_BLOB_SIZE_MULTIPLE: f64 = 1.3;

/// `FCOORD::angle`.
fn angle(v: (f32, f32)) -> f32 {
    v.1.atan2(v.0)
}

/// `BaselineRow`.
struct BaselineRow {
    pt1: (f32, f32),
    pt2: (f32, f32),
    error: f64,
    good: bool,
    disp_quant_factor: f64,
    fit_halfrange: f64,
    max_baseline_error: f64,
    fitter: DetLineFit,
}

impl BaselineRow {
    fn new(line_spacing: f64) -> Self {
        Self {
            pt1: (0.0, 0.0),
            pt2: (0.0, 0.0),
            error: 0.0,
            good: false,
            disp_quant_factor: OFFSET_QUANTIZATION_FACTOR * line_spacing,
            fit_halfrange: FIT_HALFRANGE_FACTOR * line_spacing,
            max_baseline_error: MAX_BASELINE_ERROR * line_spacing,
            fitter: DetLineFit::default(),
        }
    }

    /// `BaselineRow::BaselineAngle`.
    fn baseline_angle(&self) -> f64 {
        let a = f64::from(angle((self.pt2.0 - self.pt1.0, self.pt2.1 - self.pt1.1)));
        (a + PI * 1.5) % PI - PI * 0.5
    }

    /// `BaselineRow::StraightYAtX`.
    fn straight_y_at_x(&self, x: f64) -> f64 {
        let den = f64::from(self.pt2.0) - f64::from(self.pt1.0);
        if den == 0.0 {
            return (f64::from(self.pt1.1) + f64::from(self.pt2.1)) / 2.0;
        }
        f64::from(self.pt1.1)
            + (x - f64::from(self.pt1.0)) * (f64::from(self.pt2.1) - f64::from(self.pt1.1)) / den
    }

    /// `BaselineRow::FitBaseline` with `use_box_bottoms` false.
    fn fit_baseline(&mut self, row: &mut ToRow) -> bool {
        self.fitter.clear();
        let mut llsq = Llsq::default();
        for b in &mut row.blobs {
            b.estimate_baseline_position();
            let x_middle = b.bbox.x_middle();
            self.fitter
                .add((x_middle, b.baseline_y), b.bbox.width() / 2);
            llsq.add(f64::from(x_middle), f64::from(b.baseline_y));
        }
        let (mut error, mut pt1, mut pt2) = self.fitter.fit_skip(0, 0);
        self.error = error;
        self.pt1 = (pt1.0 as f32, pt1.1 as f32);
        self.pt2 = (pt2.0 as f32, pt2.1 as f32);
        if self.error > self.max_baseline_error && self.fitter.sufficient_points() {
            let r = self.fitter.fit_skip(NUM_SKIP_POINTS, NUM_SKIP_POINTS);
            error = r.0;
            pt1 = r.1;
            pt2 = r.2;
            if error < self.error / 2.0 {
                self.error = error;
                self.pt1 = (pt1.0 as f32, pt1.1 as f32);
                self.pt2 = (pt2.0 as f32, pt2.1 as f32);
            }
        }
        let direction = ((pt2.0 - pt1.0) as f32, (pt2.1 - pt1.1) as f32);
        let target_offset = f64::from(cross_f(direction, (pt1.0 as f32, pt1.1 as f32)));
        self.good = false;
        self.fit_constrained_if_better(direction, 0.0, target_offset);
        if self.baseline_angle().abs() > PI * 0.25 {
            self.pt1 = llsq.mean_point();
            let m = llsq.m();
            self.pt2 = (self.pt1.0 + 1.0, self.pt1.1 + m as f32);
            let c = llsq.c(m);
            self.error = llsq.rms(m, c);
            self.good = false;
        }
        self.good
    }

    /// `BaselineRow::FitConstrainedIfBetter`.
    fn fit_constrained_if_better(&mut self, direction: (f32, f32), cheat: f64, target_offset: f64) {
        let length = (direction.0 * direction.0 + direction.1 * direction.1).sqrt();
        let halfrange = self.fit_halfrange * f64::from(length);
        let (new_error, line_pt) = self.fitter.constrained_fit(
            direction,
            target_offset - halfrange,
            target_offset + halfrange,
        );
        let new_error = new_error - cheat;
        let old_angle = self.baseline_angle();
        let new_angle = f64::from(angle(direction));
        let new_good = new_error <= self.max_baseline_error
            && (cheat > 0.0 || self.fitter.sufficient_points());
        if new_error <= self.error
            || (!self.good && new_good)
            || (new_angle - old_angle).abs() > MAX_SKEW_DEVIATION
        {
            self.error = new_error;
            self.pt1 = (line_pt.0 as f32, line_pt.1 as f32);
            self.pt2 = (self.pt1.0 + direction.0, self.pt1.1 + direction.1);
            self.good = new_good;
        }
    }

    /// `BaselineRow::AdjustBaselineToParallel` with
    /// `SetupBlobDisplacements`.
    fn adjust_to_parallel(&mut self, row: &ToRow, direction: (f32, f32)) {
        let mut dists = Vec::with_capacity(row.blobs.len());
        let (mut min_dist, mut max_dist) = (f64::from(f32::MAX), f64::from(-f32::MAX));
        for b in &row.blobs {
            let pos = (
                (b.bbox.left + b.bbox.right) as f32 / 2.0,
                b.baseline_y as f32,
            );
            let offset = f64::from(cross_f(direction, pos));
            dists.push(offset);
            min_dist = min_dist.min(offset);
            max_dist = max_dist.max(offset);
        }
        if dists.is_empty() {
            return;
        }
        let q = self.disp_quant_factor;
        let mut stats = Stats::new(
            int_cast_rounded(min_dist / q),
            int_cast_rounded(max_dist / q),
        );
        for d in &dists {
            stats.add(int_cast_rounded(d / q), 1);
        }
        let modes = stats.top_n_modes(MAX_DISPLACEMENT_MODES);
        if let Some(&(key, _)) = modes.first() {
            self.fit_constrained_if_better(direction, 0.0, q * f64::from(key));
        }
    }
}

/// `BaselineDetect::ComputeStraightBaselines` for the page's one block:
/// fits each row's straight baseline, makes them parallel to the block
/// skew and sets the block's line parameters. Returns the skew angle.
pub(crate) fn compute_straight_baselines(block: &mut ToBlock) -> f64 {
    let line_spacing = f64::from(block.line_spacing);
    let mut brows = Vec::with_capacity(block.rows.len());
    let mut angles = Vec::new();
    for row in &mut block.rows {
        // The BaselineBlock constructor sorts each row a second time, which
        // can swap blobs with equal left edges.
        super::cxx::sort(&mut row.blobs, &super::makerow::blob_x_less);
        let mut brow = BaselineRow::new(line_spacing);
        if brow.fit_baseline(row) {
            angles.push(brow.baseline_angle());
        }
        brows.push(brow);
    }
    // MedianOfCircularValues; an unrotated page has no default skew.
    let skew = if angles.is_empty() {
        0.0
    } else {
        let mid = angles.len() / 2;
        super::cxx::nth_element(&mut angles, mid, &|a, b| a < b);
        angles[mid]
    };
    let direction = (skew.cos() as f32, skew.sin() as f32);
    for (brow, row) in brows.iter_mut().zip(&block.rows) {
        brow.adjust_to_parallel(row, direction);
    }
    // SetupBlockParameters.
    if line_spacing > 0.0 {
        let min_spacing = block.line_spacing.min(line_spacing as f32);
        if min_spacing < block.line_size {
            block.line_size = min_spacing;
        }
        block.line_spacing = line_spacing as f32;
        block.max_blob_size = (line_spacing * MAX_BLOB_SIZE_MULTIPLE) as f32;
    }
    for (brow, row) in brows.iter().zip(&mut block.rows) {
        let gradient = brow.baseline_angle().tan() as f32;
        let para_c = brow.straight_y_at_x(0.0) as f32;
        row.set_line(gradient, para_c, brow.error as f32);
        row.set_parallel_line(gradient, para_c, brow.error as f32);
    }
    skew
}
