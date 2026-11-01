//! Potrace: bitmap → closed Bézier outlines (Selinger, "Potrace: a
//! polygon-based tracing algorithm"). Ported from the owner's Vectorizer
//! (`daniel-rck/Vectorizer@32dd6c8`, `src/lib/potrace/potrace.ts`); the
//! arithmetic follows the TypeScript step by step so the golden corpus
//! (`docs/specs/vectorize-core.md` AC-01) matches.

use crate::error::{TraceError, try_vec};

/// A point in pixel-edge coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Point {
    pub x: f64,
    pub y: f64,
}

/// How an ambiguous boundary step resolves (Potrace's `turnpolicy`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TurnPolicy {
    /// Prefer connecting filled pixels.
    Black,
    /// Prefer connecting empty pixels.
    White,
    /// Always turn left.
    Left,
    /// Always turn right.
    Right,
    /// Prefer the colour that is in the minority locally.
    #[default]
    Minority,
    /// Prefer the colour that is in the majority locally.
    Majority,
}

/// Potrace parameters (`docs/specs/vectorize-core.md`, Terms).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PotraceParams {
    /// Turn policy for ambiguous steps.
    pub turn_policy: TurnPolicy,
    /// Outlines enclosing at most this many px² are dropped (`turdsize`).
    pub min_area: u32,
    /// Corner threshold (`alphamax`), 0 – 1.34: lower keeps more corners.
    pub alpha_max: f64,
    /// Join consecutive Bézier segments where the tolerance allows.
    pub opt_curve: bool,
    /// Tolerance of the curve optimisation.
    pub opt_tolerance: f64,
}

impl Default for PotraceParams {
    fn default() -> Self {
        Self {
            turn_policy: TurnPolicy::Minority,
            min_area: 2,
            alpha_max: 1.0,
            opt_curve: true,
            opt_tolerance: 0.2,
        }
    }
}

/// A 1-bit image: `1` is filled.
#[derive(Clone, Debug)]
pub(crate) struct Bitmap {
    pub w: i64,
    pub h: i64,
    pub data: Vec<u8>,
}

impl Bitmap {
    /// An empty bitmap, or [`TraceError::OutOfMemory`].
    pub(crate) fn new(w: usize, h: usize) -> Result<Self, TraceError> {
        Ok(Self {
            w: w as i64,
            h: h as i64,
            data: try_vec(w * h, 0_u8)?,
        })
    }
}

/// Segment kind of a traced curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tag {
    Curve,
    Corner,
}

/// The traced curve of one outline: `n` segments, 3 control points each.
#[derive(Clone, Debug)]
pub(crate) struct Curve {
    pub n: usize,
    pub tag: Vec<Tag>,
    pub c: Vec<Point>,
}

/// One traced outline.
#[derive(Clone, Debug)]
pub(crate) struct TracedPath {
    pub curve: Curve,
}

/// Traces `bm` into closed outlines.
pub(crate) fn trace(bm: &Bitmap, params: &PotraceParams) -> Vec<TracedPath> {
    let mut out = Vec::new();
    for mut path in bm_to_pathlist(bm, params) {
        calc_sums(&mut path);
        calc_lon(&mut path);
        best_polygon(&mut path);
        let mut curve = adjust_vertices(&path);
        smooth(&mut curve, params.alpha_max);
        if params.opt_curve {
            curve = opti_curve(&curve, params.opt_tolerance);
        }
        out.push(TracedPath {
            curve: Curve {
                n: curve.n,
                tag: curve.tag,
                c: curve.c,
            },
        });
    }
    out
}

// ---------- helpers (same semantics as the TypeScript) ----------

fn modi(a: i64, n: i64) -> i64 {
    if a >= n {
        a % n
    } else if a >= 0 {
        a
    } else {
        n - 1 - ((-1 - a) % n)
    }
}

fn cyclic(a: i64, b: i64, c: i64) -> bool {
    if a <= c {
        a <= b && b < c
    } else {
        a <= b || b < c
    }
}

