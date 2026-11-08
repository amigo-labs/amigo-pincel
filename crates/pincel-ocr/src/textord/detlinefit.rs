//! Robust line fitting (`src/ccstruct/detlinefit.cpp` `DetLineFit`,
//! `src/ccstruct/linlsq.cpp` `LLSQ`).

use super::cxx;

/// `kNumEndPoints`.
const NUM_END_POINTS: usize = 3;
/// `kMinPointsForErrorCount`.
const MIN_POINTS_FOR_ERROR_COUNT: usize = 16;
/// `kMaxRealDistance` (an `int` in the C++ source).
const MAX_REAL_DISTANCE: f64 = 2.0;

/// `IntCastRounded(double)`.
pub(crate) fn int_cast_rounded(x: f64) -> i32 {
    if x >= 0.0 {
        (x + 0.5) as i32
    } else {
        -((-x + 0.5) as i32)
    }
}

/// `FCOORD * FCOORD`: the cross product in single precision.
pub(crate) fn cross_f(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}

/// `DetLineFit`: fits a line through the points that agree best.
#[derive(Debug, Default, Clone)]
pub(crate) struct DetLineFit {
    pts: Vec<((i32, i32), i32)>,
    distances: Vec<(f64, (i32, i32))>,
    square_length: f64,
}

impl DetLineFit {
    pub(crate) fn clear(&mut self) {
        self.pts.clear();
        self.distances.clear();
    }

    /// `DetLineFit::Add` with a half width.
    pub(crate) fn add(&mut self, pt: (i32, i32), halfwidth: i32) {
        self.pts.push((pt, halfwidth));
    }

    /// `DetLineFit::Fit(skip_first, skip_last, pt1, pt2)`: the best line
    /// through one of the first and one of the last points; returns the
    /// error and the two points.
    pub(crate) fn fit_skip(
        &mut self,
        skip_first: usize,
        skip_last: usize,
    ) -> (f64, (i32, i32), (i32, i32)) {
        let n = self.pts.len();
        if n == 0 {
            return (0.0, (0, 0), (0, 0));
        }
        let skip_first = skip_first.min(n - 1);
        let starts: Vec<(i32, i32)> = (skip_first..(skip_first + NUM_END_POINTS).min(n))
            .map(|i| self.pts[i].0)
            .collect();
        let skip_last = skip_last.min(n - 1);
        let end_i = (n as i64 - NUM_END_POINTS as i64 - skip_last as i64).max(0) as usize;
        let ends: Vec<(i32, i32)> = (end_i..n - skip_last)
            .rev()
            .map(|i| self.pts[i].0)
            .collect();
        if n <= 2 {
            let p1 = starts[0];
            let p2 = if n > 1 { ends[0] } else { p1 };
            return (0.0, p1, p2);
        }
        let mut best_uq = -1.0f64;
        let (mut p1, mut p2) = ((0, 0), (0, 0));
        for &start in &starts {
            for &end in &ends {
                if start != end {
                    self.compute_distances(start, end);
                    let dist = self.evaluate_line_fit();
                    if dist < best_uq || best_uq < 0.0 {
                        best_uq = dist;
                        p1 = start;
                        p2 = end;
                    }
                }
            }
        }
        let err = if best_uq > 0.0 {
            best_uq.sqrt()
        } else {
            best_uq
        };
        (err, p1, p2)
    }

    /// `DetLineFit::Fit(float *m, float *c)`: gradient and intercept.
    pub(crate) fn fit_mc(&mut self) -> (f64, f32, f32) {
        let (err, s, e) = self.fit_skip(0, 0);
        if e.0 != s.0 {
            let m = (e.1 - s.1) as f32 / (e.0 - s.0) as f32;
            let c = s.1 as f32 - m * s.0 as f32;
            (err, m, c)
        } else {
            (err, 0.0, 0.0)
        }
    }

    /// `DetLineFit::ConstrainedFit(direction, min_dist, max_dist)`: the
    /// line in `direction` through the median point; returns the error
    /// and that point.
    pub(crate) fn constrained_fit(
        &mut self,
        direction: (f32, f32),
        min_dist: f64,
        max_dist: f64,
    ) -> (f64, (i32, i32)) {
        self.distances.clear();
        self.square_length = f64::from(direction.0 * direction.0 + direction.1 * direction.1);
        for &(pt, _) in &self.pts {
            let dist = f64::from(cross_f(direction, (pt.0 as f32, pt.1 as f32)));
            if min_dist <= dist && dist <= max_dist {
                self.distances.push((dist, pt));
            }
        }
        if self.pts.is_empty() || self.distances.is_empty() {
            return (0.0, (0, 0));
        }
        let median = self.distances.len() / 2;
        cxx::nth_element(&mut self.distances, median, &|a, b| a.0 < b.0);
        let line_pt = self.distances[median].1;
        let origin = f64::from(cross_f(direction, (line_pt.0 as f32, line_pt.1 as f32)));
        for d in &mut self.distances {
            d.0 -= origin;
        }
        (self.evaluate_line_fit().sqrt(), line_pt)
    }

