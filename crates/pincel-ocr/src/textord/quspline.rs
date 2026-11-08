//! Quadratic splines for baselines (`src/ccstruct/quspline.cpp`
//! `QSPLINE`, `quadratc.h` `QUAD_COEFFS`, `quadlsq.cpp` `QLSQ`).

/// `kMinVariance`.
const MIN_VARIANCE: f64 = 1.0 / 1024.0;

/// `QUAD_COEFFS`: `y = a·x² + b·x + c`, with `b` and `c` in single
/// precision like Tesseract stores them.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Quad {
    pub(crate) a: f64,
    pub(crate) b: f32,
    pub(crate) c: f32,
}

impl Quad {
    /// `QUAD_COEFFS::y`.
    pub(crate) fn y(&self, x: f32) -> f32 {
        let x = f64::from(x);
        ((self.a * x + f64::from(self.b)) * x + f64::from(self.c)) as f32
    }

    /// `QUAD_COEFFS::move`.
    fn shift(&mut self, p: i32, q: i32) {
        let (p, q) = (p as i16, q as i16);
        let pf = f64::from(p);
        let head = self.c - self.b * f32::from(p);
        self.c = (f64::from(head) + self.a * pf * pf + f64::from(q)) as f32;
        self.b = (f64::from(self.b) - 2.0 * self.a * pf) as f32;
    }
}

/// `QSPLINE`: quadratic pieces between x boundaries.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct QSpline {
    /// `segments + 1` boundaries.
    pub(crate) xcoords: Vec<i32>,
    pub(crate) quads: Vec<Quad>,
}

impl QSpline {
    /// `QSPLINE(count, xstarts, coeffs)`.
    pub(crate) fn from_coeffs(xstarts: &[i32], coeffs: &[f64]) -> Self {
        let segments = coeffs.len() / 3;
        Self {
            xcoords: xstarts[..=segments].to_vec(),
            quads: (0..segments)
                .map(|i| Quad {
                    a: coeffs[i * 3],
                    b: coeffs[i * 3 + 1] as f32,
                    c: coeffs[i * 3 + 2] as f32,
                })
                .collect(),
        }
    }

    /// `QSPLINE(xstarts, segcount, xpts, ypts, pointcount, degree)`: a
    /// least-squares fit of each segment.
    pub(crate) fn fit(
        xstarts: &[i32],
        segcount: usize,
        xpts: &[i32],
        ypts: &[i32],
        degree: i32,
    ) -> Self {
        let pointcount = xpts.len();
        let mut ptcounts = vec![0usize; segcount + 1];
        let mut segment = 0usize;
        for &x in xpts {
            while segment < segcount && x >= xstarts[segment] {
                segment += 1;
                ptcounts[segment] = ptcounts[segment - 1];
            }
            ptcounts[segment] += 1;
        }
        while segment < segcount {
            segment += 1;
            ptcounts[segment] = ptcounts[segment - 1];
        }
        let mut quads = Vec::with_capacity(segcount);
        for segment in 0..segcount {
            let mut q = Qlsq::default();
            let mut pi = ptcounts[segment];
            if pi > 0 && xpts[pi] != xpts[pi - 1] && xpts[pi] != xstarts[segment] {
                q.add(
                    f64::from(xstarts[segment]),
                    f64::from(
                        ypts[pi - 1]
                            + (ypts[pi] - ypts[pi - 1]) * (xstarts[segment] - xpts[pi - 1])
                                / (xpts[pi] - xpts[pi - 1]),
                    ),
                );
            }
            while pi < ptcounts[segment + 1] {
                q.add(f64::from(xpts[pi]), f64::from(ypts[pi]));
                pi += 1;
            }
            if pi > 0 && pi < pointcount && xpts[pi] != xstarts[segment + 1] {
                q.add(
                    f64::from(xstarts[segment + 1]),
                    f64::from(
                        ypts[pi - 1]
                            + (ypts[pi] - ypts[pi - 1]) * (xstarts[segment + 1] - xpts[pi - 1])
                                / (xpts[pi] - xpts[pi - 1]),
                    ),
                );
            }
            let (a, b, c) = q.fit(degree);
            quads.push(Quad {
                a,
                b: b as f32,
                c: c as f32,
            });
        }
        Self {
            xcoords: xstarts[..=segcount].to_vec(),
            quads,
        }
    }

    pub(crate) fn segments(&self) -> usize {
        self.quads.len()
    }

    /// `QSPLINE::spline_index`.
    fn index(&self, x: f64) -> usize {
        let (mut bottom, mut top) = (0usize, self.quads.len());
        while top - bottom > 1 {
            let i = (top + bottom) / 2;
            if x >= f64::from(self.xcoords[i]) {
                bottom = i;
            } else {
                top = i;
            }
        }
        bottom
    }

    /// `QSPLINE::y`.
    pub(crate) fn y(&self, x: f64) -> f64 {
        if self.quads.is_empty() {
            return 0.0;
        }
        f64::from(self.quads[self.index(x)].y(x as f32))
    }

