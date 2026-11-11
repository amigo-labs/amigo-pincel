//! From ink components to text lines: component classes, the skew, rows
//! of components and their split into lines at gaps of two x-heights
//! (docs/specs/ocr-page.md, "Line").
//!
//! Geometry after the skew estimate lives in deskewed coordinates: `u`
//! along the text direction, `v` across it.

use super::ink::{Comp, Ink};

/// A comp no line owns; never masked out of a line image.
pub(crate) const UNASSIGNED: u32 = u32::MAX;
/// A comp that is not text (rules, frames); masked out of every line.
pub(crate) const NOT_TEXT: u32 = u32::MAX - 1;

/// Grid cell size of the comp index, in pixels.
const CELL: i32 = 64;
/// The largest skew searched, in degrees (docs/specs/ocr-page.md, Scope).
const MAX_SKEW_DEG: f64 = 5.0;
/// Lines lower than this hold no character the recognizer can read.
const MIN_LINE_HEIGHT: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A character, word or icon.
    Text,
    /// A dot or speck, at most 3 px either way.
    Minor,
    /// A thin horizontal or vertical line.
    Rule,
    /// A rectangle around other ink (button, input field, panel).
    Frame,
    /// Ink inside another component's box (a counter, a speck in a letter).
    Inner,
}

/// A box in deskewed coordinates; `u1` and `v1` are exclusive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DBox {
    pub(crate) u0: f64,
    pub(crate) u1: f64,
    pub(crate) v0: f64,
    pub(crate) v1: f64,
}

impl DBox {
    pub(crate) fn height(&self) -> f64 {
        self.v1 - self.v0
    }

    fn union(&self, o: &DBox) -> DBox {
        DBox {
            u0: self.u0.min(o.u0),
            u1: self.u1.max(o.u1),
            v0: self.v0.min(o.v0),
            v1: self.v1.max(o.v1),
        }
    }

    fn v_overlap(&self, o: &DBox) -> f64 {
        self.v1.min(o.v1) - self.v0.max(o.v0)
    }

    fn v_center(&self) -> f64 {
        (self.v0 + self.v1) / 2.0
    }
}

/// A text line.
#[derive(Debug, Clone)]
pub(crate) struct Line {
    /// The comps of the line.
    pub(crate) comps: Vec<u32>,
    /// The line's deskewed box.
    pub(crate) bbox: DBox,
    /// The line's estimated x-height.
    pub(crate) xh: f64,
}

/// A uniform grid over comp boxes.
pub(crate) struct Grid {
    cols: i32,
    rows: i32,
    cells: Vec<Vec<u32>>,
}

impl Grid {
    fn new(comps: &[Comp], width: i32, height: i32) -> Self {
        let cols = (width + CELL - 1) / CELL;
        let rows = (height + CELL - 1) / CELL;
        let mut cells = vec![Vec::new(); (cols * rows) as usize];
        for (i, c) in comps.iter().enumerate() {
            for gy in c.top / CELL..=c.bottom / CELL {
                for gx in c.left / CELL..=c.right / CELL {
                    cells[(gy * cols + gx) as usize].push(i as u32);
                }
            }
        }
        Self { cols, rows, cells }
    }

