//! Outlines of the binary image: Tesseract's crack-following edge
//! extractor (`src/textord/scanedg.cpp`, `edgloop.cpp`) and the chain-coded
//! `C_OUTLINE` it produces (`src/ccstruct/coutln.cpp`).

use super::rect::TBox;

/// `C_OUTLINE::step_coords`, indexed by chain code.
const STEP_COORDS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];
/// `MINEDGELENGTH`: shorter loops are dropped.
const MIN_EDGE_LENGTH: i32 = 8;
/// `C_OUTLINE::kMaxOutlineLength`.
const MAX_OUTLINE_LENGTH: i32 = 16000;
/// `INTERSECTING`: a point on the outline has no winding number.
const INTERSECTING: i32 = i16::MAX as i32;

/// A closed chain-coded outline (`C_OUTLINE`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Outline {
    pub(crate) bbox: TBox,
    pub(crate) start: (i32, i32),
    /// Chain codes 0..4 (`step_coords`).
    pub(crate) steps: Vec<u8>,
    /// Outlines nested directly inside this one.
    pub(crate) children: Vec<Outline>,
    /// `COUT_INVERSE`: the outline was reversed to run anticlockwise
    /// around white-on-black.
    pub(crate) inverse: bool,
}

impl Outline {
    /// `C_OUTLINE::FakeOutline`: a stepless outline that only has a box.
    pub(crate) fn fake(bbox: TBox) -> Self {
        Self {
            bbox,
            start: (bbox.left, bbox.top),
            steps: Vec::new(),
            children: Vec::new(),
            inverse: false,
        }
    }

    /// `C_OUTLINE::step`.
    pub(crate) fn step(&self, i: usize) -> (i32, i32) {
        STEP_COORDS[usize::from(self.steps[i])]
    }

    /// `C_OUTLINE::pathlength`.
    pub(crate) fn path_length(&self) -> i32 {
        self.steps.len() as i32
    }

    /// The signed area enclosed by the path alone (`C_OUTLINE::outer_area`).
    pub(crate) fn outer_area(&self) -> i32 {
        if self.steps.is_empty() {
            return self.bbox.area();
        }
        let mut pos = self.start;
        let mut total = 0;
        for i in 0..self.steps.len() {
            let s = self.step(i);
            if s.0 < 0 {
                total += pos.1;
            } else if s.0 > 0 {
                total -= pos.1;
            }
            pos = (pos.0 + s.0, pos.1 + s.1);
        }
        total
    }

    /// `C_OUTLINE::area`: the path's area plus that of its children.
    pub(crate) fn area(&self) -> i32 {
        let mut pos = self.start;
        let mut total = 0;
        for i in 0..self.steps.len() {
            let s = self.step(i);
            if s.0 < 0 {
                total += pos.1;
            } else if s.0 > 0 {
                total -= pos.1;
            }
            pos = (pos.0 + s.0, pos.1 + s.1);
        }
        total + self.children.iter().map(Outline::area).sum::<i32>()
    }

    /// `C_OUTLINE::perimeter`: the path length plus the children's.
    pub(crate) fn perimeter(&self) -> i32 {
        self.path_length() + self.children.iter().map(Outline::path_length).sum::<i32>()
    }

    /// `C_OUTLINE::winding_number`.
    fn winding_number(&self, point: (i32, i32)) -> i32 {
        let mut vec = (self.start.0 - point.0, self.start.1 - point.1);
        let mut count = 0;
        for i in 0..self.steps.len() {
            let s = self.step(i);
            if vec.1 <= 0 && vec.1 + s.1 > 0 {
                let cross = vec.0 * s.1 - vec.1 * s.0;
                if cross > 0 {
                    count += 1;
                } else if cross == 0 {
                    return INTERSECTING;
                }
            } else if vec.1 > 0 && vec.1 + s.1 <= 0 {
                let cross = vec.0 * s.1 - vec.1 * s.0;
                if cross < 0 {
                    count -= 1;
                } else if cross == 0 {
                    return INTERSECTING;
                }
            }
            vec = (vec.0 + s.0, vec.1 + s.1);
        }
        count
    }

