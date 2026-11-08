//! From textord words to recognized words (`src/ccstruct/pageres.cpp`
//! `ROW_RES`, `PAGE_RES_IT::ReplaceCurrentWord`; `src/ccmain/linerec.cpp`
//! `LSTMRecognizeWord`).

use super::blob::CBlob;
use super::rect::TBox;
use super::tospace::{Row, Werd};

/// `kMaxWordSizeRatio`.
const MAX_WORD_SIZE_RATIO: f64 = 1.25;
/// `kMaxLineSizeRatio`.
const MAX_LINE_SIZE_RATIO: f64 = 1.25;
/// `kMaxWordGapRatio`.
const MAX_WORD_GAP_RATIO: f64 = 2.0;
/// `kImagePadding`.
const IMAGE_PADDING: i32 = 4;

/// A word the recognizer runs on: one textord word, or a combination of
/// neighbouring similar words (`WERD_RES` with `combination`).
#[derive(Debug, Clone)]
pub(crate) struct RecogWord {
    /// The word (for a combination, a copy with the blobs of all parts).
    pub(crate) word: Werd,
    /// The boxes of the textord words a combination is made of
    /// (`part_of_combo` entries); empty for a lone word.
    pub(crate) parts: Vec<TBox>,
}

/// `ROW_RES::ROW_RES` with `merge_similar_words`: the words the
/// recognizer sees, in row order.
pub(crate) fn recog_words(row: &Row) -> Vec<RecogWord> {
    let line_height = f64::from(row.xheight + row.ascrise - row.descdrop);
    let n = row.words.len();
    let mut out: Vec<RecogWord> = Vec::new();
    let mut combo: Option<usize> = None;
    let mut add_next = false;
    let mut union_box = TBox::default();
    for i in 0..n {
        let word = &row.words[i];
        if add_next {
            if let Some(c) = combo {
                copy_on(&mut out[c].word, word);
                out[c].parts.push(word.bounding_box());
            }
        } else {
            union_box = word.bounding_box();
            add_next = f64::from(union_box.height()) <= line_height * MAX_WORD_SIZE_RATIO;
        }
        // The word list is circular: the last word's next is the first.
        let next = &row.words[(i + 1) % n];
        if add_next {
            let next_box = next.bounding_box();
            let prev_right = union_box.right;
            union_box = union_box.union(&next_box);
            if f64::from(next_box.height()) > line_height * MAX_WORD_SIZE_RATIO
                || f64::from(union_box.height()) > line_height * MAX_LINE_SIZE_RATIO
                || f64::from(next_box.left)
                    > f64::from(prev_right) + line_height * MAX_WORD_GAP_RATIO
            {
                add_next = false;
            }
        }
        if add_next {
            if combo.is_none() {
                out.push(RecogWord {
                    word: word.clone(),
                    parts: vec![word.bounding_box()],
                });
                combo = Some(out.len() - 1);
            }
        } else {
            if combo.is_none() {
                out.push(RecogWord {
                    word: word.clone(),
                    parts: Vec::new(),
                });
            }
            combo = None;
        }
    }
    out
}

/// `WERD::copy_on`: appends (or prepends, when `other` starts further
/// left) copies of `other`'s blobs.
fn copy_on(word: &mut Werd, other: &Werd) {
    let reversed = other.bounding_box().left < word.bounding_box().left;
    if reversed {
        let mut blobs = other.blobs.clone();
        blobs.append(&mut word.blobs);
        word.blobs = blobs;
        let mut rej = other.rej.clone();
        rej.append(&mut word.rej);
        word.rej = rej;
    } else {
        word.blobs.extend(other.blobs.iter().cloned());
        word.rej.extend(other.rej.iter().cloned());
    }
}

/// `Tesseract::LSTMRecognizeWord` and `GetRectImage`: the box cropped for
/// the LSTM, extended to the row's ascender and descender lines at the
/// word's middle, padded and clipped to the image; `None` when nothing is
/// left.
pub(crate) fn lstm_box(row: &Row, word_box: TBox, width: i32, height: i32) -> Option<TBox> {
    let mut b = word_box;
    let baseline = row.base_line(((b.left + b.right) / 2) as f32);
    if baseline + row.descdrop < b.bottom as f32 {
        b.bottom = (baseline + row.descdrop) as i32;
    }
    if baseline + row.xheight + row.ascrise > b.top as f32 {
        b.top = (baseline + row.xheight + row.ascrise) as i32;
    }
    let padded = TBox::new(
        b.left - IMAGE_PADDING,
        b.bottom - IMAGE_PADDING,
        b.right + IMAGE_PADDING,
        b.top + IMAGE_PADDING,
    );
    let clipped = padded.intersection(&TBox::new(0, 0, width, height));
    (!clipped.is_null()).then_some(clipped)
}

