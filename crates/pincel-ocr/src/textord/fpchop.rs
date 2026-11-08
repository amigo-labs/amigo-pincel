//! Cutting blobs at a vertical line (`src/textord/fpchop.cpp`
//! `split_to_blob` and the outline-fragment machinery behind it), which
//! `restore_underlined_blobs` uses to cut letters out of underlines.

use super::outline::Outline;
use super::rect::TBox;

/// `C_OUTLINE::kMaxOutlineLength`.
const MAX_OUTLINE_LENGTH: usize = 16000;
/// Chain code of a step down (`DIR128` 32) and up (96).
const STEP_DOWN: u8 = 1;
const STEP_UP: u8 = 3;

/// `C_OUTLINE_FRAG`: a piece of a cut outline between two points on the
/// cut line. A head carries the steps; its tail only marks the other end.
#[derive(Debug, Clone)]
struct Frag {
    start: (i32, i32),
    end: (i32, i32),
    steps: Option<Vec<u8>>,
    other_end: usize,
    ycoord: i32,
}

/// The fragment arena and the two lists of fragment indices.
#[derive(Debug, Default)]
struct Frags {
    all: Vec<Frag>,
}

impl Frags {
    /// `add_frag_to_list`: keeps `list` in ascending `ycoord` order.
    fn add_to_list(&self, frag: usize, list: &mut Vec<usize>) {
        let f = &self.all[frag];
        let other_y = self.all[f.other_end].ycoord;
        let pos = list.iter().position(|&i| {
            let d = &self.all[i];
            d.ycoord > f.ycoord || (d.ycoord == f.ycoord && other_y < f.ycoord)
        });
        match pos {
            Some(p) => list.insert(p, frag),
            None => list.push(frag),
        }
    }

    /// `save_chop_cfragment`.
    fn save(
        &mut self,
        head: (usize, (i32, i32)),
        tail: (usize, (i32, i32)),
        src: &Outline,
        list: &mut Vec<usize>,
    ) {
        let n = src.steps.len();
        let stepcount = (tail.0 + n - head.0) % n;
        let jump = (tail.1.1 - head.1.1).abs();
        if jump == stepcount as i32 {
            return;
        }
        let steps: Vec<u8> = (0..stepcount)
            .map(|i| src.steps[(head.0 + i) % n])
            .collect();
        let h = self.all.len();
        self.all.push(Frag {
            start: head.1,
            end: tail.1,
            steps: Some(steps),
            other_end: h + 1,
            ycoord: head.1.1,
        });
        self.all.push(Frag {
            start: head.1,
            end: tail.1,
            steps: None,
            other_end: h,
            ycoord: tail.1.1,
        });
        self.add_to_list(h, list);
        self.add_to_list(h + 1, list);
    }

    /// `join_segments`: appends `top`'s steps to `bottom`'s, closing the
    /// gap with vertical steps.
    fn join_segments(&mut self, bottom: usize, top: usize) {
        let (b_end, t_start) = (self.all[bottom].end, self.all[top].start);
        let mut fake = t_start.1 - b_end.1;
        let fake_step = if fake < 0 {
            fake = -fake;
            STEP_DOWN
        } else {
            STEP_UP
        };
        let top_steps = self.all[top].steps.clone().unwrap_or_default();
        let t_end = self.all[top].end;
        let b = &mut self.all[bottom];
        let steps = b.steps.get_or_insert_with(Vec::new);
        steps.extend(std::iter::repeat_n(fake_step, fake as usize));
        steps.extend(top_steps);
        b.end = t_end;
        let other = b.other_end;
        self.all[other].end = t_end;
    }

    /// `C_OUTLINE_FRAG::close`: joins the ends into an outline.
    fn close(&self, f: usize) -> Option<Outline> {
        let fr = &self.all[f];
        let mut fake = fr.start.1 - fr.end.1;
        let fake_step = if fake < 0 {
            fake = -fake;
            STEP_DOWN
        } else {
            STEP_UP
        };
        let mut steps = fr.steps.clone().unwrap_or_default();
        if steps.len() + fake as usize > MAX_OUTLINE_LENGTH {
            return None;
        }
        steps.extend(std::iter::repeat_n(fake_step, fake as usize));
        Some(outline_from_steps(fr.start, &steps))
    }