    /// `C_OUTLINE::operator<`: whether this outline lies inside `other`.
    pub(crate) fn is_inside(&self, other: &Outline) -> bool {
        if !self.bbox.overlap(&other.bbox) {
            return false;
        }
        if self.steps.is_empty() {
            return other.bbox.contains(&self.bbox);
        }
        let mut pos = self.start;
        let mut count = INTERSECTING;
        for i in 0..self.steps.len() {
            count = other.winding_number(pos);
            if count != INTERSECTING {
                break;
            }
            let s = self.step(i);
            pos = (pos.0 + s.0, pos.1 + s.1);
        }
        if count == INTERSECTING {
            let mut pos = other.start;
            for i in 0..other.steps.len() {
                count = self.winding_number(pos);
                if count != INTERSECTING {
                    break;
                }
                let s = other.step(i);
                pos = (pos.0 + s.0, pos.1 + s.1);
            }
            return count == INTERSECTING || count == 0;
        }
        count != 0
    }

    /// `C_OUTLINE::turn_direction`: +4 or −4 quarter turns (in chain
    /// units; Tesseract counts 128ths of a turn).
    pub(crate) fn turn_direction(&self) -> i32 {
        let Some(&last) = self.steps.last() else {
            return 128;
        };
        let mut prev = i32::from(last);
        let mut count = 0;
        for &d in &self.steps {
            let d = i32::from(d);
            let mut diff = d - prev;
            if diff > 2 {
                diff -= 4;
            } else if diff < -2 {
                diff += 4;
            }
            count += diff;
            prev = d;
        }
        count
    }

    /// `C_OUTLINE::reverse`.
    pub(crate) fn reverse(&mut self) {
        let n = self.steps.len();
        let half = n.div_ceil(2);
        for i in 0..half {
            let far = n - i - 1;
            let d = self.steps[i];
            self.steps[i] = (self.steps[far] + 2) % 4;
            self.steps[far] = (d + 2) % 4;
        }
    }

    /// `C_OUTLINE::IsLegallyNested`.
    pub(crate) fn is_legally_nested(&self) -> bool {
        if self.steps.is_empty() {
            return true;
        }
        let parent = i64::from(self.outer_area());
        self.children
            .iter()
            .all(|c| i64::from(c.outer_area()) * parent <= 0 && c.is_legally_nested())
    }

    /// `C_OUTLINE::count_transitions`.
    pub(crate) fn count_transitions(&self, threshold: i32) -> i32 {
        let mut pos = self.start;
        let mut total = 0;
        let (mut max_x, mut min_x, mut max_y, mut min_y) = (pos.0, pos.0, pos.1, pos.1);
        let (mut look_max_x, mut look_min_x, mut look_max_y, mut look_min_y) =
            (true, true, true, true);
        let (mut first_max_x, mut first_max_y) = (false, false);
        let (mut initial_x, mut initial_y) = (pos.0, pos.1);
        for i in 0..self.steps.len() {
            let s = self.step(i);
            pos = (pos.0 + s.0, pos.1 + s.1);
            if s.0 < 0 {
                if look_max_x && pos.0 < min_x {
                    min_x = pos.0;
                }
                if look_min_x && max_x - pos.0 > threshold {
                    if look_max_x {
                        initial_x = max_x;
                        first_max_x = false;
                    }
                    total += 1;
                    look_max_x = true;
                    look_min_x = false;
                    min_x = pos.0;
                }
            } else if s.0 > 0 {
                if look_min_x && pos.0 > max_x {
                    max_x = pos.0;
                }
                if look_max_x && pos.0 - min_x > threshold {
                    if look_min_x {
                        initial_x = min_x;
                        first_max_x = true;
                    }
                    total += 1;
                    look_max_x = false;
                    look_min_x = true;
                    max_x = pos.0;
                }
            } else if s.1 < 0 {
                if look_max_y && pos.1 < min_y {
                    min_y = pos.1;
                }
                if look_min_y && max_y - pos.1 > threshold {
                    if look_max_y {
                        initial_y = max_y;
                        first_max_y = false;
                    }
                    total += 1;
                    look_max_y = true;
                    look_min_y = false;
                    min_y = pos.1;
                }
            } else {
                if look_min_y && pos.1 > max_y {
                    max_y = pos.1;
                }
                if look_max_y && pos.1 - min_y > threshold {
                    if look_min_y {
                        initial_y = min_y;
                        first_max_y = true;
                    }
                    total += 1;
                    look_max_y = false;
                    look_min_y = true;
                    max_y = pos.1;
                }
            }
        }
        if first_max_x && look_min_x {
            total += if max_x - initial_x > threshold { 1 } else { -1 };
        } else if !first_max_x && look_max_x {
            total += if initial_x - min_x > threshold { 1 } else { -1 };
        }
        if first_max_y && look_min_y {
            total += if max_y - initial_y > threshold { 1 } else { -1 };
        } else if !first_max_y && look_max_y {
            total += if initial_y - min_y > threshold { 1 } else { -1 };
        }
        total
    }
}