    /// Comps whose cells meet the rectangle (inclusive edges), each once
    /// per cell it shares; callers test the boxes themselves.
    pub(crate) fn near(&self, left: i32, top: i32, right: i32, bottom: i32) -> Vec<u32> {
        let gx0 = (left / CELL).clamp(0, self.cols - 1);
        let gx1 = (right / CELL).clamp(0, self.cols - 1);
        let gy0 = (top / CELL).clamp(0, self.rows - 1);
        let gy1 = (bottom / CELL).clamp(0, self.rows - 1);
        let mut out = Vec::new();
        for gy in gy0..=gy1 {
            for gx in gx0..=gx1 {
                out.extend_from_slice(&self.cells[(gy * self.cols + gx) as usize]);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// The page's lines and what the line images need.
pub(crate) struct Layout {
    /// The text direction: `(cos, sin)` of the skew angle.
    pub(crate) cos: f64,
    pub(crate) sin: f64,
    pub(crate) lines: Vec<Line>,
    /// Per comp: the index of the line that owns it, [`UNASSIGNED`] or
    /// [`NOT_TEXT`].
    pub(crate) owner: Vec<u32>,
    pub(crate) grid: Grid,
}

impl Layout {
    /// Image coordinates of a deskewed point.
    pub(crate) fn to_xy(&self, u: f64, v: f64) -> (f64, f64) {
        (u * self.cos - v * self.sin, u * self.sin + v * self.cos)
    }
}

/// Share of a comp's ink within `d` px of its box's edges.
fn perimeter_share(ink: &Ink, c: &Comp, d: i32) -> f64 {
    let mut near = 0u64;
    for r in ink.runs_of(c) {
        let (y, x0, x1) = (r.y as i32, r.x0 as i32, r.x1 as i32);
        if y - c.top < d || c.bottom - y < d {
            near += (x1 - x0 + 1) as u64;
            continue;
        }
        let left_band = (x1.min(c.left + d - 1) - x0 + 1).max(0);
        let right_band = (x1 - x0.max(c.right - d + 1) + 1).max(0);
        near += (left_band + right_band).min(x1 - x0 + 1) as u64;
    }
    near as f64 / f64::from(c.area.max(1))
}

fn inside(outer: &Comp, inner: &Comp) -> bool {
    outer.left < inner.left
        && outer.top < inner.top
        && inner.right < outer.right
        && inner.bottom < outer.bottom
}

fn classify(ink: &Ink, grid: &Grid) -> Vec<Kind> {
    let comps = &ink.comps;
    let mut kind: Vec<Kind> = comps
        .iter()
        .map(|c| {
            let (w, h) = (c.width(), c.height());
            if w <= 3 && h <= 3 {
                Kind::Minor
            } else if w >= 10 * h || h >= 10 * w {
                Kind::Rule
            } else {
                Kind::Text
            }
        })
        .collect();
    // Frames: hollow rectangles with ink inside.
    for (i, c) in comps.iter().enumerate() {
        let (w, h) = (c.width(), c.height());
        if kind[i] != Kind::Text || w < 10 || h < 10 {
            continue;
        }
        let d = 3 + w.min(h) / 10;
        if perimeter_share(ink, c, d) < 0.85 {
            continue;
        }
        let has_child = grid
            .near(c.left, c.top, c.right, c.bottom)
            .into_iter()
            .any(|j| j as usize != i && inside(c, &comps[j as usize]));
        if has_child {
            kind[i] = Kind::Frame;
        }
    }
    // Ink inside another text comp's box adds nothing to the layout.
    let mut inner = Vec::new();
    for (i, c) in comps.iter().enumerate() {
        if kind[i] != Kind::Text && kind[i] != Kind::Minor {
            continue;
        }
        let (cx, cy) = ((c.left + c.right) / 2, (c.top + c.bottom) / 2);
        let enclosed = grid.near(cx, cy, cx, cy).into_iter().any(|j| {
            j as usize != i && kind[j as usize] == Kind::Text && inside(&comps[j as usize], c)
        });
        if enclosed {
            inner.push(i);
        }
    }
    for i in inner {
        kind[i] = Kind::Inner;
    }
    kind
}

/// The skew angle that lines up the bottoms of the text comps best
/// (a projection profile), as `(cos, sin)`.
fn skew(comps: &[Comp], kind: &[Kind]) -> (f64, f64) {
    let points: Vec<(f64, f64)> = comps
        .iter()
        .zip(kind)
        .filter(|(c, k)| **k == Kind::Text && c.height() >= 5)
        .map(|(c, _)| (f64::from(c.left + c.right) / 2.0, f64::from(c.bottom) + 1.0))
        .collect();
    if points.len() < 3 {
        return (1.0, 0.0);
    }
    let score = |deg: f64| -> f64 {
        let (s, c) = deg.to_radians().sin_cos();
        let mut v: Vec<i64> = points
            .iter()
            .map(|&(x, y)| ((-x * s + y * c) / 2.0).floor() as i64)
            .collect();
        v.sort_unstable();
        let mut total = 0.0;
        let mut run = 0.0;
        for k in 0..v.len() {
            run += 1.0;
            if k + 1 == v.len() || v[k + 1] != v[k] {
                total += run * run;
                run = 0.0;
            }
        }
        total
    };
    let level = score(0.0);
    let mut best = (level, 0.0);
    let steps = (MAX_SKEW_DEG * 10.0) as i32;
    for k in -steps..=steps {
        let deg = f64::from(k) / 10.0;
        let s = score(deg);
        if s > best.0 {
            best = (s, deg);
        }
    }
    let coarse = best.1;
    for k in -10..=10 {
        let deg = coarse + f64::from(k) / 100.0;
        let s = score(deg);
        if s > best.0 {
            best = (s, deg);
        }
    }
    // A page that lines up as well level is level.
    if best.0 <= level * 1.02 {
        return (1.0, 0.0);
    }
    let (s, c) = best.1.to_radians().sin_cos();
    (c, s)
}

/// The deskewed box of a comp: its centre rotated, its size undone from
/// the rotated rectangle's bounding box.
fn deskew(c: &Comp, cos: f64, sin: f64) -> DBox {
    let (cx, cy) = (
        f64::from(c.left + c.right + 1) / 2.0,
        f64::from(c.top + c.bottom + 1) / 2.0,
    );
    let (u, v) = (cx * cos + cy * sin, -cx * sin + cy * cos);
    let (w, h) = (f64::from(c.width()), f64::from(c.height()));
    let s = sin.abs();
    let det = cos * cos - s * s;
    let tw = ((w * cos - h * s) / det).max(1.0).min(w);
    let th = ((h * cos - w * s) / det).max(1.0).min(h);
    DBox {
        u0: u - tw / 2.0,
        u1: u + tw / 2.0,
        v0: v - th / 2.0,
        v1: v + th / 2.0,
    }
}

/// Union-find over comp indices.
fn root(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        let p = parent[x as usize];
        parent[x as usize] = parent[p as usize];
        x = p;
    }
    x
}

/// Rows: text comps chained to their nearest right neighbour on the same
/// baseline.
fn rows(boxes: &[DBox], text: &[u32]) -> Vec<Vec<u32>> {
    if text.is_empty() {
        return Vec::new();
    }
    // Buckets by deskewed v-centre.
    const BIN: f64 = 16.0;
    let vmin = text
        .iter()
        .map(|&i| boxes[i as usize].v0)
        .fold(f64::INFINITY, f64::min);
    let vmax = text
        .iter()
        .map(|&i| boxes[i as usize].v1)
        .fold(f64::NEG_INFINITY, f64::max);
    let nbins = (((vmax - vmin) / BIN).ceil() as usize).max(1) + 1;
    let mut bins: Vec<Vec<u32>> = vec![Vec::new(); nbins];
    let bin_of = |v: f64| (((v - vmin) / BIN).floor().max(0.0) as usize).min(nbins - 1);
    for &i in text {
        bins[bin_of(boxes[i as usize].v_center())].push(i);
    }
    for b in &mut bins {
        b.sort_by(|&a, &c| boxes[a as usize].u0.total_cmp(&boxes[c as usize].u0));
    }
    // Each comp links to its nearest compatible neighbour on the right and
    // on the left, so a period between two words does not break the row.
    let mut best_left: Vec<Option<(f64, u32)>> = vec![None; boxes.len()];
    let mut links: Vec<(u32, u32)> = Vec::new();
    for &i in text {
        let a = boxes[i as usize];
        let ha = a.height();
        let mut best: Option<(f64, f64, u32)> = None;
        for bin in &bins[bin_of(a.v0 - ha)..=bin_of(a.v1 + ha)] {
            let start = bin.partition_point(|&j| boxes[j as usize].u0 < a.u0);
            for &j in &bin[start..] {
                if j == i {
                    continue;
                }
                let b = boxes[j as usize];
                let hb = b.height();
                let gap = b.u0 - a.u1;
                if gap > 9.0 * ha {
                    break;
                }
                let overlap = a.v_overlap(&b);
                if gap > 3.0 * ha.max(hb)
                    || overlap < 0.5 * ha.min(hb)
                    || ha.max(hb) > 3.0 * ha.min(hb)
                {
                    continue;
                }
                if best.is_none_or(|(g, o, _)| gap < g || (gap == g && overlap > o)) {
                    best = Some((gap, overlap, j));
                }
                if best_left[j as usize].is_none_or(|(g, _)| gap < g) {
                    best_left[j as usize] = Some((gap, i));
                }
            }
        }
        if let Some((_, _, j)) = best {
            links.push((i, j));
        }
    }
    for (j, left) in best_left.iter().enumerate() {
        if let Some((_, i)) = left {
            links.push((*i, j as u32));
        }
    }
    let mut parent: Vec<u32> = (0..boxes.len() as u32).collect();
    for (i, j) in links {
        let (ra, rb) = (root(&mut parent, i), root(&mut parent, j));
        if ra != rb {
            parent[ra.max(rb) as usize] = ra.min(rb);
        }
    }
    let mut groups: Vec<Vec<u32>> = Vec::new();
    let mut slot = vec![u32::MAX; boxes.len()];
    for &i in text {
        let r = root(&mut parent, i) as usize;
        if slot[r] == u32::MAX {
            slot[r] = groups.len() as u32;
            groups.push(Vec::new());
        }
        groups[slot[r] as usize].push(i);
    }
    groups
}

fn bbox_of(boxes: &[DBox], comps: &[u32]) -> DBox {
    comps
        .iter()
        .map(|&i| boxes[i as usize])
        .reduce(|a, b| a.union(&b))
        .unwrap_or(DBox {
            u0: 0.0,
            u1: 0.0,
            v0: 0.0,
            v1: 0.0,
        })
}

/// The x-height of a set of comps: the height of the densest horizontal
/// band of their ink (the band between baseline and x-line holds the most
/// ink of a text line), less the edge ink's one-pixel margins.
fn profile_x_height(ink: &Ink, comps: &[u32], sin: f64, cos: f64) -> f64 {
    let mut lo = f64::INFINITY;
    let mut samples: Vec<(f64, f64)> = Vec::new();
    for &i in comps {
        for r in ink.runs_of(&ink.comps[i as usize]) {
            let x = f64::from(r.x0 + r.x1 + 1) / 2.0;
            let v = -x * sin + (f64::from(r.y) + 0.5) * cos;
            lo = lo.min(v);
            samples.push((v, f64::from(r.x1 - r.x0 + 1)));
        }
    }
    if samples.is_empty() {
        return 1.0;
    }
    let mut profile: Vec<f64> = Vec::new();
    for (v, n) in samples {
        let k = (v - lo) as usize;
        if profile.len() <= k {
            profile.resize(k + 1, 0.0);
        }
        profile[k] += n;
    }
    let (peak_at, peak) = profile
        .iter()
        .enumerate()
        .fold((0, 0.0), |b, (k, &n)| if n > b.1 { (k, n) } else { b });
    let limit = 0.4 * peak;
    let mut a = peak_at;
    while a > 0 && profile[a - 1] >= limit {
        a -= 1;
    }
    let mut b = peak_at;
    while b + 1 < profile.len() && profile[b + 1] >= limit {
        b += 1;
    }
    ((b - a + 1) as f64 - 2.0).max(1.0)
}

/// The x-height of a set of comps: the ink profile's estimate, kept
/// between half and three quarters of the ink height (an x-height letter
/// alone, and capitals with descenders). The clamp catches the profile's
/// failures: a hollow outline of a very large letter, or one word whose
/// densest band is a single stroke.
fn x_height(ink: &Ink, boxes: &[DBox], comps: &[u32], sin: f64, cos: f64) -> f64 {
    let h = (bbox_of(boxes, comps).height() - 2.0).max(1.0);
    profile_x_height(ink, comps, sin, cos).clamp(0.5 * h, 0.75 * h)
}

/// Gaps along `u` between consecutive comps (sorted by `u0`): the index
/// after which each gap starts and its width.
fn u_gaps(boxes: &[DBox], comps: &[u32]) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    let mut reach = f64::NEG_INFINITY;
    for (k, &i) in comps.iter().enumerate() {
        let b = boxes[i as usize];
        if k > 0 && b.u0 > reach {
            out.push((k, b.u0 - reach));
        }
        reach = reach.max(b.u1);
    }
    out
}

/// Finds the text lines of a page.
pub(crate) fn find(ink: &Ink, width: usize, height: usize) -> Layout {
    let comps = &ink.comps;
    let grid = Grid::new(comps, width as i32, height as i32);
    let kind = classify(ink, &grid);
    let (cos, sin) = skew(comps, &kind);
    let boxes: Vec<DBox> = comps.iter().map(|c| deskew(c, cos, sin)).collect();
    let mut owner: Vec<u32> = kind
        .iter()
        .map(|k| match k {
            Kind::Rule | Kind::Frame => NOT_TEXT,
            _ => UNASSIGNED,
        })
        .collect();
    let text: Vec<u32> = (0..comps.len() as u32)
        .filter(|&i| kind[i as usize] == Kind::Text)
        .collect();
    // Rows, and the comps too small to be rows of their own (dots, accents,
    // punctuation) attached to the row they belong to.
    let mut groups = rows(&boxes, &text);
    let minor: Vec<u32> = (0..comps.len() as u32)
        .filter(|&i| kind[i as usize] == Kind::Minor)
        .collect();
    let mut row_boxes: Vec<DBox> = groups.iter().map(|g| bbox_of(&boxes, g)).collect();
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|&a, &b| row_boxes[a].height().total_cmp(&row_boxes[b].height()));
    let mut alive = vec![true; groups.len()];
    // Candidate hosts sorted by v for a windowed search.
    let mut by_v: Vec<usize> = (0..groups.len()).collect();
    by_v.sort_by(|&a, &b| row_boxes[a].v0.total_cmp(&row_boxes[b].v0));
    let tallest = row_boxes.iter().map(DBox::height).fold(0.0, f64::max);
    let host = |small: &DBox, row_boxes: &[DBox], alive: &[bool], skip: usize| -> Option<usize> {
        let hs = small.height();
        let c = small.v_center();
        let start = by_v.partition_point(|&k| row_boxes[k].v0 < small.v0 - 2.0 * tallest);
        let mut best: Option<(f64, usize)> = None;
        for &k in &by_v[start..] {
            let s = row_boxes[k];
            if s.v0 > small.v1 + tallest {
                break;
            }
            if k == skip || !alive[k] {
                continue;
            }
            let h = s.height();
            if hs > 0.5 * h || small.u1 - small.u0 > h {
                continue;
            }
            if c < s.v0 - 0.5 * h || c > s.v1 + 0.3 * h {
                continue;
            }
            if small.u1 < s.u0 - 0.5 * h || small.u0 > s.u1 + 0.5 * h {
                continue;
            }
            let d = (c - s.v_center()).abs();
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, k));
            }
        }
        best.map(|(_, k)| k)
    };
    for &r in &order {
        if groups[r].len() > 3 {
            continue;
        }
        if let Some(k) = host(&row_boxes[r], &row_boxes, &alive, r) {
            let moved = std::mem::take(&mut groups[r]);
            groups[k].extend(moved);
            row_boxes[k] = row_boxes[k].union(&row_boxes[r]);
            alive[r] = false;
        }
    }
    let mut attached: Vec<Vec<u32>> = vec![Vec::new(); groups.len()];
    for &m in &minor {
        if let Some(k) = host(&boxes[m as usize], &row_boxes, &alive, usize::MAX) {
            attached[k].push(m);
        }
    }
    // Split rows into lines at gaps of at least two x-heights.
    let mut lines: Vec<Line> = Vec::new();
    for (r, mut row) in groups.into_iter().enumerate() {
        if !alive[r] || row.is_empty() {
            continue;
        }
        let row_first = lines.len();
        row.sort_by(|&a, &b| boxes[a as usize].u0.total_cmp(&boxes[b as usize].u0));
        let row_xh = x_height(ink, &boxes, &row, sin, cos);
        let candidates: Vec<(usize, f64)> = u_gaps(&boxes, &row)
            .into_iter()
            .filter(|&(_, g)| g >= row_xh)
            .collect();
        let mut cuts = vec![0];
        cuts.extend(candidates.iter().map(|&(k, _)| k));
        cuts.push(row.len());
        let seg_xh: Vec<f64> = cuts
            .windows(2)
            .map(|w| x_height(ink, &boxes, &row[w[0]..w[1]], sin, cos))
            .collect();
        let mut start = 0;
        for (n, &(k, gap)) in candidates.iter().enumerate() {
            let xh = (seg_xh[n] + seg_xh[n + 1]) / 2.0;
            if gap >= 2.0 * xh {
                lines.push(new_line(ink, &boxes, row[start..k].to_vec(), sin, cos));
                start = k;
            }
        }
        lines.push(new_line(ink, &boxes, row[start..].to_vec(), sin, cos));
        // Attached minor comps go to the row's line nearest along u.
        for &m in &attached[r] {
            let c = (boxes[m as usize].u0 + boxes[m as usize].u1) / 2.0;
            let distance = |l: &Line| (l.bbox.u0 - c).max(c - l.bbox.u1).max(0.0);
            if let Some(line) = lines[row_first..]
                .iter_mut()
                .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            {
                line.comps.push(m);
                line.bbox = line.bbox.union(&boxes[m as usize]);
            }
        }
    }
    // Lines too small to hold a character are specks.
    lines.retain(|l| l.bbox.height() >= MIN_LINE_HEIGHT);
    for (k, line) in lines.iter().enumerate() {
        for &c in &line.comps {
            owner[c as usize] = k as u32;
        }
    }
    Layout {
        cos,
        sin,
        lines,
        owner,
        grid,
    }
}

fn new_line(ink: &Ink, boxes: &[DBox], comps: Vec<u32>, sin: f64, cos: f64) -> Line {
    let bbox = bbox_of(boxes, &comps);
    let xh = x_height(ink, boxes, &comps, sin, cos);
    Line { comps, bbox, xh }
}