fn signf(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

fn signi(x: i64) -> i64 {
    x.signum()
}

fn xprod(p1: (i64, i64), p2: (i64, i64)) -> i64 {
    p1.0 * p2.1 - p1.1 * p2.0
}

fn ddist(p: Point, q: Point) -> f64 {
    ((p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y)).sqrt()
}

fn dpara(p0: Point, p1: Point, p2: Point) -> f64 {
    let x1 = p1.x - p0.x;
    let y1 = p1.y - p0.y;
    let x2 = p2.x - p0.x;
    let y2 = p2.y - p0.y;
    x1 * y2 - x2 * y1
}

fn cprod(p0: Point, p1: Point, p2: Point, p3: Point) -> f64 {
    let x1 = p1.x - p0.x;
    let y1 = p1.y - p0.y;
    let x2 = p3.x - p2.x;
    let y2 = p3.y - p2.y;
    x1 * y2 - x2 * y1
}

fn iprod(p0: Point, p1: Point, p2: Point) -> f64 {
    let x1 = p1.x - p0.x;
    let y1 = p1.y - p0.y;
    let x2 = p2.x - p0.x;
    let y2 = p2.y - p0.y;
    x1 * x2 + y1 * y2
}

fn iprod1(p0: Point, p1: Point, p2: Point, p3: Point) -> f64 {
    let x1 = p1.x - p0.x;
    let y1 = p1.y - p0.y;
    let x2 = p3.x - p2.x;
    let y2 = p3.y - p2.y;
    x1 * x2 + y1 * y2
}

fn interval(lambda: f64, a: Point, b: Point) -> Point {
    Point {
        x: a.x + lambda * (b.x - a.x),
        y: a.y + lambda * (b.y - a.y),
    }
}

fn ddenom(p0: Point, p2: Point) -> f64 {
    let rx = -signf(p2.y - p0.y);
    let ry = signf(p2.x - p0.x);
    ry * (p2.x - p0.x) - rx * (p2.y - p0.y)
}

fn bezier(t: f64, p0: Point, p1: Point, p2: Point, p3: Point) -> Point {
    let s = 1.0 - t;
    Point {
        x: s * s * s * p0.x
            + 3.0 * (s * s * t) * p1.x
            + 3.0 * (t * t * s) * p2.x
            + t * t * t * p3.x,
        y: s * s * s * p0.y
            + 3.0 * (s * s * t) * p1.y
            + 3.0 * (t * t * s) * p2.y
            + t * t * t * p3.y,
    }
}

fn tangent(p0: Point, p1: Point, p2: Point, p3: Point, q0: Point, q1: Point) -> f64 {
    let a_ = cprod(p0, p1, q0, q1);
    let b_ = cprod(p1, p2, q0, q1);
    let c_ = cprod(p2, p3, q0, q1);
    let a = a_ - 2.0 * b_ + c_;
    let b = -2.0 * a_ + 2.0 * b_;
    let c = a_;
    let d = b * b - 4.0 * a * c;
    if a == 0.0 || d < 0.0 {
        return -1.0;
    }
    let s = d.sqrt();
    let r1 = (-b + s) / (2.0 * a);
    let r2 = (-b - s) / (2.0 * a);
    if (0.0..=1.0).contains(&r1) {
        return r1;
    }
    if (0.0..=1.0).contains(&r2) {
        return r2;
    }
    -1.0
}

// ---------- Phase 1: bitmap -> path list ----------

#[derive(Clone, Copy, Default)]
struct Sums {
    x: f64,
    y: f64,
    xy: f64,
    x2: f64,
    y2: f64,
}

#[derive(Default)]
struct RawPath {
    area: i64,
    pt: Vec<(i64, i64)>,
    max_x: i64,
    x0: i64,
    y0: i64,
    sums: Vec<Sums>,
    lon: Vec<i64>,
    po: Vec<i64>,
    m: usize,
}

fn at(bm: &Bitmap, x: i64, y: i64) -> bool {
    x >= 0 && x < bm.w && y >= 0 && y < bm.h && bm.data[(bm.w * y + x) as usize] == 1
}

fn majority(bm1: &Bitmap, x: i64, y: i64) -> i64 {
    for i in 2..5 {
        let mut ct = 0;
        for a in (-i + 1)..=(i - 1) {
            ct += if at(bm1, x + a, y + i - 1) { 1 } else { -1 };
            ct += if at(bm1, x + i - 1, y + a - 1) { 1 } else { -1 };
            ct += if at(bm1, x + a - 1, y - i) { 1 } else { -1 };
            ct += if at(bm1, x - i, y + a) { 1 } else { -1 };
        }
        if ct > 0 {
            return 1;
        }
        if ct < 0 {
            return 0;
        }
    }
    0
}

fn find_path(bm: &Bitmap, bm1: &Bitmap, start: (i64, i64), policy: TurnPolicy) -> RawPath {
    let mut path = RawPath {
        max_x: -1,
        ..RawPath::default()
    };
    let (mut x, mut y) = start;
    let (mut dirx, mut diry) = (0_i64, 1_i64);
    let positive = at(bm, start.0, start.1);
    loop {
        path.pt.push((x, y));
        if x > path.max_x {
            path.max_x = x;
        }
        x += dirx;
        y += diry;
        path.area -= x * diry;
        if x == start.0 && y == start.1 {
            break;
        }
        let l = at(bm1, x + (dirx + diry - 1) / 2, y + (diry - dirx - 1) / 2);
        let r = at(bm1, x + (dirx - diry - 1) / 2, y + (diry + dirx - 1) / 2);
        if r && !l {
            let left = match policy {
                TurnPolicy::Right => true,
                TurnPolicy::Black => positive,
                TurnPolicy::White => !positive,
                TurnPolicy::Majority => majority(bm1, x, y) != 0,
                TurnPolicy::Minority => majority(bm1, x, y) == 0,
                TurnPolicy::Left => false,
            };
            let tmp = dirx;
            if left {
                dirx = -diry;
                diry = tmp;
            } else {
                dirx = diry;
                diry = -tmp;
            }
        } else if r {
            let tmp = dirx;
            dirx = -diry;
            diry = tmp;
        } else if !l {
            let tmp = dirx;
            dirx = diry;
            diry = -tmp;
        }
    }
    path
}

fn xor_path(bm1: &mut Bitmap, path: &RawPath) {
    let mut y1 = path.pt[0].1;
    for &(x, y) in &path.pt[1..] {
        if y != y1 {
            let min_y = y1.min(y);
            for j in x..path.max_x {
                let i = (bm1.w * min_y + j) as usize;
                bm1.data[i] = u8::from(bm1.data[i] == 0);
            }
            y1 = y;
        }
    }
}

fn bm_to_pathlist(bm: &Bitmap, params: &PotraceParams) -> Vec<RawPath> {
    let mut bm1 = bm.clone();
    let size = bm1.w * bm1.h;
    let mut pathlist = Vec::new();
    let mut i = 0_i64;
    loop {
        while i < size && bm1.data[i as usize] != 1 {
            i += 1;
        }
        if i >= size {
            break;
        }
        let start = (i % bm1.w, i / bm1.w);
        let path = find_path(bm, &bm1, start, params.turn_policy);
        xor_path(&mut bm1, &path);
        if path.area > i64::from(params.min_area) {
            pathlist.push(path);
        }
    }
    pathlist
}

// ---------- Phase 2: path -> polygon ----------

fn calc_sums(path: &mut RawPath) {
    path.x0 = path.pt[0].0;
    path.y0 = path.pt[0].1;
    let mut sums = Vec::with_capacity(path.pt.len() + 1);
    sums.push(Sums::default());
    for (i, &(px, py)) in path.pt.iter().enumerate() {
        let x = (px - path.x0) as f64;
        let y = (py - path.y0) as f64;
        let s: Sums = sums[i];
        sums.push(Sums {
            x: s.x + x,
            y: s.y + y,
            xy: s.xy + x * y,
            x2: s.x2 + x * x,
            y2: s.y2 + y * y,
        });
    }
    path.sums = sums;
}

/// `ct[dir]++` with `dir = (3 + 3 sx + sy) / 2`: in the TypeScript a
/// fractional `dir` (both or neither of sx, sy non-zero) sets a stray
/// property instead of a counter, so only even numerators count.
fn bump(ct: &mut [i64; 4], sx: i64, sy: i64) {
    let v = 3 + 3 * sx + sy;
    if v % 2 == 0 && (0..8).contains(&v) {
        ct[(v / 2) as usize] += 1;
    }
}

fn calc_lon(path: &mut RawPath) {
    let pt = &path.pt;
    let n = pt.len() as i64;
    let nu = pt.len();
    let mut pivk = vec![0_i64; nu];
    let mut nc = vec![0_i64; nu];
    let mut lon = vec![0_i64; nu];

    let mut k: i64 = 0;
    for i in (0..n).rev() {
        let pk = pt[k as usize];
        if pt[i as usize].0 != pk.0 && pt[i as usize].1 != pk.1 {
            k = i + 1;
        }
        nc[i as usize] = k;
    }

    for i in (0..n).rev() {
        let mut ct = [0_i64; 4];
        let next = pt[modi(i + 1, n) as usize];
        bump(
            &mut ct,
            next.0 - pt[i as usize].0,
            next.1 - pt[i as usize].1,
        );
        let mut constraint = [(0_i64, 0_i64), (0_i64, 0_i64)];

        k = nc[i as usize];
        let mut k1 = i;
        let mut foundk = false;
        loop {
            let pk = pt[k as usize];
            let pk1 = pt[k1 as usize];
            bump(&mut ct, signi(pk.0 - pk1.0), signi(pk.1 - pk1.1));
            if ct[0] != 0 && ct[1] != 0 && ct[2] != 0 && ct[3] != 0 {
                pivk[i as usize] = k1;
                foundk = true;
                break;
            }
            let cur = (pk.0 - pt[i as usize].0, pk.1 - pt[i as usize].1);
            if xprod(constraint[0], cur) < 0 || xprod(constraint[1], cur) > 0 {
                break;
            }
            if !(cur.0.abs() <= 1 && cur.1.abs() <= 1) {
                let off = (
                    cur.0
                        + if cur.1 >= 0 && (cur.1 > 0 || cur.0 < 0) {
                            1
                        } else {
                            -1
                        },
                    cur.1
                        + if cur.0 <= 0 && (cur.0 < 0 || cur.1 < 0) {
                            1
                        } else {
                            -1
                        },
                );
                if xprod(constraint[0], off) >= 0 {
                    constraint[0] = off;
                }
                let off = (
                    cur.0
                        + if cur.1 <= 0 && (cur.1 < 0 || cur.0 < 0) {
                            1
                        } else {
                            -1
                        },
                    cur.1
                        + if cur.0 >= 0 && (cur.0 > 0 || cur.1 < 0) {
                            1
                        } else {
                            -1
                        },
                );
                if xprod(constraint[1], off) <= 0 {
                    constraint[1] = off;
                }
            }
            k1 = k;
            k = nc[k1 as usize];
            if !cyclic(k, i, k1) {
                break;
            }
        }
        if !foundk {
            let pk = pt[k as usize];
            let pk1 = pt[k1 as usize];
            let dk = (signi(pk.0 - pk1.0), signi(pk.1 - pk1.1));
            let cur = (pk1.0 - pt[i as usize].0, pk1.1 - pt[i as usize].1);
            let a = xprod(constraint[0], cur);
            let b = xprod(constraint[0], dk);
            let c = xprod(constraint[1], cur);
            let d = xprod(constraint[1], dk);
            let mut j: i64 = 10_000_000;
            if b < 0 {
                j = (a as f64 / -b as f64).floor() as i64;
            }
            if d > 0 {
                j = j.min((-c as f64 / d as f64).floor() as i64);
            }
            pivk[i as usize] = modi(k1 + j, n);
        }
    }

    let mut j = pivk[nu - 1];
    lon[nu - 1] = j;
    for i in (0..n - 1).rev() {
        if cyclic(i + 1, pivk[i as usize], j) {
            j = pivk[i as usize];
        }
        lon[i as usize] = j;
    }
    let mut i = n - 1;
    while i >= 0 && cyclic(modi(i + 1, n), j, lon[i as usize]) {
        lon[i as usize] = j;
        i -= 1;
    }
    path.lon = lon;
}

fn penalty3(path: &RawPath, i: i64, mut j: i64) -> f64 {
    let n = path.pt.len() as i64;
    let pt = &path.pt;
    let sums = &path.sums;
    let mut r = 0;
    if j >= n {
        j -= n;
        r = 1;
    }
    let (iu, ju, nu) = (i as usize, j as usize, n as usize);
    let (x, y, x2, xy, y2, k) = if r == 0 {
        (
            sums[ju + 1].x - sums[iu].x,
            sums[ju + 1].y - sums[iu].y,
            sums[ju + 1].x2 - sums[iu].x2,
            sums[ju + 1].xy - sums[iu].xy,
            sums[ju + 1].y2 - sums[iu].y2,
            (j + 1 - i) as f64,
        )
    } else {
        (
            sums[ju + 1].x - sums[iu].x + sums[nu].x,
            sums[ju + 1].y - sums[iu].y + sums[nu].y,
            sums[ju + 1].x2 - sums[iu].x2 + sums[nu].x2,
            sums[ju + 1].xy - sums[iu].xy + sums[nu].xy,
            sums[ju + 1].y2 - sums[iu].y2 + sums[nu].y2,
            (j + 1 - i + n) as f64,
        )
    };
    let px = (pt[iu].0 + pt[ju].0) as f64 / 2.0 - pt[0].0 as f64;
    let py = (pt[iu].1 + pt[ju].1) as f64 / 2.0 - pt[0].1 as f64;
    let ey = (pt[ju].0 - pt[iu].0) as f64;
    let ex = -((pt[ju].1 - pt[iu].1) as f64);
    let a = (x2 - 2.0 * x * px) / k + px * px;
    let b = (xy - x * py - y * px) / k + px * py;
    let c = (y2 - 2.0 * y * py) / k + py * py;
    let s = ex * ex * a + 2.0 * ex * ey * b + ey * ey * c;
    s.sqrt()
}

fn best_polygon(path: &mut RawPath) {
    let n = path.pt.len() as i64;
    let nu = path.pt.len();
    let mut pen = vec![0.0_f64; nu + 1];
    let mut prev = vec![0_i64; nu + 1];
    let mut clip0 = vec![0_i64; nu];
    let mut clip1 = vec![0_i64; nu + 1];
    let mut seg0 = vec![0_i64; nu + 1];
    let mut seg1 = vec![0_i64; nu + 1];

    for i in 0..n {
        let mut c = modi(path.lon[modi(i - 1, n) as usize] - 1, n);
        if c == i {
            c = modi(i + 1, n);
        }
        clip0[i as usize] = if c < i { n } else { c };
    }
    let mut j: i64 = 1;
    for i in 0..n {
        while j <= clip0[i as usize] {
            clip1[j as usize] = i;
            j += 1;
        }
    }
    let mut i: i64 = 0;
    j = 0;
    while i < n {
        seg0[j as usize] = i;
        i = clip0[i as usize];
        j += 1;
    }
    seg0[j as usize] = n;
    let m = j;
    i = n;
    j = m;
    while j > 0 {
        seg1[j as usize] = i;
        i = clip1[i as usize];
        j -= 1;
    }
    seg1[0] = 0;

    pen[0] = 0.0;
    for j in 1..=m {
        let mut i = seg1[j as usize];
        while i <= seg0[j as usize] {
            let mut best = -1.0_f64;
            let mut k = seg0[(j - 1) as usize];
            while k >= clip1[i as usize] {
                let thispen = penalty3(path, k, i) + pen[k as usize];
                if best < 0.0 || thispen < best {
                    prev[i as usize] = k;
                    best = thispen;
                }
                k -= 1;
            }
            pen[i as usize] = best;
            i += 1;
        }
    }
    path.m = m as usize;
    let mut po = vec![0_i64; m as usize];
    let mut i = n;
    let mut j = m - 1;
    while i > 0 {
        i = prev[i as usize];
        po[j as usize] = i;
        j -= 1;
    }
    path.po = po;
}

// ---------- Phase 3: polygon -> vertices ----------

struct WorkCurve {
    n: usize,
    tag: Vec<Tag>,
    c: Vec<Point>,
    vertex: Vec<Point>,
    alpha: Vec<f64>,
}

impl WorkCurve {
    fn new(n: usize) -> Self {
        Self {
            n,
            tag: vec![Tag::Corner; n],
            c: vec![Point::default(); n * 3],
            vertex: vec![Point::default(); n],
            alpha: vec![0.0; n],
        }
    }
}

fn pointslope(path: &RawPath, mut i: i64, mut j: i64) -> (Point, Point) {
    let n = path.pt.len() as i64;
    let sums = &path.sums;
    let mut r: i64 = 0;
    while j >= n {
        j -= n;
        r += 1;
    }
    while i >= n {
        i -= n;
        r -= 1;
    }
    while j < 0 {
        j += n;
        r -= 1;
    }
    while i < 0 {
        i += n;
        r += 1;
    }
    let (iu, ju, nu) = (i as usize, j as usize, n as usize);
    let rf = r as f64;
    let x = sums[ju + 1].x - sums[iu].x + rf * sums[nu].x;
    let y = sums[ju + 1].y - sums[iu].y + rf * sums[nu].y;
    let x2 = sums[ju + 1].x2 - sums[iu].x2 + rf * sums[nu].x2;
    let xy = sums[ju + 1].xy - sums[iu].xy + rf * sums[nu].xy;
    let y2 = sums[ju + 1].y2 - sums[iu].y2 + rf * sums[nu].y2;
    let k = (j + 1 - i + r * n) as f64;
    let ctr = Point { x: x / k, y: y / k };
    let mut a = (x2 - (x * x) / k) / k;
    let b = (xy - (x * y) / k) / k;
    let mut c = (y2 - (y * y) / k) / k;
    let lambda2 = (a + c + ((a - c) * (a - c) + 4.0 * b * b).sqrt()) / 2.0;
    a -= lambda2;
    c -= lambda2;
    let mut dir = Point::default();
    let l;
    if a.abs() >= c.abs() {
        l = (a * a + b * b).sqrt();
        if l != 0.0 {
            dir = Point {
                x: -b / l,
                y: a / l,
            };
        }
    } else {
        l = (c * c + b * b).sqrt();
        if l != 0.0 {
            dir = Point {
                x: -c / l,
                y: b / l,
            };
        }
    }
    if l == 0.0 {
        dir = Point::default();
    }
    (ctr, dir)
}

type Mat3 = [[f64; 3]; 3];

fn quadform(q: &Mat3, w: Point) -> f64 {
    let v = [w.x, w.y, 1.0];
    let mut sum = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            sum += v[i] * q[i][j] * v[j];
        }
    }
    sum
}

