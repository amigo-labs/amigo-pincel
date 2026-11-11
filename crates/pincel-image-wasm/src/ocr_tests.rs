//! The OCR session's source area and reading (docs/specs/ocr-dialog.md
//! AC-10, AC-11, AC-17).

use super::*;
use crate::{close_document, open_image, snapshot_document};
use pincel_image_core::{Color, Layer, SelectionMask};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const INK: [u8; 4] = [10, 20, 30, 255];
const RED: [u8; 4] = [200, 0, 0, 255];

fn color(c: [u8; 4]) -> Color {
    Color::rgba(c[0], c[1], c[2], c[3])
}

/// A `w × h` document whose single layer is filled with `fill`.
fn filled(w: u32, h: u32, fill: [u8; 4]) -> Document {
    let mut doc = Document::new(w, h).expect("doc");
    let px = &mut doc.active_layer_mut().pixels;
    for y in 0..h {
        for x in 0..w {
            px.set_pixel(x, y, color(fill));
        }
    }
    doc
}

fn pixel(area: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 4] {
    let i = (y * area.0 + x) as usize * 4;
    [area.2[i], area.2[i + 1], area.2[i + 2], area.2[i + 3]]
}

fn paint(doc: &mut Document, x: u32, y: u32, c: [u8; 4]) {
    doc.active_layer_mut().pixels.set_pixel(x, y, color(c));
}

/// A mask selecting the pixels for which `keep` is true.
fn mask(w: u32, h: u32, keep: impl Fn(u32, u32) -> u8) -> SelectionMask {
    let mut m = SelectionMask::new_empty(w, h);
    for y in 0..h {
        for x in 0..w {
            m.set(x, y, keep(x, y));
        }
    }
    m
}

#[test]
fn source_area_without_selection_is_the_whole_composite() {
    let mut doc = filled(6, 4, WHITE);
    paint(&mut doc, 2, 1, INK);
    let area = source_area(&doc);
    assert_eq!((area.0, area.1), (6, 4));
    assert_eq!(area.2, compose(doc.layers()).into_raw());
}

#[test]
fn source_area_leaves_out_hidden_layers() {
    let mut doc = filled(6, 4, WHITE);
    paint(&mut doc, 1, 1, RED);
    doc.active_layer_mut().visible = false;
    let mut top = Layer::new_transparent("top", 6, 4);
    top.pixels.set_pixel(4, 2, color(INK));
    doc.insert_layer(1, top).expect("layer");
    let area = source_area(&doc);
    assert_eq!(pixel(&area, 4, 2), INK, "the visible layer above counts");
    assert_eq!(
        pixel(&area, 1, 1),
        [0, 0, 0, 0],
        "the hidden layer does not"
    );
}

#[test]
fn source_area_is_the_selection_bounding_box() {
    let mut doc = filled(10, 8, WHITE);
    paint(&mut doc, 3, 2, INK);
    doc.selection = Some(mask(10, 8, |x, y| {
        if (2..6).contains(&x) && (1..4).contains(&y) {
            255
        } else {
            0
        }
    }));
    let area = source_area(&doc);
    assert_eq!((area.0, area.1), (4, 3));
    assert_eq!(pixel(&area, 1, 1), INK);
    assert_eq!(pixel(&area, 0, 0), WHITE);
}

#[test]
fn source_area_fills_unselected_pixels_with_the_border_median() {
    // An L-shaped selection over grey, with ink in its bounding box but
    // outside the selection: the ink becomes the grey of the border.
    let grey = [180, 170, 160, 255];
    let mut doc = filled(8, 8, grey);
    paint(&mut doc, 6, 6, INK);
    doc.selection = Some(mask(8, 8, |x, y| if x < 3 || y < 3 { 255 } else { 0 }));
    let area = source_area(&doc);
    assert_eq!((area.0, area.1), (8, 8));
    assert_eq!(
        pixel(&area, 6, 6),
        grey,
        "excluded ink takes the background"
    );
}

#[test]
fn source_area_border_median_ignores_ink_inside_the_selection() {
    // White border, dark ink inside: the median stays white, so the
    // unselected corner turns white, not dark.
    let mut doc = filled(9, 9, WHITE);
    for y in 2..5 {
        for x in 2..5 {
            paint(&mut doc, x, y, INK);
        }
    }
    paint(&mut doc, 8, 8, RED);
    doc.selection = Some(mask(9, 9, |x, y| if x + y < 12 { 255 } else { 0 }));
    let area = source_area(&doc);
    assert_eq!(pixel(&area, 3, 3), INK, "selected ink is kept");
    assert_eq!(pixel(&area, 8, 8), WHITE);
}

#[test]
fn source_area_keeps_pixels_at_least_half_selected() {
    // White ends, ink in the middle; the border median is white.
    let mut doc = filled(4, 1, WHITE);
    paint(&mut doc, 1, 0, INK);
    paint(&mut doc, 2, 0, INK);
    doc.selection = Some(mask(4, 1, |x, _| [255, 128, 127, 255][x as usize]));
    let area = source_area(&doc);
    assert_eq!(pixel(&area, 1, 0), INK, "coverage 128 is at least half");
    assert_eq!(pixel(&area, 2, 0), WHITE, "coverage 127 is less than half");
}

#[test]
fn source_area_of_an_empty_selection_is_empty() {
    let mut doc = filled(4, 4, INK);
    doc.selection = Some(SelectionMask::new_empty(4, 4));
    let area = source_area(&doc);
    assert_eq!((area.0, area.1, area.2.len()), (0, 0, 0));
}

fn model(lang: &str) -> Model {
    let path = format!(
        "{}/../pincel-ocr/models/{lang}.traineddata",
        env!("CARGO_MANIFEST_DIR")
    );
    Model::from_bytes(&std::fs::read(path).expect("model file")).expect("model")
}

#[test]
fn ocr_session_reads_the_snapshot_like_pincel_ocr() {
    let dir = format!(
        "{}/../pincel-ocr/tests/pages/eng",
        env!("CARGO_MANIFEST_DIR")
    );
    let png = std::fs::read(format!("{dir}/ui_light.png")).expect("page");
    let want = std::fs::read_to_string(format!("{dir}/ui_light.snapshot.txt")).expect("snapshot");
    let handle = open_image(&png, "image/png").expect("open");
    let snapshot = snapshot_document(handle).expect("snapshot");
    close_document(handle);

    let session = OcrSession::of(&from_snapshot_bytes(&snapshot).expect("snapshot doc"));
    let mut reports = Vec::new();
    let text = session
        .read_with(&model("eng"), &mut |p| reports.push(p))
        .expect("read");
    assert_eq!(format!("{text}\n"), want);

    // Layout reports, then "0 of n" and one report per line.
    let lines: Vec<_> = reports
        .iter()
        .filter_map(|p| match *p {
            OcrProgress::Lines { done, total } => Some((done, total)),
            OcrProgress::Layout { .. } => None,
        })
        .collect();
    assert!(matches!(reports[0], OcrProgress::Layout { .. }));
    let n = lines[0].1;
    assert!(n > 10, "{n} lines");
    assert_eq!(lines, (0..=n).map(|d| (d, n)).collect::<Vec<_>>());
}

#[test]
fn ocr_model_rejects_bytes_that_are_no_model() {
    assert!(Model::from_bytes(b"not a model").is_err());
}

#[test]
fn ocr_progress_serializes_with_its_phase() {
    let json = serde_json::to_string(&OcrProgress::Lines { done: 2, total: 5 }).expect("json");
    assert_eq!(json, r#"{"phase":"lines","done":2,"total":5}"#);
}
