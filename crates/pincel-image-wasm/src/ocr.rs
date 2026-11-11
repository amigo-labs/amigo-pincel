//! Recognize Text (`docs/specs/ocr-dialog.md`): an OCR session over the
//! source area of a document snapshot, read with a `pincel-ocr` model.
//!
//! The source area is composed here, in Rust, from the snapshot the page
//! hands the OCR worker: the composite of the visible layers (the pixels of
//! File ▸ Export ▸ PNG), cropped to the selection's bounding box when there
//! is a selection. Pixels at least half selected keep their colour and
//! alpha; the others take the background around the selection — the median
//! colour of the selected pixels along the selection's border — so the
//! outline of a lasso adds no ink.

use pincel_image_core::codec::snapshot::from_snapshot_bytes;
use pincel_image_core::{Document, compose};
use pincel_ocr::{Control, Model, PageError, Progress};
use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::to_js;

/// Coverage from which a pixel counts as selected: at least half.
const HALF: u8 = 128;

/// A progress report of [`OcrSession::read`], handed to its callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum OcrProgress {
    /// Layout analysis: `done` of `total` units of pixel work.
    Layout {
        /// Units done so far.
        done: u32,
        /// Units in all.
        total: u32,
    },
    /// Recognition: `done` of `total` lines recognized.
    Lines {
        /// Lines recognized so far.
        done: u32,
        /// Lines found by the layout analysis.
        total: u32,
    },
}

impl From<Progress> for OcrProgress {
    fn from(p: Progress) -> Self {
        let small = |v: u64| u32::try_from(v).unwrap_or(u32::MAX);
        match p {
            Progress::Layout { done, total } => Self::Layout {
                done: small(done),
                total: small(total),
            },
            Progress::Lines { done, total } => Self::Lines {
                done: small(done as u64),
                total: small(total as u64),
            },
        }
    }
}

/// A loaded OCR language model (a `tessdata_fast` `.traineddata` file).
#[wasm_bindgen]
pub struct OcrModel {
    model: Model,
}

/// Loads a language model; fails when the engine rejects the bytes.
#[wasm_bindgen]
pub fn ocr_model(bytes: &[u8]) -> Result<OcrModel, JsError> {
    Model::from_bytes(bytes)
        .map(|model| OcrModel { model })
        .map_err(|e| JsError::new(&e.to_string()))
}

/// The source area of one recognition, ready to be read.
#[wasm_bindgen]
pub struct OcrSession {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// An OCR session over the source area of the document in `snapshot`
/// (bytes from `snapshot_document`).
#[wasm_bindgen]
pub fn ocr_session(snapshot: &[u8]) -> Result<OcrSession, JsError> {
    let doc = from_snapshot_bytes(snapshot).map_err(to_js)?;
    Ok(OcrSession::of(&doc))
}

impl OcrSession {
    /// The session over `doc`'s source area.
    fn of(doc: &Document) -> Self {
        let (width, height, rgba) = source_area(doc);
        Self {
            width,
            height,
            rgba,
        }
    }

    /// Reads the source area with `model`, reporting progress to `report`.
    fn read_with(
        &self,
        model: &Model,
        report: &mut dyn FnMut(OcrProgress),
    ) -> Result<String, PageError> {
        let page = model.read_page(self.width, self.height, &self.rgba, &mut |p| {
            report(p.into());
            Control::Continue
        })?;
        Ok(page.text)
    }
}

#[wasm_bindgen]
impl OcrSession {
    /// Width of the source area in pixels.
    #[wasm_bindgen(getter)]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the source area in pixels.
    #[wasm_bindgen(getter)]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Reads the source area with `model` and returns the page text
    /// (`docs/specs/ocr-page.md`). `progress` is called with an
    /// `OcrProgress` object during layout analysis and after each line;
    /// the reading cannot be cancelled from here — the page ends the
    /// worker it runs in.
    pub fn read(&self, model: &OcrModel, progress: &js_sys::Function) -> Result<String, JsError> {
        self.read_with(&model.model, &mut |p| {
            if let Ok(value) = serde_wasm_bindgen::to_value(&p) {
                // A throwing callback must not stop the reading.
                let _ = progress.call1(&JsValue::NULL, &value);
            }
        })
        .map_err(|e| JsError::new(&e.to_string()))
    }
}

/// The source area of `doc` as `(width, height, rgba)` (module docs).
fn source_area(doc: &Document) -> (u32, u32, Vec<u8>) {
    let img = compose(doc.layers());
    let (w, h) = (img.width(), img.height());
    let rgba = img.into_raw();
    let Some(mask) = doc.selection.as_ref() else {
        return (w, h, rgba);
    };
    if mask.width() != w || mask.height() != h {
        // A mask is always canvas-sized; anything else reads the canvas.
        return (w, h, rgba);
    }
    let Some(bounds) = mask.bounding_box() else {
        return (0, 0, Vec::new());
    };
    let x0 = u32::try_from(bounds.x.max(0)).unwrap_or(0).min(w);
    let y0 = u32::try_from(bounds.y.max(0)).unwrap_or(0).min(h);
    let x1 = x0.saturating_add(bounds.w).min(w);
    let y1 = y0.saturating_add(bounds.h).min(h);
    let selected = |x: i64, y: i64| {
        x >= 0
            && y >= 0
            && x < i64::from(w)
            && y < i64::from(h)
            && mask.coverage(x as u32, y as u32) >= HALF
    };
    let at = |x: u32, y: u32| (y as usize * w as usize + x as usize) * 4;

    // Per-channel histograms of the selected pixels on the border.
    let mut hist = [[0u32; 256]; 4];
    let mut border = 0u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let (sx, sy) = (i64::from(x), i64::from(y));
            if !selected(sx, sy) {
                continue;
            }
            let edge = !selected(sx - 1, sy)
                || !selected(sx + 1, sy)
                || !selected(sx, sy - 1)
                || !selected(sx, sy + 1);
            if edge {
                let i = at(x, y);
                for (c, channel) in hist.iter_mut().enumerate() {
                    channel[usize::from(rgba[i + c])] += 1;
                }
                border += 1;
            }
        }
    }
    let background = hist.map(|channel| median(&channel, border));

    let (aw, ah) = (x1 - x0, y1 - y0);
    let mut out = Vec::with_capacity(aw as usize * ah as usize * 4);
    for y in y0..y1 {
        for x in x0..x1 {
            if selected(i64::from(x), i64::from(y)) {
                let i = at(x, y);
                out.extend_from_slice(&rgba[i..i + 4]);
            } else {
                out.extend_from_slice(&background);
            }
        }
    }
    (aw, ah, out)
}

/// The lower median of `count` values given as a histogram; 0 without
/// values.
fn median(hist: &[u32; 256], count: u32) -> u8 {
    if count == 0 {
        return 0;
    }
    let target = (count - 1) / 2;
    let mut seen = 0u32;
    for (value, &n) in hist.iter().enumerate() {
        seen += n;
        if seen > target {
            return value as u8;
        }
    }
    u8::MAX
}

#[cfg(test)]
#[path = "ocr_tests.rs"]
mod tests;