    /// `QSPLINE::step`: the sum of the jumps between `x1` and `x2`.
    pub(crate) fn step(&self, x1: f64, x2: f64) -> f64 {
        if self.quads.is_empty() {
            return 0.0;
        }
        let (mut i1, i2) = (self.index(x1), self.index(x2));
        let mut total = 0.0f64;
        while i1 < i2 {
            let x = self.xcoords[i1 + 1] as f32;
            total += f64::from(self.quads[i1 + 1].y(x));
            total -= f64::from(self.quads[i1].y(x));
            i1 += 1;
        }
        total
    }

    /// `QSPLINE::move`.
    pub(crate) fn shift(&mut self, dx: i32, dy: i32) {
        let dx16 = i32::from(dx as i16);
        for x in &mut self.xcoords {
            *x += dx16;
        }
        for q in &mut self.quads {
            q.shift(dx, dy);
        }
    }

    /// `QSPLINE::extrapolate`: straight pieces of `gradient` out to
    /// `xmin` and `xmax`.
    pub(crate) fn extrapolate(&mut self, gradient: f64, xmin: i32, xmax: i32) {
        let segs = self.segments();
        let grow_left = xmin < self.xcoords[0];
        let grow_right = xmax > self.xcoords[segs];
        if !grow_left && !grow_right {
            return;
        }
        let mut xs = Vec::with_capacity(segs + 3);
        let mut qs = Vec::with_capacity(segs + 2);
        if grow_left {
            xs.push(xmin);
            let x0 = self.xcoords[0];
            let b = gradient as f32;
            qs.push(Quad {
                a: 0.0,
                b,
                c: (self.y(f64::from(x0)) - f64::from(b * x0 as f32)) as f32,
            });
        }
        for s in 0..segs {
            xs.push(self.xcoords[s]);
            qs.push(self.quads[s]);
        }
        xs.push(self.xcoords[segs]);
        if grow_right {
            let xe = self.xcoords[segs];
            let b = gradient as f32;
            qs.push(Quad {
                a: 0.0,
                b,
                c: (self.y(f64::from(xe)) - f64::from(b * xe as f32)) as f32,
            });
            xs.push(xmax + 1);
        }
        self.xcoords = xs;
        self.quads = qs;
    }
}

/// `QLSQ`: least squares for polynomials up to degree two.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Qlsq {
    n: i32,
    sigx: f64,
    sigy: f64,
    sigxx: f64,
    sigxy: f64,
    sigxxx: f64,
    sigxxy: f64,
    sigxxxx: f64,
}

impl Qlsq {
    pub(crate) fn add(&mut self, x: f64, y: f64) {
        self.n += 1;
        self.sigx += x;
        self.sigy += y;
        self.sigxx += x * x;
        self.sigxy += x * y;
        self.sigxxx += x * x * x;
        self.sigxxy += x * x * y;
        self.sigxxxx += x * x * x * x;
    }

    /// `QLSQ::fit`: returns `(a, b, c)`.
    pub(crate) fn fit(&self, degree: i32) -> (f64, f64, f64) {
        let n = f64::from(self.n);
        let x_variance = self.sigxx * n - self.sigx * self.sigx;
        if x_variance < MIN_VARIANCE * n * n || degree < 1 || self.n < 2 {
            let c = if self.n >= 1 && degree >= 0 {
                self.sigy / n
            } else {
                0.0
            };
            return (0.0, 0.0, c);
        }
        let mut top = 0.0f64;
        let mut bottom = 0.0f64;
        let cubevar = self.sigxxx * n - self.sigxx * self.sigx;
        let covariance = self.sigxy * n - self.sigx * self.sigy;
        if self.n >= 4 && degree >= 2 {
            top = cubevar * covariance;
            top += x_variance * (self.sigxx * self.sigy - self.sigxxy * n);
            bottom = cubevar * cubevar;
            bottom -= x_variance * (self.sigxxxx * n - self.sigxx * self.sigxx);
        }
        let (a, b);
        if bottom >= MIN_VARIANCE * n * n * n * n {
            a = top / bottom;
            b = (covariance - cubevar * a) / x_variance;
        } else {
            a = 0.0;
            b = covariance / x_variance;
        }
        let c = (self.sigy - a * self.sigxx - b * self.sigx) / n;
        (a, b, c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_points_on_a_line_gives_that_line() {
        let xs = [0, 10, 20, 30, 40];
        let ys = [5, 6, 7, 8, 9];
        let spline = QSpline::fit(&[-1, 41], 1, &xs, &ys, 2);
        assert_eq!(spline.segments(), 1);
        assert!((spline.y(25.0) - 7.5).abs() < 1e-4);
    }

    #[test]
    fn extrapolate_adds_straight_ends() {
        let mut spline = QSpline::from_coeffs(&[10, 20], &[0.0, 0.0, 3.0]);
        spline.extrapolate(0.0, 0, 30);
        assert_eq!(spline.xcoords, vec![0, 10, 20, 31]);
        assert_eq!(spline.y(5.0), 3.0);
    }
}