fn adjust_vertices(path: &RawPath) -> WorkCurve {
    let m = path.m;
    let mi = m as i64;
    let po = &path.po;
    let n = path.pt.len() as i64;
    let pt = &path.pt;
    let (x0, y0) = (path.x0 as f64, path.y0 as f64);
    let mut ctr = vec![Point::default(); m];
    let mut dir = vec![Point::default(); m];
    let mut q = vec![[[0.0_f64; 3]; 3]; m];
    let mut curve = WorkCurve::new(m);

    for i in 0..m {
        let mut j = po[modi(i as i64 + 1, mi) as usize];
        j = modi(j - po[i], n) + po[i];
        (ctr[i], dir[i]) = pointslope(path, po[i], j);
    }
    for i in 0..m {
        let d = dir[i].x * dir[i].x + dir[i].y * dir[i].y;
        if d != 0.0 {
            let v0 = dir[i].y;
            let v1 = -dir[i].x;
            let v = [v0, v1, -v1 * ctr[i].y - v0 * ctr[i].x];
            for l in 0..3 {
                for k in 0..3 {
                    q[i][l][k] = (v[l] * v[k]) / d;
                }
            }
        }
    }
    for i in 0..m {
        let mut q2: Mat3 = [[0.0; 3]; 3];
        let s = Point {
            x: pt[po[i] as usize].0 as f64 - x0,
            y: pt[po[i] as usize].1 as f64 - y0,
        };
        let j = modi(i as i64 - 1, mi) as usize;
        for l in 0..3 {
            for k in 0..3 {
                q2[l][k] = q[j][l][k] + q[i][l][k];
            }
        }
        let det = q2[0][0] * q2[1][1] - q2[0][1] * q2[1][0];
        if det != 0.0 {
            let w0 = (-q2[0][2] * q2[1][1] + q2[1][2] * q2[0][1]) / det;
            let w1 = (q2[0][2] * q2[1][0] - q2[1][2] * q2[0][0]) / det;
            if (w0 - s.x).abs() <= 0.5 && (w1 - s.y).abs() <= 0.5 {
                curve.vertex[i] = Point {
                    x: w0 + x0,
                    y: w1 + y0,
                };
                continue;
            }
        }
        let mut min = quadform(&q2, s);
        let mut xv = s.x;
        let mut yv = s.y;
        let cand_at = |a: f64, b: f64| {
            q2[0][0] * a * a
                + 2.0 * q2[1][0] * a * b
                + q2[1][1] * b * b
                + 2.0 * q2[2][0] * a
                + 2.0 * q2[2][1] * b
                + q2[2][2]
        };
        if q2[0][0] != 0.0 {
            for z in 0..2 {
                let wv = s.y - 0.5 + f64::from(z);
                let ww = -(q2[1][0] * wv + q2[2][0]) / q2[0][0];
                let cand = cand_at(ww, wv);
                if (ww - s.x).abs() <= 0.5 && cand < min {
                    min = cand;
                    xv = ww;
                    yv = wv;
                }
            }
        }
        if q2[1][1] != 0.0 {
            for z in 0..2 {
                let wv = s.x - 0.5 + f64::from(z);
                let ww = -(q2[0][1] * wv + q2[2][1]) / q2[1][1];
                let cand = cand_at(wv, ww);
                if (ww - s.y).abs() <= 0.5 && cand < min {
                    min = cand;
                    xv = wv;
                    yv = ww;
                }
            }
        }
        for l in 0..2 {
            for k in 0..2 {
                let wv = s.x - 0.5 + f64::from(l);
                let ww = s.y - 0.5 + f64::from(k);
                let cand = cand_at(wv, ww);
                if cand < min {
                    min = cand;
                    xv = wv;
                    yv = ww;
                }
            }
        }
        curve.vertex[i] = Point {
            x: xv + x0,
            y: yv + y0,
        };
    }
    curve
}