    /// `DetLineFit::ConstrainedFit(double m, float *c)`.
    pub(crate) fn constrained_fit_m(&mut self, m: f64) -> (f64, f32) {
        if self.pts.is_empty() {
            return (0.0, 0.0);
        }
        let cos = 1.0 / (1.0 + m * m).sqrt();
        let direction = (cos as f32, (m * cos) as f32);
        let (err, pt) = self.constrained_fit(direction, f64::from(-f32::MAX), f64::from(f32::MAX));
        (err, (f64::from(pt.1) - f64::from(pt.0) * m) as f32)
    }

    /// `DetLineFit::SufficientPointsForIndependentFit`.
    pub(crate) fn sufficient_points(&self) -> bool {
        self.distances.len() >= MIN_POINTS_FOR_ERROR_COUNT
    }

    /// `DetLineFit::EvaluateLineFit`.
    fn evaluate_line_fit(&mut self) -> f64 {
        let mut dist = self.upper_quartile_error();
        if self.distances.len() >= MIN_POINTS_FOR_ERROR_COUNT
            && dist > MAX_REAL_DISTANCE * MAX_REAL_DISTANCE
        {
            let threshold = MAX_REAL_DISTANCE * self.square_length.sqrt();
            dist = self.distances.iter().filter(|d| d.0 > threshold).count() as f64;
        }
        dist
    }

    /// `DetLineFit::ComputeUpperQuartileError`.
    fn upper_quartile_error(&mut self) -> f64 {
        let n = self.distances.len();
        if n == 0 {
            return 0.0;
        }
        for d in &mut self.distances {
            if d.0 < 0.0 {
                d.0 = -d.0;
            }
        }
        let index = 3 * n / 4;
        cxx::nth_element(&mut self.distances, index, &|a, b| a.0 < b.0);
        let dist = self.distances[index].0;
        if self.square_length > 0.0 {
            dist * dist / self.square_length
        } else {
            0.0
        }
    }

    /// `DetLineFit::ComputeDistances`.
    fn compute_distances(&mut self, start: (i32, i32), end: (i32, i32)) {
        self.distances.clear();
        // Tesseract computes these products in `int`; i64 gives the same
        // values wherever that does not overflow and stays sound for any
        // coordinates.
        let lv = (i64::from(end.0 - start.0), i64::from(end.1 - start.1));
        self.square_length = (lv.0 * lv.0 + lv.1 * lv.1) as f32 as f64;
        let line_length = i64::from(int_cast_rounded(self.square_length.sqrt()));
        let mut prev_abs_dist = 0i64;
        let mut prev_dot = 0i64;
        for i in 0..self.pts.len() {
            let pt = self.pts[i].0;
            let pv = (i64::from(pt.0 - start.0), i64::from(pt.1 - start.1));
            let dot = lv.0 * pv.0 + lv.1 * pv.1;
            let dist = lv.0 * pv.1 - lv.1 * pv.0;
            let abs_dist = dist.abs();
            if abs_dist > prev_abs_dist && i > 0 {
                let separation = (dot - prev_dot).abs();
                if separation < line_length * i64::from(self.pts[i].1)
                    || separation < line_length * i64::from(self.pts[i - 1].1)
                {
                    continue;
                }
            }
            self.distances.push((dist as f64, pt));
            prev_abs_dist = abs_dist;
            prev_dot = dot;
        }
    }
}

/// `LLSQ`: linear least squares.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Llsq {
    n: f64,
    sx: f64,
    sy: f64,
    sxx: f64,
    sxy: f64,
    syy: f64,
}

impl Llsq {
    pub(crate) fn add(&mut self, x: f64, y: f64) {
        self.n += 1.0;
        self.sx += x;
        self.sy += y;
        self.sxx += x * x;
        self.sxy += x * y;
        self.syy += y * y;
    }

    fn covariance(&self) -> f64 {
        if self.n > 0.0 {
            (self.sxy - self.sx * self.sy / self.n) / self.n
        } else {
            0.0
        }
    }

    fn x_variance(&self) -> f64 {
        if self.n > 0.0 {
            (self.sxx - self.sx * self.sx / self.n) / self.n
        } else {
            0.0
        }
    }

    /// `LLSQ::m`.
    pub(crate) fn m(&self) -> f64 {
        let v = self.x_variance();
        if v != 0.0 { self.covariance() / v } else { 0.0 }
    }

    /// `LLSQ::c`.
    pub(crate) fn c(&self, m: f64) -> f64 {
        if self.n > 0.0 {
            (self.sy - m * self.sx) / self.n
        } else {
            0.0
        }
    }

    /// `LLSQ::rms`.
    pub(crate) fn rms(&self, m: f64, c: f64) -> f64 {
        if self.n <= 0.0 {
            return 0.0;
        }
        let e = self.syy
            + m * (m * self.sxx + 2.0 * (c * self.sx - self.sxy))
            + c * (self.n * c - 2.0 * self.sy);
        if e >= 0.0 { (e / self.n).sqrt() } else { 0.0 }
    }

    /// `LLSQ::mean_point`.
    pub(crate) fn mean_point(&self) -> (f32, f32) {
        if self.n > 0.0 {
            ((self.sx / self.n) as f32, (self.sy / self.n) as f32)
        } else {
            (0.0, 0.0)
        }
    }
}