// --- Crack following ----------------------------------------------------------

/// `CRACKEDGE`: one unit step of an outline under construction.
#[derive(Debug, Clone, Copy)]
struct Crack {
    x: i32,
    y: i32,
    step_x: i32,
    step_y: i32,
    dir: u8,
    prev: usize,
    next: usize,
}

/// `WHITE_PIX`; black is 0.
const WHITE: u8 = 1;

/// The state of `block_edges`: the crack arena, its free slots and the
/// finished outlines.
struct EdgeScan {
    cracks: Vec<Crack>,
    /// Slots of completed loops, reused like Tesseract's `free_cracks`, so
    /// the arena holds only the open loops of the scan.
    free: Vec<usize>,
    outlines: Vec<Outline>,
}

impl EdgeScan {
    fn alloc(&mut self, c: Crack) -> usize {
        if let Some(slot) = self.free.pop() {
            self.cracks[slot] = c;
            return slot;
        }
        self.cracks.push(c);
        self.cracks.len() - 1
    }

    /// Returns the cracks of a closed loop to the free list.
    fn release_loop(&mut self, start: usize) {
        let mut e = start;
        loop {
            self.free.push(e);
            e = self.cracks[e].next;
            if e == start {
                break;
            }
        }
    }

    /// Links a new crack to `join` like `h_edge` / `v_edge` do.
    fn link(&mut self, new: usize, join: Option<usize>) {
        let Some(join) = join else {
            self.cracks[new].prev = new;
            self.cracks[new].next = new;
            return;
        };
        let n = self.cracks[new];
        let j = self.cracks[join];
        if n.x + n.step_x == j.x && n.y + n.step_y == j.y {
            let p = j.prev;
            self.cracks[new].prev = p;
            self.cracks[p].next = new;
            self.cracks[new].next = join;
            self.cracks[join].prev = new;
        } else {
            let nx = j.next;
            self.cracks[new].next = nx;
            self.cracks[nx].prev = new;
            self.cracks[new].prev = join;
            self.cracks[join].next = new;
        }
    }

    /// `h_edge`.
    fn h_edge(&mut self, sign: i32, join: Option<usize>, x: i32, y: i32) -> usize {
        let (cx, step_x, dir) = if sign > 0 { (x + 1, -1, 0) } else { (x, 1, 2) };
        let new = self.alloc(Crack {
            x: cx,
            y: y + 1,
            step_x,
            step_y: 0,
            dir,
            prev: 0,
            next: 0,
        });
        self.link(new, join);
        new
    }