// ---------- Phase 4: smoothing and corners ----------

fn smooth(curve: &mut WorkCurve, alpha_max: f64) {
    let m = curve.n as i64;
    for i in 0..m {
        let j = modi(i + 1, m) as usize;
        let k = modi(i + 2, m) as usize;
        let iu = i as usize;
        let p4 = interval(0.5, curve.vertex[k], curve.vertex[j]);
        let denom = ddenom(curve.vertex[iu], curve.vertex[k]);
        let mut alpha;
        if denom != 0.0 {
            let dd = (dpara(curve.vertex[iu], curve.vertex[j], curve.vertex[k]) / denom).abs();
            alpha = if dd > 1.0 { 1.0 - 1.0 / dd } else { 0.0 };
            alpha /= 0.75;
        } else {
            alpha = 4.0 / 3.0;
        }
        if alpha >= alpha_max {
            curve.tag[j] = Tag::Corner;
            curve.c[3 * j + 1] = curve.vertex[j];
            curve.c[3 * j + 2] = p4;
        } else {
            alpha = alpha.clamp(0.55, 1.0);
            let p2 = interval(0.5 + 0.5 * alpha, curve.vertex[iu], curve.vertex[j]);
            let p3 = interval(0.5 + 0.5 * alpha, curve.vertex[k], curve.vertex[j]);
            curve.tag[j] = Tag::Curve;
            curve.c[3 * j] = p2;
            curve.c[3 * j + 1] = p3;
            curve.c[3 * j + 2] = p4;
        }
        curve.alpha[j] = alpha;
    }
}

