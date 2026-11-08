//! Recognition of one line image: Tesseract's `LSTMRecognizeWord` and
//! `LSTMRecognizer::RecognizeLine` (`src/ccmain/linerec.cpp`,
//! `src/lstm/lstmrecognizer.cpp`) with `--psm 7`, no dictionary.

use crate::beam::BeamSearch;
use crate::functions::TRand;
use crate::image::scale::{Rgb32, rgb_to_luminance};
use crate::image::{channel_bytes, crop, flatten, invert, network_input, prescale};
use crate::network::NetworkIo;
use crate::textord::{self, TBox, clip_to_range};
use crate::{Model, RecognizeError, RecognizedLine, Word, WordBox};

/// `invert_threshold`: below this mean output confidence the inverted line
/// is tried as well.
const INVERT_THRESHOLD: f32 = 0.7;
/// `UNICHAR_SPACE`: the id of the space in every unicharset.
const UNICHAR_SPACE: i32 = 0;
/// The largest width or height Tesseract recognizes (`INT16_MAX`).
const MAX_SIDE: usize = i16::MAX as usize;
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
    // Tesseract's thresholder refuses images beyond INT16_MAX in either
    // direction, and its page layout stores coordinates as int16.
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Ok(RecognizedLine::default());
    }
    let Some(flat) = flatten(w, h, rgba) else {
        return Ok(RecognizedLine::default());
    };
    let first = flat.data[0] >> 8;
    if flat.data.iter().all(|&p| p >> 8 == first) {
        return Ok(RecognizedLine::default());
    }
    let black = textord::binarize(&channel_bytes(&flat), w, h);
    let (iw, ih) = (width as i32, height as i32);
    // Each row's words with their certainties (`recog_all_words`, pass 1).
    let mut rows: Vec<Vec<(bool, Word, f32)>> = Vec::new();
    for row in textord::page_layout(&black, w, h) {
        let mut words = Vec::new();
        for input in textord::recog_words(&row) {
            let Some(line_box) = textord::lstm_box(&row, input.word.bounding_box(), iw, ih) else {
                continue;
            };
            let found = recognize_group(model, &flat, &line_box);
            if found.is_empty() {
                continue;
            }
            let chars: Vec<Vec<TBox>> = found.iter().map(|f| f.chars.clone()).collect();
            let boxes = textord::replace_current_word(&input, &chars);
            for (f, b) in found.into_iter().zip(boxes) {
                words.push((
                    f.all_spaces,
                    Word {
                        text: f.text,
                        bbox: to_word_box(&b, iw, ih),
                        confidence: f.confidence,
                    },
                    f.certainty,
                ));
            }
        }
        rows.push(words);
    }
    // Textord::CleanupSingleRowResult: keep the row with the best mean
    // certainty; then recog_all_words drops the words that are all spaces.
    let mut best: Option<(usize, f32)> = None;
    for (i, r) in rows.iter().enumerate() {
        if r.is_empty() {
            continue;
        }
        let mean = r.iter().map(|w| w.2).sum::<f32>() / r.len() as f32;
        if best.is_none_or(|(_, b)| b < mean) {
            best = Some((i, mean));
        }
    }
    let mut words: Vec<Word> = match best {
        Some((i, _)) => rows
            .swap_remove(i)
            .into_iter()
            .filter(|w| !w.0)
            .map(|w| w.1)
            .collect(),
        None => Vec::new(),
    };
    // Tesseract's word order, made strictly left to right.
    words.sort_by_key(|wd| wd.bbox.left);
    let text = words
        .iter()
        .map(|wd| wd.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Ok(RecognizedLine { words, text })
}

/// A word the LSTM read: its text, confidence and certainty, and the
/// boxes of its characters (`RecodeBeamSearch::InitializeWord`).
struct FoundWord {
    text: String,
    /// Every character is `UNICHAR_SPACE` (`WERD_CHOICE::IsAllSpaces`).
    all_spaces: bool,
    confidence: u8,
    certainty: f32,
    chars: Vec<TBox>,
}

/// Recognizes one crop (`LSTMRecognizer::RecognizeLine`): runs the
/// network (and on the inverted crop when the outputs are unsure) and
/// decodes the best path into words.
fn recognize_group(model: &Model, flat: &Rgb32, line_box: &TBox) -> Vec<FoundWord> {
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
    search
        .words(&path)
        .iter()
        .map(|pw| {
            let text: String = pw
                .ids
                .iter()
                .map(|&id| model.unicharset.text(id as usize))
                .collect();
            let certainty = pw.certainty * CERTAINTY_SCALE;
            let confidence = (100.0 + 5.0 * certainty).clamp(0.0, 100.0);
            let chars = pw
                .spans
                .iter()
                .map(|&(a, b)| TBox {
                    left: i32::from((a as f32 * scale_factor).floor() as i16) + line_box.left,
                    bottom: line_box.bottom,
                    right: i32::from((b as f32 * scale_factor).ceil() as i16) + line_box.left,
                    top: line_box.top,
                })
                .collect();
            FoundWord {
                text,
                all_spaces: pw.ids.iter().all(|&id| id == UNICHAR_SPACE),
                confidence: (confidence + 0.5) as u8,
                certainty,
                chars,
            }
        })
        .collect()
}

/// `PageIterator::BoundingBox` for a word: Tesseract's box in top-down
/// image coordinates, clipped to the image.
fn to_word_box(b: &TBox, width: i32, height: i32) -> WordBox {
    let left = clip_to_range(b.left, 0, width);
    let top = clip_to_range(height - b.top, 0, height);
    let right = clip_to_range(b.right, left, width);
    let bottom = clip_to_range(height - b.bottom, top, height);
    WordBox {
        left: left as u32,
        top: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    }
}
