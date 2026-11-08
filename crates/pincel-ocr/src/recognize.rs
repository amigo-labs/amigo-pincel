//! Recognition of one line image: Tesseract's `LSTMRecognizeWord` and
//! `LSTMRecognizer::RecognizeLine` (`src/ccmain/linerec.cpp`,
//! `src/lstm/lstmrecognizer.cpp`) with `--psm 7`, no dictionary.

use crate::beam::BeamSearch;
use crate::functions::TRand;
use crate::image::scale::{Rgb32, rgb_to_luminance};
use crate::image::{channel_bytes, crop, flatten, invert, network_input, prescale};
use crate::layout::{self, Blob, TBox};
use crate::network::NetworkIo;
use crate::{Model, RecognizeError, RecognizedLine, Word, WordBox};

/// `invert_threshold`: below this mean output confidence the inverted line
/// is tried as well.
const INVERT_THRESHOLD: f32 = 0.7;
/// `kCertaintyScale` in `linerec.cpp`.
const CERTAINTY_SCALE: f32 = 7.0;

/// `LSTMRecognizer::SetRandomSeed`.
pub(crate) fn seeded_rand(model: &Model) -> TRand {
    let seed = i64::from(model.sample_iteration).wrapping_mul(0x1000_0001);
    let mut rand = TRand::new(seed as u64);
    rand.int_rand();
    rand
}

/// Runs the network on a scaled crop: `PreparePixInput` then `Forward`,
/// both from a freshly seeded randomizer.
fn forward(model: &Model, scaled: &Rgb32) -> NetworkIo {
    let mut rand = seeded_rand(model);
    let grey = rgb_to_luminance(scaled);
    let input = network_input(&grey, model.input_height, &mut rand);
    model.network.forward(&input, &mut rand)
}