// ---------- Phase 5: curve optimisation ----------

#[derive(Clone, Copy, Default)]
struct Opti {
    pen: f64,
    c: [Point; 2],
    s: f64,
    alpha: f64,
}

fn opti_penalty(
    curve: &WorkCurve,
    i: i64,
    j: i64,
    opttolerance: f64,
    convc: &[i64],
    areac: &[f64],
) -> Option<Opti> {
    let m = curve.n as i64;
    let vertex = &curve.vertex;
    let c = &curve.c;
    let v = |idx: i64| vertex[idx as usize];
    let cp = |idx: i64| c[(idx * 3 + 2) as usize];
    if i == j {
        return None;
    }
    let i1 = modi(i + 1, m);
    let mut k1 = modi(i + 1, m);
    let conv = convc[k1 as usize];
    if conv == 0 {
        return None;
    }
    let d0 = ddist(v(i), v(i1));
    let mut k = k1;
    while k != j {
        k1 = modi(k + 1, m);
        let k2 = modi(k + 2, m);
        if convc[k1 as usize] != conv {
            return None;
        }
        if signf(cprod(v(i), v(i1), v(k1), v(k2))) as i64 != conv {
            return None;
        }
        if iprod1(v(i), v(i1), v(k1), v(k2)) < d0 * ddist(v(k1), v(k2)) * -0.999847695156 {
            return None;
        }
        k = k1;
    }
    let p0 = cp(modi(i, m));
    let p1 = v(modi(i + 1, m));
    let p2 = v(modi(j, m));
    let p3 = cp(modi(j, m));

    let mut area = areac[j as usize] - areac[i as usize];
    area -= dpara(v(0), cp(i), cp(j)) / 2.0;
    if i >= j {
        area += areac[m as usize];
    }
    let a1 = dpara(p0, p1, p2);
    let a2 = dpara(p0, p1, p3);
    let a3 = dpara(p0, p2, p3);
    let a4 = a1 + a3 - a2;
    if a2 == a1 {
        return None;
    }
    let t = a3 / (a3 - a4);
    let s = a2 / (a2 - a1);
    let a = (a2 * t) / 2.0;
    if a == 0.0 {
        return None;
    }
    let r = area / a;
    let alpha = 2.0 - (4.0 - r / 0.3).sqrt();
    let mut res = Opti {
        pen: 0.0,
        c: [interval(t * alpha, p0, p1), interval(s * alpha, p3, p2)],
        s,
        alpha,
    };
    let (p1, p2) = (res.c[0], res.c[1]);

    let mut k = modi(i + 1, m);
    while k != j {
        let k1 = modi(k + 1, m);
        let tt = tangent(p0, p1, p2, p3, v(k), v(k1));
        if tt < -0.5 {
            return None;
        }
        let pt = bezier(tt, p0, p1, p2, p3);
        let dd = ddist(v(k), v(k1));
        if dd == 0.0 {
            return None;
        }
        let d1 = dpara(v(k), v(k1), pt) / dd;
        if d1.abs() > opttolerance {
            return None;
        }
        if iprod(v(k), v(k1), pt) < 0.0 || iprod(v(k1), v(k), pt) < 0.0 {
            return None;
        }
        res.pen += d1 * d1;
        k = k1;
    }
    let mut k = i;
    while k != j {
        let k1 = modi(k + 1, m);
        let tt = tangent(p0, p1, p2, p3, cp(k), cp(k1));
        if tt < -0.5 {
            return None;
        }
        let pt = bezier(tt, p0, p1, p2, p3);
        let dd = ddist(cp(k), cp(k1));
        if dd == 0.0 {
            return None;
        }
        let mut d1 = dpara(cp(k), cp(k1), pt) / dd;
        let mut d2 = dpara(cp(k), cp(k1), v(k1)) / dd;
        d2 *= 0.75 * curve.alpha[k1 as usize];
        if d2 < 0.0 {
            d1 = -d1;
            d2 = -d2;
        }
        if d1 < d2 - opttolerance {
            return None;
        }
        if d1 < d2 {
            res.pen += (d1 - d2) * (d1 - d2);
        }
        k = k1;
    }
    Some(res)
}

