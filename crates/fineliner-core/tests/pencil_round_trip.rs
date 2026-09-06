//! End-to-end M5 exit criterion (spec §18 M5): open a PNG, paint with the
//! Pencil, export a PNG, and confirm the exported pixels are correct.
//!
//! This mirrors the browser demo without a browser: it exercises decode →
//! command apply → compose → encode → decode at the core level.

use fineliner_core::codec::{decode, to_png_bytes};
use fineliner_core::command::Command;
use fineliner_core::{compose, Brush, Color, Document, Pencil, Point};

/// Builds a PNG of a solid white WxH image.
fn white_png(w: u32, h: u32) -> Vec<u8> {
    let mut buf = fineliner_core::ImageBuffer::new_transparent(w, h);
    for y in 0..h {
        for x in 0..w {
            buf.set_pixel(x, y, Color::WHITE);
        }
    }
    to_png_bytes(&buf, 6).unwrap()
}

#[test]
fn open_png_paint_pencil_export_png_is_pixel_correct() {
    // 1. Open: decode a white PNG into a single-layer document.
    let png = white_png(32, 32);
    let imported = decode(&png).unwrap();
    let mut doc = Document::from_pixels(imported).unwrap();

    // 2. Paint: a black pencil stroke across the middle.
    let pencil = Pencil::new(Brush::new(5, Color::BLACK, 1.0));
    let mut cmd = pencil
        .stroke(0, &[Point::new(4.0, 16.0), Point::new(28.0, 16.0)], &doc)
        .expect("stroke should touch canvas");
    cmd.apply(&mut doc).unwrap();

    // 3. Export: compose and encode to PNG.
    let composite = compose(doc.layers());
    let exported = to_png_bytes(&composite, 6).unwrap();

    // 4. Verify: re-decode and check painted vs untouched pixels.
    let back = decode(&exported).unwrap();
    assert_eq!(back.width(), 32);
    assert_eq!(back.height(), 32);
    // On the stroke line: black.
    assert_eq!(back.get_pixel(16, 16), Some(Color::BLACK));
    // A corner far from the stroke: still white.
    assert_eq!(back.get_pixel(0, 0), Some(Color::WHITE));
}

#[test]
fn paint_then_undo_then_export_matches_original() {
    let png = white_png(16, 16);
    let mut doc = Document::from_pixels(decode(&png).unwrap()).unwrap();

    let pencil = Pencil::new(Brush::new(8, Color::BLACK, 1.0));
    let mut cmd = pencil.stroke(0, &[Point::new(8.0, 8.0)], &doc).unwrap();
    cmd.apply(&mut doc).unwrap();
    cmd.revert(&mut doc).unwrap();

    let exported = to_png_bytes(&compose(doc.layers()), 6).unwrap();
    let back = decode(&exported).unwrap();
    // Every pixel back to white after undo.
    assert!(back
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .all(|p| p == &[255, 255, 255, 255]));
}