    /// `v_edge`.
    fn v_edge(&mut self, sign: i32, join: Option<usize>, x: i32, y: i32) -> usize {
        let (cy, step_y, dir) = if sign > 0 { (y, 1, 3) } else { (y + 1, -1, 1) };
        let new = self.alloc(Crack {
            x,
            y: cy,
            step_x: 0,
            step_y,
            dir,
            prev: 0,
            next: 0,
        });
        self.link(new, join);
        new
    }

    /// `join_edges`: joins two open ends, completing the loop when they
    /// belong to the same one.
    fn join_edges(&mut self, a: Option<usize>, b: Option<usize>) {
        let (Some(mut e1), Some(mut e2)) = (a, b) else {
            return;
        };
        let c1 = self.cracks[e1];
        let c2 = self.cracks[e2];
        if c1.x + c1.step_x != c2.x || c1.y + c1.step_y != c2.y {
            std::mem::swap(&mut e1, &mut e2);
        }
        if self.cracks[e1].next == e2 {
            self.complete_edge(e1);
        } else {
            let p2 = self.cracks[e2].prev;
            let n1 = self.cracks[e1].next;
            self.cracks[p2].next = n1;
            self.cracks[n1].prev = p2;
            self.cracks[e1].next = e2;
            self.cracks[e2].prev = e1;
        }
    }

    /// `complete_edge`: turns a closed loop into an outline when
    /// `check_path_legal` accepts it.
    fn complete_edge(&mut self, start: usize) {
        if self.path_legal(start) {
            self.push_outline(start);
        }
        self.release_loop(start);
    }

    /// `loop_bounding_box` and the `C_OUTLINE` constructor for a legal loop.
    fn push_outline(&mut self, start: usize) {
        // loop_bounding_box: the box and the top-left start.
        let s = self.cracks[start];
        let (mut left, mut bottom, mut right, mut top) = (s.x, s.y, s.x, s.y);
        let mut real_start = start;
        let mut leftmost = s.x;
        let mut length = 0usize;
        let mut e = start;
        loop {
            e = self.cracks[e].next;
            let c = self.cracks[e];
            if c.x < left {
                left = c.x;
            } else if c.x > right {
                right = c.x;
            }
            if c.y < bottom {
                bottom = c.y;
            } else if c.y > top {
                real_start = e;
                leftmost = c.x;
                top = c.y;
            } else if c.y == top && c.x < leftmost {
                leftmost = c.x;
                real_start = e;
            }
            length += 1;
            if e == start {
                break;
            }
        }
        let mut steps = Vec::with_capacity(length);
        let mut e = real_start;
        for _ in 0..length {
            steps.push(self.cracks[e].dir);
            e = self.cracks[e].next;
        }
        let rs = self.cracks[real_start];
        self.outlines.push(Outline {
            bbox: TBox::from_corners((left, bottom), (right, top)),
            start: (rs.x, rs.y),
            steps,
            children: Vec::new(),
            inverse: false,
        });
    }

    /// `check_path_legal`: long enough, closed, and turning once.
    fn path_legal(&self, start: usize) -> bool {
        let mut length = 0;
        let mut chainsum = 0;
        let mut e = start;
        let mut last = i32::from(self.cracks[self.cracks[e].prev].dir);
        loop {
            length += 1;
            let d = i32::from(self.cracks[e].dir);
            if d != last {
                let mut diff = d - last;
                if diff > 2 {
                    diff -= 4;
                } else if diff < -2 {
                    diff += 4;
                }
                chainsum += diff;
                last = d;
            }
            e = self.cracks[e].next;
            if e == start || length >= MAX_OUTLINE_LENGTH {
                break;
            }
        }
        (chainsum == 4 || chainsum == -4) && e == start && length >= MIN_EDGE_LENGTH
    }