fn opti_curve(curve: &WorkCurve, opttolerance: f64) -> WorkCurve {
    let m = curve.n;
    let mi = m as i64;
    let vert = &curve.vertex;
    let mut pt = vec![0_i64; m + 1];
    let mut pen = vec![0.0_f64; m + 1];
    let mut len = vec![0_i64; m + 1];
    let mut opt: Vec<Option<Opti>> = vec![None; m + 1];
    let mut convc = vec![0_i64; m];
    let mut areac = vec![0.0_f64; m + 1];

    for i in 0..mi {
        convc[i as usize] = if curve.tag[i as usize] == Tag::Curve {
            signf(dpara(
                vert[modi(i - 1, mi) as usize],
                vert[i as usize],
                vert[modi(i + 1, mi) as usize],
            )) as i64
        } else {
            0
        };
    }
    let mut area = 0.0;
    areac[0] = 0.0;
    let p0 = curve.vertex[0];
    for i in 0..mi {
        let i1 = modi(i + 1, mi) as usize;
        let iu = i as usize;
        if curve.tag[i1] == Tag::Curve {
            let a = curve.alpha[i1];
            area +=
                (0.3 * a * (4.0 - a) * dpara(curve.c[iu * 3 + 2], vert[i1], curve.c[i1 * 3 + 2]))
                    / 2.0;
            area += dpara(p0, curve.c[iu * 3 + 2], curve.c[i1 * 3 + 2]) / 2.0;
        }
        areac[iu + 1] = area;
    }

    pt[0] = -1;
    pen[0] = 0.0;
    len[0] = 0;
    for j in 1..=mi {
        let ju = j as usize;
        pt[ju] = j - 1;
        pen[ju] = pen[ju - 1];
        len[ju] = len[ju - 1] + 1;
        let mut i = j - 2;
        while i >= 0 {
            let iu = i as usize;
            let Some(o) = opti_penalty(curve, i, modi(j, mi), opttolerance, &convc, &areac) else {
                break;
            };
            if len[ju] > len[iu] + 1 || (len[ju] == len[iu] + 1 && pen[ju] > pen[iu] + o.pen) {
                pt[ju] = i;
                pen[ju] = pen[iu] + o.pen;
                len[ju] = len[iu] + 1;
                opt[ju] = Some(o);
            }
            i -= 1;
        }
    }
    let om = len[m] as usize;
    let mut ocurve = WorkCurve::new(om);
    let mut j = mi;
    for i in (0..om).rev() {
        let ju = j as usize;
        let jm = modi(j, mi) as usize;
        match opt[ju] {
            Some(oj) if pt[ju] != j - 1 => {
                ocurve.tag[i] = Tag::Curve;
                ocurve.c[i * 3] = oj.c[0];
                ocurve.c[i * 3 + 1] = oj.c[1];
                ocurve.c[i * 3 + 2] = curve.c[jm * 3 + 2];
                ocurve.vertex[i] = interval(oj.s, curve.c[jm * 3 + 2], vert[jm]);
                ocurve.alpha[i] = oj.alpha;
            }
            _ => {
                ocurve.tag[i] = curve.tag[jm];
                ocurve.c[i * 3] = curve.c[jm * 3];
                ocurve.c[i * 3 + 1] = curve.c[jm * 3 + 1];
                ocurve.c[i * 3 + 2] = curve.c[jm * 3 + 2];
                ocurve.vertex[i] = curve.vertex[jm];
                ocurve.alpha[i] = curve.alpha[jm];
            }
        }
        j = pt[ju];
    }
    ocurve
}