/// `ComputeWordBounds`: limits a recognized word to the textord words of
/// the combination it significantly overlaps.
fn compute_word_bounds(words: &[Vec<TBox>], w: usize, prev_box: TBox, parts: &[TBox]) -> TBox {
    let union = |b: &[TBox]| b.iter().fold(TBox::default(), |a, x| a.union(x));
    let current = union(&words[w]);
    let next = words.get(w + 1).map_or(TBox::default(), |n| union(n));
    let mut clipped = TBox::default();
    for wb in parts {
        let height_limit = wb.height().min(wb.width() / 2);
        let width_limit = wb.width() / 4;
        let min_overlap = height_limit.max(width_limit);
        let overlap = wb.intersection(&current).width();
        let prev_overlap = wb.intersection(&prev_box).width();
        let next_overlap = wb.intersection(&next).width();
        if overlap > min_overlap {
            if prev_overlap > min_overlap {
                clipped.left = current.left;
            } else if next_overlap > min_overlap {
                clipped.right = current.right;
            } else {
                clipped = clipped.union(wb);
            }
        }
    }
    if clipped.height() <= 0 {
        clipped.top = current.top;
        clipped.bottom = current.bottom;
    }
    if clipped.width() <= 0 {
        clipped = current;
    }
    clipped
}

/// `MoveAndClipBlob`: the blob's box, or the box clipped into `clip`.
fn clip_blob(b: TBox, clip: &TBox) -> TBox {
    if clip.contains(&b) {
        return b;
    }
    let left = clip_to_range(b.left, clip.left, clip.right - 1);
    let right = clip_to_range(b.right, clip.left + 1, clip.right);
    let top = clip_to_range(b.top, clip.bottom + 1, clip.top);
    let bottom = clip_to_range(b.bottom, clip.bottom, clip.top - 1);
    TBox::new(left, bottom, right, top)
}

/// `ClipToRange` with C++ semantics (the lower bound wins a crossed range).
pub(crate) fn clip_to_range(x: i32, lo: i32, hi: i32) -> i32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// `PAGE_RES_IT::ReplaceCurrentWord`: gives each recognized word (its
/// characters' boxes from the LSTM, `chars`) the blobs of the input word
/// whose x middle falls before each character's end, and returns each
/// word's box (`WERD::restricted_bounding_box(false, false)`).
pub(crate) fn replace_current_word(input: &RecogWord, chars: &[Vec<TBox>]) -> Vec<TBox> {
    let sort_by_x_middle = |list: &[CBlob]| {
        let mut v: Vec<TBox> = list.iter().map(CBlob::bounding_box).collect();
        super::cxx::sort(&mut v, &|a: &TBox, b: &TBox| a.x_middle() < b.x_middle());
        v
    };
    let src = sort_by_x_middle(&input.word.blobs);
    let rej = sort_by_x_middle(&input.word.rej);
    let (mut si, mut ri) = (0usize, 0usize);
    let mut clip = TBox::default();
    let mut out = Vec::with_capacity(chars.len());
    for (w, word_chars) in chars.iter().enumerate() {
        clip = compute_word_bounds(chars, w, clip, &input.parts);
        // ComputeBlobEnds: midpoints between neighbouring characters.
        let mut ends: Vec<i32> = Vec::with_capacity(word_chars.len());
        for (c, cb) in word_chars.iter().enumerate() {
            let next = word_chars
                .get(c + 1)
                .or_else(|| chars.get(w + 1).and_then(|n| n.first()));
            let end = next.map_or(i32::MAX, |nb| (cb.right + nb.left) / 2);
            ends.push(clip_to_range(end, clip.left, clip.right));
        }
        if let Some(last) = ends.last_mut() {
            *last = clip.right;
        }
        let mut word_box = TBox::default();
        for (c, &end_x) in ends.iter().enumerate() {
            let mut blob_box = TBox::default();
            while si < src.len() && src[si].x_middle() < end_x {
                blob_box = blob_box.union(&clip_blob(src[si], &clip));
                si += 1;
            }
            while ri < rej.len() && rej[ri].x_middle() < end_x {
                blob_box = blob_box.union(&clip_blob(rej[ri], &clip));
                ri += 1;
            }
            if blob_box.is_null() {
                blob_box = clip_blob(word_chars[c], &clip);
            }
            word_box = word_box.union(&blob_box);
        }
        out.push(word_box);
    }
    out
}