    /// `line_edges`: scans one image row (`bw`, 0 = black) at `y` and
    /// updates the edges in progress in `prevline`.
    fn line_edges(&mut self, y: i32, bw: &[u8], prevline: &mut [Option<usize>]) {
        let mut upper = WHITE;
        let mut prev_colour = WHITE;
        let mut current: Option<usize> = None;
        for (x, &colour) in bw.iter().enumerate() {
            let xi = x as i32;
            if let Some(pl) = prevline[x] {
                upper = 1 - upper;
                if colour == prev_colour {
                    if colour == upper {
                        self.join_edges(current, Some(pl));
                        current = None;
                    } else {
                        current = Some(self.h_edge(
                            i32::from(upper) - i32::from(colour),
                            Some(pl),
                            xi,
                            y,
                        ));
                    }
                    prevline[x] = None;
                } else {
                    let sign_v = i32::from(colour) - i32::from(prev_colour);
                    if colour == upper {
                        prevline[x] = Some(self.v_edge(sign_v, Some(pl), xi, y));
                    } else if colour == WHITE {
                        self.join_edges(current, Some(pl));
                        let h = self.h_edge(i32::from(upper) - i32::from(colour), None, xi, y);
                        current = Some(h);
                        prevline[x] = Some(self.v_edge(sign_v, current, xi, y));
                    } else {
                        let h = self.h_edge(i32::from(upper) - i32::from(colour), Some(pl), xi, y);
                        prevline[x] = Some(self.v_edge(sign_v, current, xi, y));
                        current = Some(h);
                    }
                    prev_colour = colour;
                }
            } else {
                if colour != prev_colour {
                    let v = self.v_edge(i32::from(colour) - i32::from(prev_colour), current, xi, y);
                    prevline[x] = Some(v);
                    current = Some(v);
                    prev_colour = colour;
                }
                if colour != upper {
                    current =
                        Some(self.h_edge(i32::from(upper) - i32::from(colour), current, xi, y));
                } else {
                    current = None;
                }
            }
        }
        let xe = bw.len();
        let flip = i32::from(1 - prev_colour) - i32::from(prev_colour);
        if current.is_some() {
            if let Some(pl) = prevline[xe] {
                self.join_edges(current, Some(pl));
                prevline[xe] = None;
            } else {
                prevline[xe] = Some(self.v_edge(flip, current, xe as i32, y));
            }
        } else if let Some(pl) = prevline[xe] {
            prevline[xe] = Some(self.v_edge(flip, Some(pl), xe as i32, y));
        }
    }
}

/// `block_edges` over the whole image: every legal outline, in the order
/// the scan closes them. `black` is row-major from the top.
pub(crate) fn extract_outlines(black: &[bool], width: usize, height: usize) -> Vec<Outline> {
    let mut scan = EdgeScan {
        cracks: Vec::new(),
        free: Vec::new(),
        outlines: Vec::new(),
    };
    let mut prevline: Vec<Option<usize>> = vec![None; width + 1];
    let mut bw = vec![WHITE; width];
    for y in (-1..height as i32).rev() {
        if y >= 0 {
            let row = height - 1 - y as usize;
            for (x, p) in bw.iter_mut().enumerate() {
                *p = if black[row * width + x] { 0 } else { WHITE };
            }
        } else {
            bw.fill(WHITE);
        }
        scan.line_edges(y, &bw, &mut prevline);
    }
    scan.outlines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_outlines_square_with_hole_has_two_loops() {
        // 4×4 ring with a 2×2 hole.
        let mut black = vec![true; 16];
        for (x, y) in [(1, 1), (2, 1), (1, 2), (2, 2)] {
            black[y * 4 + x] = false;
        }
        let outlines = extract_outlines(&black, 4, 4);
        assert_eq!(outlines.len(), 2);
        let mut areas: Vec<i32> = outlines.iter().map(Outline::outer_area).collect();
        areas.sort_unstable();
        assert_eq!(areas[0].abs(), 4);
        assert_eq!(areas[1].abs(), 16);
    }

    #[test]
    fn extract_outlines_drops_single_pixel() {
        let outlines = extract_outlines(&[true], 1, 1);
        assert!(outlines.is_empty());
    }
}