// ---------- SVG path data ----------

/// JavaScript's `Math.round`: nearest integer, ties towards +∞.
pub(crate) fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 { f + 1.0 } else { f }
}

fn q3(n: f64) -> f64 {
    js_round(n * 1000.0) / 1000.0
}

/// A number as JavaScript prints it after `q3`, with the leading zero of
/// `0.x` / `-0.x` dropped.
fn nfmt(n: f64) -> String {
    let v = q3(n);
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{v}");
    if let Some(rest) = s.strip_prefix("0.") {
        format!(".{rest}")
    } else if let Some(rest) = s.strip_prefix("-0.") {
        format!("-.{rest}")
    } else {
        s
    }
}

fn pair(x: f64, y: f64) -> String {
    let sy = nfmt(y);
    let sep = if sy.starts_with('-') { "" } else { " " };
    format!("{}{sep}{sy}", nfmt(x))
}

/// Serialises traced outlines as SVG path data: one absolute move per
/// outline, then relative steps on a 1/1000 grid accumulated so rounding
/// never drifts, then `z`.
pub(crate) fn path_data(paths: &[TracedPath]) -> String {
    let mut out = String::new();
    for path in paths {
        let cv = &path.curve;
        let n = cv.n;
        if n == 0 {
            continue;
        }
        let mut cx = q3(cv.c[(n - 1) * 3 + 2].x);
        let mut cy = q3(cv.c[(n - 1) * 3 + 2].y);
        out.push('M');
        out.push_str(&pair(cx, cy));
        for i in 0..n {
            if cv.tag[i] == Tag::Curve {
                let (c1, c2, c3) = (cv.c[i * 3], cv.c[i * 3 + 1], cv.c[i * 3 + 2]);
                let (sx, sy) = (cx, cy);
                let d1x = q3(q3(c1.x) - sx);
                let d1y = q3(q3(c1.y) - sy);
                let d2x = q3(q3(c2.x) - sx);
                let d2y = q3(q3(c2.y) - sy);
                let d3x = q3(q3(c3.x) - sx);
                let d3y = q3(q3(c3.y) - sy);
                out.push('c');
                out.push_str(&pair(d1x, d1y));
                out.push(' ');
                out.push_str(&pair(d2x, d2y));
                out.push(' ');
                out.push_str(&pair(d3x, d3y));
                cx = q3(sx + d3x);
                cy = q3(sy + d3y);
            } else {
                for p in [cv.c[i * 3 + 1], cv.c[i * 3 + 2]] {
                    let dx = q3(q3(p.x) - cx);
                    let dy = q3(q3(p.y) - cy);
                    cx = q3(cx + dx);
                    cy = q3(cy + dy);
                    out.push('l');
                    out.push_str(&pair(dx, dy));
                }
            }
        }
        out.push('z');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bitmap(w: usize, h: usize, f: impl Fn(i64, i64) -> bool) -> Bitmap {
        let mut bm = Bitmap::new(w, h).expect("bitmap");
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                bm.data[(y * w as i64 + x) as usize] = u8::from(f(x, y));
            }
        }
        bm
    }

    fn bezier(t: f64, p0: Point, p1: Point, p2: Point, p3: Point) -> Point {
        let s = 1.0 - t;
        Point {
            x: s * s * s * p0.x
                + 3.0 * s * s * t * p1.x
                + 3.0 * s * t * t * p2.x
                + t * t * t * p3.x,
            y: s * s * s * p0.y
                + 3.0 * s * s * t * p1.y
                + 3.0 * s * t * t * p2.y
                + t * t * t * p3.y,
        }
    }

    #[test]
    fn trace_disc_gives_few_segments_close_to_the_circle() {
        let bm = bitmap(100, 100, |x, y| {
            (x - 50).pow(2) + (y - 50).pow(2) <= 30 * 30
        });
        let paths = trace(&bm, &PotraceParams::default());
        assert_eq!(paths.len(), 1);
        let cv = &paths[0].curve;
        assert!(cv.n <= 6, "{} segments", cv.n);
        let mut prev = cv.c[(cv.n - 1) * 3 + 2];
        let mut worst: f64 = 0.0;
        for i in 0..cv.n {
            let end = cv.c[i * 3 + 2];
            if cv.tag[i] == Tag::Curve {
                for step in 0..=10 {
                    let p = bezier(
                        f64::from(step) / 10.0,
                        prev,
                        cv.c[i * 3],
                        cv.c[i * 3 + 1],
                        end,
                    );
                    let err = ((p.x - 50.0).hypot(p.y - 50.0) - 30.0).abs();
                    worst = worst.max(err);
                }
            }
            prev = end;
        }
        assert!(worst < 1.5, "worst deviation {worst}");
    }

    #[test]
    fn trace_ring_gives_outline_and_hole() {
        let bm = bitmap(120, 120, |x, y| {
            let d = (x - 60).pow(2) + (y - 60).pow(2);
            (20 * 20..=45 * 45).contains(&d)
        });
        assert_eq!(trace(&bm, &PotraceParams::default()).len(), 2);
    }

    #[test]
    fn trace_rectangle_has_exact_corners_and_finite_numbers() {
        let bm = bitmap(80, 60, |x, y| {
            (10..70).contains(&x) && (10..50).contains(&y)
        });
        let paths = trace(&bm, &PotraceParams::default());
        let d = path_data(&paths);
        assert!(d.starts_with("M10 30"), "{d}");
        for bad in ["NaN", "inf", "  "] {
            assert!(!d.contains(bad), "{d}");
        }
        assert_eq!(paths.len(), 1);
        let cv = &paths[0].curve;
        let mut corners: Vec<(f64, f64)> = (0..cv.n)
            .filter(|&i| cv.tag[i] == Tag::Corner)
            .map(|i| (cv.c[i * 3 + 1].x, cv.c[i * 3 + 1].y))
            .collect();
        corners.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
        assert_eq!(
            corners,
            vec![(10.0, 10.0), (10.0, 50.0), (70.0, 10.0), (70.0, 50.0)]
        );
    }

    #[test]
    fn trace_min_area_drops_small_outlines() {
        let bm = bitmap(60, 60, |x, y| {
            ((10..40).contains(&x) && (10..40).contains(&y)) || (x == 50 && y == 50)
        });
        let two = PotraceParams {
            min_area: 2,
            ..PotraceParams::default()
        };
        let zero = PotraceParams {
            min_area: 0,
            ..PotraceParams::default()
        };
        assert_eq!(trace(&bm, &two).len(), 1);
        assert_eq!(trace(&bm, &zero).len(), 2);
    }

    /// Splits path data into commands and numbers.
    fn tokens(d: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for ch in d.chars() {
            match ch {
                'M' | 'c' | 'l' | 'z' => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    out.push(ch.to_string());
                }
                '-' | ' ' => {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    if ch == '-' {
                        cur.push(ch);
                    }
                }
                '.' if cur.contains('.') => {
                    out.push(std::mem::take(&mut cur));
                    cur.push(ch);
                }
                _ => cur.push(ch),
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
        out
    }

    #[test]
    fn path_data_relative_steps_close_without_drift() {
        let bm = bitmap(300, 300, |x, y| {
            let d = (x - 150).pow(2) + (y - 150).pow(2);
            (60 * 60..=120 * 120).contains(&d)
        });
        let d = path_data(&trace(&bm, &PotraceParams::default()));
        let toks = tokens(&d);
        assert!(!toks.is_empty());
        let num = |t: &str| t.parse::<f64>().expect("number");
        let (mut i, mut x, mut y, mut mx, mut my, mut closed) = (0, 0.0, 0.0, 0.0, 0.0, 0);
        while i < toks.len() {
            let cmd = toks[i].clone();
            i += 1;
            match cmd.as_str() {
                "M" => {
                    x = num(&toks[i]);
                    y = num(&toks[i + 1]);
                    i += 2;
                    (mx, my) = (x, y);
                }
                "l" => {
                    x += num(&toks[i]);
                    y += num(&toks[i + 1]);
                    i += 2;
                }
                "c" => {
                    x += num(&toks[i + 4]);
                    y += num(&toks[i + 5]);
                    i += 6;
                }
                "z" => {
                    assert!((x - mx).abs() < 0.002 && (y - my).abs() < 0.002);
                    closed += 1;
                }
                other => panic!("unexpected token {other}"),
            }
        }
        assert_eq!(closed, 2);
        assert_eq!(d.matches('M').count(), 2, "one absolute move per outline");
    }
}
