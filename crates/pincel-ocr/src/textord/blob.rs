//! Blobs: outlines grouped with the holes and islands nested inside them
//! (`src/textord/edgblob.cpp` `OL_BUCKETS`, `outlines_to_blobs`;
//! `src/ccstruct/stepblob.cpp` `C_BLOB`).

use super::outline::Outline;
use super::rect::TBox;

/// `BUCKETSIZE`.
const BUCKET_SIZE: i32 = 16;
/// `edges_children_per_grandchild`.
const CHILDREN_PER_GRANDCHILD: i32 = 10;
/// `edges_children_count_limit`.
const CHILDREN_COUNT_LIMIT: i32 = 45;
/// `edges_childarea`.
const CHILD_AREA: f64 = 0.5;
/// `edges_boxarea`.
const BOX_AREA: f64 = 0.875;
/// `kMaxPerimeterWidthRatio`.
const MAX_PERIMETER_WIDTH_RATIO: f64 = 8.0;

/// A blob of outlines (`C_BLOB`): top-level outlines with their children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CBlob {
    pub(crate) outlines: Vec<Outline>,
}

impl CBlob {
    /// `C_BLOB(C_OUTLINE*)` followed by `CheckInverseFlagAndDirection`.
    pub(crate) fn from_outline(mut outline: Outline) -> Self {
        if outline.turn_direction() < 0 {
            outline.reverse();
            reverse_all(&mut outline.children);
            outline.inverse = true;
        } else {
            outline.inverse = false;
        }
        Self {
            outlines: vec![outline],
        }
    }

    /// `C_BLOB(C_OUTLINE_LIST*)`: nests the outlines, then
    /// `CheckInverseFlagAndDirection` on each top-level one.
    pub(crate) fn from_outlines(list: Vec<Outline>) -> Self {
        let mut nested: Vec<Outline> = Vec::new();
        for o in list {
            position_outline(o, &mut nested);
        }
        let outlines = nested
            .into_iter()
            .flat_map(|o| CBlob::from_outline(o).outlines)
            .collect();
        Self { outlines }
    }

    /// `C_BLOB::FakeBlob`.
    pub(crate) fn fake(bbox: TBox) -> Self {
        Self {
            outlines: vec![Outline::fake(bbox)],
        }
    }

    /// `C_BLOB::bounding_box`.
    pub(crate) fn bounding_box(&self) -> TBox {
        self.outlines
            .iter()
            .fold(TBox::default(), |b, o| b.union(&o.bbox))
    }

    /// `C_BLOB::area`.
    pub(crate) fn area(&self) -> i32 {
        self.outlines.iter().map(Outline::area).sum()
    }

    /// `C_BLOB::perimeter`.
    pub(crate) fn perimeter(&self) -> i32 {
        self.outlines.iter().map(Outline::perimeter).sum()
    }

    /// `C_BLOB::count_transitions`.
    pub(crate) fn count_transitions(&self, threshold: i32) -> i32 {
        self.outlines
            .iter()
            .map(|o| o.count_transitions(threshold))
            .sum()
    }

    /// `C_BLOB::EstimateBaselinePosition`: the bottom of the blob, or the
    /// lowest flat run above it that is longer than the bottom (for
    /// letters whose lowest point is a descender tip or serif).
    pub(crate) fn estimate_baseline_position(&self) -> i32 {
        let bbox = self.bounding_box();
        let (left, width, bottom) = (bbox.left, bbox.width(), bbox.bottom);
        if self.outlines.is_empty()
            || f64::from(self.perimeter()) > f64::from(width) * MAX_PERIMETER_WIDTH_RATIO
        {
            return bottom;
        }
        let w = width.max(0) as usize;
        let mut y_mins = vec![bbox.top; w + 1];
        for o in &self.outlines {
            let mut pos = o.start;
            for s in 0..o.steps.len() {
                let i = (pos.0 - left) as usize;
                if pos.1 < y_mins[i] {
                    y_mins[i] = pos.1;
                }
                let st = o.step(s);
                pos = (pos.0 + st.0, pos.1 + st.1);
            }
        }
        let bottom_extent = y_mins
            .iter()
            .filter(|&&y| y == bottom || y == bottom + 1)
            .count();
        let mut best_min = bbox.top;
        let mut prev_run = 0usize;
        let mut prev_y = bbox.top;
        let mut prev_prev_y = bbox.top;
        let mut x = 0usize;
        while x < w {
            let y_at_x = y_mins[x];
            let mut run = 1;
            while x + run <= w && y_mins[x + run] == y_at_x {
                run += 1;
            }
            if y_at_x > bottom + 1 {
                let mut total_run = run;
                while x + total_run <= w
                    && (y_mins[x + total_run] == y_at_x || y_mins[x + total_run] == y_at_x + 1)
                {
                    total_run += 1;
                }
                if prev_prev_y > y_at_x + 1
                    || x + total_run > w
                    || y_mins[x + total_run] > y_at_x + 1
                {
                    if prev_run > 0 && prev_y == y_at_x + 1 {
                        total_run += prev_run;
                    }
                    if total_run > bottom_extent && y_at_x < best_min {
                        best_min = y_at_x;
                    }
                }
            }
            prev_run = run;
            prev_prev_y = prev_y;
            prev_y = y_at_x;
            x += prev_run;
        }
        if best_min == bbox.top {
            bottom
        } else {
            best_min
        }
    }
}