    /// `join_chopped_fragments`.
    fn join(&mut self, bottom: usize, top: usize) -> Option<Outline> {
        if self.all[bottom].other_end == top {
            return if self.all[bottom].steps.is_none() {
                self.close(top)
            } else {
                self.close(bottom)
            };
        }
        if self.all[bottom].steps.is_none() {
            let b_other = self.all[bottom].other_end;
            self.join_segments(b_other, top);
        } else {
            let t_other = self.all[top].other_end;
            self.join_segments(t_other, bottom);
        }
        let (b_other, t_other) = (self.all[bottom].other_end, self.all[top].other_end);
        self.all[t_other].other_end = b_other;
        self.all[b_other].other_end = t_other;
        None
    }
}

/// `C_OUTLINE(ICOORD, DIR128*, length)`: an outline from steps, with
/// there-and-back pairs cancelled.
fn outline_from_steps(start: (i32, i32), src: &[u8]) -> Outline {
    const COORDS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];
    let mut steps: Vec<u8> = Vec::with_capacity(src.len());
    let mut pos = start;
    let mut bbox = TBox::default();
    let last = src.last().copied().unwrap_or(0);
    let mut prev = last;
    for &d in src {
        bbox = bbox.union(&TBox::new(pos.0, pos.1, pos.0, pos.1));
        steps.push(d);
        let s = COORDS[usize::from(d)];
        pos = (pos.0 + s.0, pos.1 + s.1);
        if (d + 4 - prev) % 4 == 2 && steps.len() > 1 {
            steps.truncate(steps.len() - 2);
            prev = steps.last().copied().unwrap_or(last);
        } else {
            prev = d;
        }
    }
    let mut start = start;
    while steps.len() > 1 {
        let first = steps[0];
        let lastd = steps[steps.len() - 1];
        if (lastd + 4 - first) % 4 != 2 {
            break;
        }
        let s = COORDS[usize::from(first)];
        start = (start.0 + s.0, start.1 + s.1);
        steps.remove(0);
        steps.pop();
    }
    Outline {
        bbox,
        start,
        steps,
        children: Vec::new(),
        inverse: false,
    }
}

/// `fixed_chop_coutline`: cuts the outline at `x = chop`, adding the
/// pieces to the fragment lists; false when it does not cross the line.
fn chop_outline(
    src: &Outline,
    chop: i32,
    pitch_error: f32,
    frags: &mut Frags,
    left: &mut Vec<usize>,
    right: &mut Vec<usize>,
) -> bool {
    let n = src.steps.len();
    if n == 0 {
        return false;
    }
    let mut pos = src.start;
    let mut left_edge = pos.0;
    let mut tail_index = 0usize;
    let mut tail_pos = pos;
    for i in 0..n {
        if pos.0 < left_edge {
            left_edge = pos.0;
            tail_index = i;
            tail_pos = pos;
        }
        let s = src.step(i);
        pos = (pos.0 + s.0, pos.1 + s.1);
    }
    if left_edge as f32 >= chop as f32 - pitch_error {
        return false;
    }
    let advance = |index: &mut usize, p: &mut (i32, i32)| {
        let s = src.step(*index);
        *p = (p.0 + s.0, p.1 + s.1);
        *index = (*index + 1) % n;
    };
    let startindex = tail_index;
    let mut first_frag = true;
    let mut head_index = tail_index;
    let mut head_pos = tail_pos;
    let (mut first_index, mut first_pos) = (0usize, (0, 0));
    loop {
        loop {
            advance(&mut tail_index, &mut tail_pos);
            if tail_pos.0 == chop || tail_index == startindex {
                break;
            }
        }
        if tail_index == startindex {
            if first_frag {
                return false;
            }
            break;
        }
        if first_frag {
            first_index = tail_index;
            first_pos = tail_pos;
            first_frag = false;
        } else {
            frags.save((head_index, head_pos), (tail_index, tail_pos), src, left);
        }
        while src.step(tail_index).0 == 0 {
            advance(&mut tail_index, &mut tail_pos);
        }
        head_index = tail_index;
        head_pos = tail_pos;
        while src.step(tail_index).0 > 0 {
            loop {
                advance(&mut tail_index, &mut tail_pos);
                if tail_pos.0 == chop {
                    break;
                }
            }
            frags.save((head_index, head_pos), (tail_index, tail_pos), src, right);
            while src.step(tail_index).0 == 0 {
                advance(&mut tail_index, &mut tail_pos);
            }
            head_index = tail_index;
            head_pos = tail_pos;
        }
        if tail_index == startindex {
            break;
        }
    }
    frags.save((head_index, head_pos), (first_index, first_pos), src, left);
    true
}