/// `LSTMRecognizer::OutputStats`: the mean confidence of the best
/// non-null label per step.
fn mean_output(model: &Model, outputs: &NetworkIo) -> f32 {
    let mut buckets = [0i64; 128];
    let mut total = 0i64;
    for t in 0..outputs.steps() {
        let row = outputs.float_row(t);
        let mut best = -f32::MAX;
        let mut label = usize::MAX;
        for (i, &v) in row.iter().enumerate() {
            if v > best {
                best = v;
                label = i;
            }
        }
        if label != model.null_char && label != usize::MAX {
            let bucket = ((127.0 * best) as i32).clamp(0, 127) as usize;
            buckets[bucket] += 1;
            total += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    let sum: i64 = buckets.iter().enumerate().map(|(i, &c)| i as i64 * c).sum();
    ((sum as f64 / total as f64) / 127.0) as f32
}

pub(crate) fn recognize_line(
    model: &Model,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<RecognizedLine, RecognizeError> {
    let (w, h) = (width as usize, height as usize);
    let expected = w
        .checked_mul(h)
        .and_then(|n| n.checked_mul(4))
        .ok_or(RecognizeError::BadImage)?;
    if rgba.len() != expected {
        return Err(RecognizeError::BadImage);
    }
    if w == 0 || h == 0 {
        return Ok(RecognizedLine::default());
    }
    let Some(flat) = flatten(w, h, rgba) else {
        return Ok(RecognizedLine::default());
    };
    let first = flat.data[0] >> 8;
    if flat.data.iter().all(|&p| p >> 8 == first) {
        return Ok(RecognizedLine::default());
    }
    let black = layout::binarize(&channel_bytes(&flat), w, h);
    let blobs = layout::components(&black, w, h);
    let mut words = Vec::new();
    for (line_box, members) in layout::line_boxes(&blobs, width as i32, height as i32) {
        words.extend(recognize_group(
            model,
            &flat,
            &line_box,
            &members,
            height as i32,
        ));
    }
    words.sort_by_key(|wd| wd.bbox.left);
    let text = words
        .iter()
        .map(|wd| wd.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(RecognizedLine { words, text })
}

/// Recognizes one group of words: crops it, runs the network (and on the
/// inverted crop when the outputs are unsure) and decodes the best path.
fn recognize_group(
    model: &Model,
    flat: &Rgb32,
    line_box: &TBox,
    blobs: &[Blob],
    image_height: i32,
) -> Vec<Word> {
    let cropped = crop(flat, line_box);
    if cropped.width == 0 || cropped.height == 0 {
        return Vec::new();
    }
    let (scaled, im_factor) = prescale(&cropped, model.input_height);
    let min_width = model.x_scale;
    if scaled.width < min_width || scaled.height < min_width {
        return Vec::new();
    }
    let scale_factor = min_width as f32 / im_factor;
    let mut outputs = forward(model, &scaled);
    let pos_mean = mean_output(model, &outputs);
    if pos_mean < INVERT_THRESHOLD {
        let inverted = forward(model, &invert(&scaled));
        if mean_output(model, &inverted) > pos_mean {
            outputs = inverted;
        }
    }
    let mut search = BeamSearch::new(&model.recoder, &model.unicharset, model.null_char);
    let flat_outputs: Vec<f32> = (0..outputs.steps())
        .flat_map(|t| outputs.float_row(t).iter().copied())
        .collect();
    search.decode(&flat_outputs, outputs.features);
    let path = search.best_path();
    let path_words = search.words(&path);
    let boxes = word_boxes(&path_words, line_box, scale_factor, blobs);
    path_words
        .iter()
        .zip(boxes)
        .map(|(pw, bbox)| {
            let text: String = pw
                .ids
                .iter()
                .map(|&id| model.unicharset.text(id as usize))
                .collect();
            let confidence = (100.0 + 5.0 * pw.certainty * CERTAINTY_SCALE).clamp(0.0, 100.0);
            Word {
                text,
                bbox: to_word_box(&bbox, image_height),
                confidence: (confidence + 0.5) as u8,
            }
        })
        .collect()
}

/// The pixel boxes of the words: the blobs whose x middle falls in each
/// word's share of the line (`PAGE_RES_IT::ReplaceCurrentWord`), or the
/// word's character cells when it has none.
fn word_boxes(
    words: &[crate::beam::PathWord],
    line: &TBox,
    scale_factor: f32,
    blobs: &[Blob],
) -> Vec<TBox> {
    // Character cells (`InitializeWord`): output steps scaled back to pixels.
    let cells: Vec<Option<TBox>> = words
        .iter()
        .map(|w| {
            let first = w.spans.first()?;
            let last = w.spans.last()?;
            Some(TBox {
                left: (first.0 as f32 * scale_factor).floor() as i16 as i32 + line.left,
                bottom: line.bottom,
                right: (last.1 as f32 * scale_factor).ceil() as i16 as i32 + line.left,
                top: line.top,
            })
        })
        .collect();
    let mut out = Vec::with_capacity(words.len());
    for (i, cell) in cells.iter().enumerate() {
        let start = if i == 0 {
            i32::MIN
        } else {
            boundary(cells[i - 1].as_ref(), cell.as_ref())
        };
        let end = if i + 1 == cells.len() {
            i32::MAX
        } else {
            boundary(cell.as_ref(), cells[i + 1].as_ref())
        };
        let found = blobs
            .iter()
            .map(|b| b.bbox)
            .filter(|b| (start..end).contains(&b.x_middle()))
            .reduce(|a, b| a.union(&b));
        out.push(found.or(*cell).unwrap_or(*line));
    }
    out
}

/// The x position between two neighbouring word cells.
fn boundary(a: Option<&TBox>, b: Option<&TBox>) -> i32 {
    match (a, b) {
        (Some(a), Some(b)) => (a.right + b.left) / 2,
        (Some(a), None) => a.right,
        (None, Some(b)) => b.left,
        (None, None) => 0,
    }
}

fn to_word_box(b: &TBox, image_height: i32) -> WordBox {
    WordBox {
        left: b.left.max(0) as u32,
        top: (image_height - b.top).max(0) as u32,
        width: b.width().max(0) as u32,
        height: b.height().max(0) as u32,
    }
}