/// `reverse_outline_list`.
fn reverse_all(list: &mut [Outline]) {
    for o in list {
        o.reverse();
        o.inverse = true;
        reverse_all(&mut o.children);
    }
}

/// `position_outline`: puts `outline` where it belongs in the nesting
/// hierarchy of `dest`.
fn position_outline(mut outline: Outline, dest: &mut Vec<Outline>) {
    let mut i = 0;
    while i < dest.len() {
        if dest[i].is_inside(&outline) {
            let inner = std::mem::replace(&mut dest[i], Outline::fake(TBox::default()));
            outline.children.push(inner);
            let mut j = i + 1;
            while j < dest.len() {
                if dest[j].is_inside(&outline) {
                    outline.children.push(dest.remove(j));
                } else {
                    j += 1;
                }
            }
            dest[i] = outline;
            return;
        } else if outline.is_inside(&dest[i]) {
            position_outline(outline, &mut dest[i].children);
            return;
        }
        i += 1;
    }
    dest.push(outline);
}

/// `OL_BUCKETS`: outlines sorted into 16-pixel buckets by their
/// bottom-left corner.
struct Buckets {
    bxdim: i32,
    bydim: i32,
    buckets: Vec<Vec<Outline>>,
}

impl Buckets {
    fn index(&self, x: i32, y: i32) -> usize {
        ((y / BUCKET_SIZE) * self.bxdim + x / BUCKET_SIZE) as usize
    }

    /// The bucket index ranges an outline's box covers.
    fn range(&self, b: &TBox) -> (i32, i32, i32, i32) {
        let _ = self.bydim;
        (
            b.left / BUCKET_SIZE,
            b.right / BUCKET_SIZE,
            b.bottom / BUCKET_SIZE,
            b.top / BUCKET_SIZE,
        )
    }