/// `close_chopped_cfragments`: joins the fragments into outlines, each
/// taking the children inside it, and moves them to `dest`.
fn close_fragments(
    frags: &mut Frags,
    list: &mut Vec<usize>,
    children: &mut Vec<Outline>,
    pitch_error: f32,
    dest: &mut Vec<Outline>,
) {
    while !list.is_empty() {
        let bottom = list.remove(0);
        if list.is_empty() {
            break;
        }
        let mut top_i = 0usize;
        let top = list[0];
        if frags.all[bottom].steps.is_some() == frags.all[top].steps.is_some() {
            let next = list[1 % list.len()];
            if frags.all[next].ycoord == frags.all[top].ycoord {
                top_i = 1 % list.len();
            }
        }
        let top = list.remove(top_i);
        if frags.all[top].other_end != bottom {
            frags.join(bottom, top);
        } else if let Some(mut outline) = frags.join(bottom, top) {
            let mut i = 0;
            while i < children.len() {
                if children[i].is_inside(&outline) {
                    outline.children.push(children.remove(i));
                } else {
                    i += 1;
                }
            }
            if outline.bbox.width() as f32 > pitch_error {
                dest.push(outline);
            }
        }
    }
    dest.append(children);
}

/// `fixed_split_coutline`: sorts the outline (cut if it straddles the
/// line) into the left and right lists.
fn split_outline(
    mut src: Outline,
    chop: i32,
    pitch_error: f32,
    left: &mut Vec<Outline>,
    right: &mut Vec<Outline>,
) {
    let b = src.bbox;
    let centre_left = b.left + b.right <= chop * 2;
    if centre_left && (b.right as f32) < chop as f32 + pitch_error {
        left.push(src);
        return;
    }
    if !centre_left && b.left as f32 > chop as f32 - pitch_error {
        right.push(src);
        return;
    }
    let mut frags = Frags::default();
    let (mut lf, mut rf) = (Vec::new(), Vec::new());
    if !chop_outline(&src, chop, pitch_error, &mut frags, &mut lf, &mut rf) {
        if centre_left {
            left.push(src);
        } else {
            right.push(src);
        }
        return;
    }
    let (mut left_ch, mut right_ch) = (Vec::new(), Vec::new());
    for child in std::mem::take(&mut src.children) {
        let cb = child.bbox;
        if cb.right < chop {
            left_ch.push(child);
        } else if cb.left > chop {
            right_ch.push(child);
        } else if !chop_outline(&child, chop, 0.0, &mut frags, &mut lf, &mut rf) {
            if cb.left + cb.right <= chop * 2 {
                left_ch.push(child);
            } else {
                right_ch.push(child);
            }
        }
    }
    close_fragments(&mut frags, &mut lf, &mut left_ch, pitch_error, left);
    close_fragments(&mut frags, &mut rf, &mut right_ch, pitch_error, right);
}

/// `split_to_blob` / `fixed_chop_cblob`: cuts the outlines already on the
/// right and those of `blob` at `x = chop`, moving the pieces left of the
/// line to `left`.
pub(crate) fn split_to_blob(
    blob: Option<Vec<Outline>>,
    chop: i32,
    pitch_error: f32,
    left: &mut Vec<Outline>,
    right: &mut Vec<Outline>,
) {
    if right.is_empty() && blob.is_none() {
        return;
    }
    let old_right = std::mem::take(right);
    let mut new_right = Vec::new();
    for o in old_right {
        split_outline(o, chop, pitch_error, left, &mut new_right);
    }
    *right = new_right;
    if let Some(outlines) = blob {
        for o in outlines {
            split_outline(o, chop, pitch_error, left, right);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::outline::extract_outlines;
    use super::*;

    #[test]
    fn split_to_blob_cuts_bar_at_line() {
        // A 10 × 2 bar from x = 1 to 11.
        let mut black = vec![false; 12 * 4];
        for y in 1..3 {
            for x in 1..11 {
                black[y * 12 + x] = true;
            }
        }
        let outlines = extract_outlines(&black, 12, 4);
        let (mut left, mut right) = (Vec::new(), Vec::new());
        split_to_blob(Some(outlines), 6, 2.5, &mut left, &mut right);
        assert_eq!(left.len(), 1);
        assert_eq!(right.len(), 1);
        assert_eq!((left[0].bbox.left, left[0].bbox.right), (1, 6));
        assert_eq!((right[0].bbox.left, right[0].bbox.right), (6, 11));
    }
}