    /// `OL_BUCKETS::count_children`.
    fn count_children(&self, outline: &Outline, max_count: i32) -> i32 {
        let (xmin, xmax, ymin, ymax) = self.range(&outline.bbox);
        let mut child_count = 0;
        let mut grandchild_count = 0;
        let mut parent_area = 0;
        let mut parent_box = true;
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                let bucket = &self.buckets[(y * self.bxdim + x) as usize];
                for child in bucket {
                    if std::ptr::eq(child, outline) || !child.is_inside(outline) {
                        continue;
                    }
                    child_count += 1;
                    if child_count <= max_count {
                        let max_grand = (max_count - child_count) / CHILDREN_PER_GRANDCHILD;
                        if max_grand > 0 {
                            grandchild_count +=
                                self.count_children(child, max_grand) * CHILDREN_PER_GRANDCHILD;
                        } else {
                            grandchild_count += self.count_children(child, 1);
                        }
                    }
                    if child_count + grandchild_count > max_count {
                        return child_count + grandchild_count;
                    }
                    if parent_area == 0 {
                        parent_area = outline.outer_area().abs();
                        let max_parent_area = (f64::from(outline.bbox.area()) * BOX_AREA) as f32;
                        if (parent_area as f32) < max_parent_area {
                            parent_box = false;
                        }
                    }
                    if parent_box {
                        let child_area = child.outer_area().abs();
                        if f64::from(child_area) < f64::from(child.bbox.area()) * CHILD_AREA {
                            return max_count + 1;
                        }
                    }
                }
            }
        }
        child_count + grandchild_count
    }

    /// `OL_BUCKETS::extract_children`.
    fn extract_children(&mut self, outline: &Outline, out: &mut Vec<Outline>) {
        let (xmin, xmax, ymin, ymax) = self.range(&outline.bbox);
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                let bucket = &mut self.buckets[(y * self.bxdim + x) as usize];
                let mut i = 0;
                while i < bucket.len() {
                    if bucket[i].is_inside(outline) {
                        out.push(bucket.remove(i));
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }
}

/// `C_BLOB::ConstructBlobsFromOutlines`: nests the outlines and makes a
/// blob of each top-level one, sorting them into good and bad blobs.
fn construct_blobs(
    good: bool,
    outlines: Vec<Outline>,
    good_out: &mut Vec<CBlob>,
    bad: &mut Vec<CBlob>,
) {
    let mut nested: Vec<Outline> = Vec::new();
    for o in outlines {
        position_outline(o, &mut nested);
    }
    let mut queue: std::collections::VecDeque<Outline> = nested.into();
    while let Some(mut outline) = queue.pop_front() {
        let mut blob_good = good;
        if !outline.is_legally_nested() {
            blob_good = false;
            let children = std::mem::take(&mut outline.children);
            for c in children.into_iter().rev() {
                queue.push_front(c);
            }
        }
        let blob = CBlob::from_outline(outline);
        if blob_good {
            good_out.push(blob);
        } else {
            bad.push(blob);
        }
    }
}

/// `extract_edges` / `outlines_to_blobs` for an image of `width × height`:
/// returns the good blobs and the rejected ones, in Tesseract's order.
pub(crate) fn outlines_to_blobs(
    outlines: Vec<Outline>,
    width: i32,
    height: i32,
) -> (Vec<CBlob>, Vec<CBlob>) {
    let bxdim = width / BUCKET_SIZE + 1;
    let bydim = height / BUCKET_SIZE + 1;
    let mut buckets = Buckets {
        bxdim,
        bydim,
        buckets: vec![Vec::new(); (bxdim * bydim) as usize],
    };
    for o in outlines {
        let i = buckets.index(o.bbox.left, o.bbox.bottom);
        buckets.buckets[i].push(o);
    }
    let mut good = Vec::new();
    let mut bad = Vec::new();
    let mut b = 0usize;
    loop {
        while b < buckets.buckets.len() && buckets.buckets[b].is_empty() {
            b += 1;
        }
        if b >= buckets.buckets.len() {
            break;
        }
        let list = &buckets.buckets[b];
        let len = list.len();
        let mut cur = 0usize;
        let mut parent;
        loop {
            parent = cur;
            loop {
                cur = (cur + 1) % len;
                if cur == 0 || list[parent].is_inside(&list[cur]) {
                    break;
                }
            }
            if cur == 0 {
                break;
            }
        }
        let parent_outline = buckets.buckets[b].remove(parent);
        let child_count = buckets.count_children(&parent_outline, CHILDREN_COUNT_LIMIT);
        let good_blob = child_count <= CHILDREN_COUNT_LIMIT;
        let mut out = Vec::new();
        if good_blob && child_count > 0 {
            buckets.extract_children(&parent_outline, &mut out);
        }
        out.insert(0, parent_outline);
        construct_blobs(good_blob, out, &mut good, &mut bad);
    }
    (good, bad)
}

#[cfg(test)]
mod tests {
    use super::super::outline::extract_outlines;
    use super::*;

    /// A `w × h` image with the given black rectangles `(x, y, w, h)`,
    /// `y` from the top.
    fn image(w: usize, h: usize, rects: &[(usize, usize, usize, usize)]) -> Vec<bool> {
        let mut black = vec![false; w * h];
        for &(x, y, rw, rh) in rects {
            for yy in y..y + rh {
                for xx in x..x + rw {
                    black[yy * w + xx] = true;
                }
            }
        }
        black
    }

    #[test]
    fn outlines_to_blobs_ring_nests_its_hole() {
        let mut black = image(8, 8, &[(1, 1, 6, 6)]);
        for y in 3..5 {
            for x in 3..5 {
                black[y * 8 + x] = false;
            }
        }
        let (good, bad) = outlines_to_blobs(extract_outlines(&black, 8, 8), 8, 8);
        assert!(bad.is_empty());
        assert_eq!(good.len(), 1);
        assert_eq!(good[0].outlines[0].children.len(), 1);
        assert_eq!(good[0].area(), 36 - 4);
        assert_eq!(good[0].bounding_box(), TBox::new(1, 1, 7, 7));
    }

    #[test]
    fn outlines_to_blobs_separate_squares_make_separate_blobs() {
        let black = image(12, 4, &[(1, 1, 3, 3), (8, 1, 3, 3)]);
        let (good, _) = outlines_to_blobs(extract_outlines(&black, 12, 4), 12, 4);
        assert_eq!(good.len(), 2);
    }
}
