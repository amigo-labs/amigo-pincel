//! The budget session of docs/specs/performance-image-undo-memory.md AC-03,
//! natively: a 3840×2160, 10-layer document with 50 undo steps — 40 brush
//! strokes within 512 px, 5 whole-layer adjustments, 3 merge-downs and 2
//! pastes of 1024² without canvas expansion. The undo stack's reported byte
//! count must equal what its commands hold, and document plus undo data
//! must stay within the 1 GB budget.

use pincel_image_core::{
    AddLayer, Brush, Color, CommandBus, Document, ImageBuffer, MergeDown, PasteLayer, Pencil,
    Point, Rect, SetPixels,
};

const W: u32 = 3840;
const H: u32 = 2160;
const MB: usize = 1024 * 1024;

/// Pixel bytes of the document's layers.
fn document_bytes(doc: &Document) -> usize {
    doc.layers().iter().map(|l| l.pixels.data().len()).sum()
}

/// A whole-layer adjustment: every pixel of `layer` changes. A flat fill
/// stands in for an effect; the undo data is the same whole layer either
/// way, and it keeps this debug-build test fast.
fn adjust(bus: &mut CommandBus, layer: usize) {
    let data = vec![40 + layer as u8; (W * H * 4) as usize];
    let after = ImageBuffer::from_raw(W, H, data).expect("buffer");
    bus.apply(Box::new(SetPixels::new(
        layer,
        Rect::new(0, 0, W, H),
        after,
    )))
    .expect("adjust");
}

#[test]
fn budget_session_reports_its_undo_bytes_and_fits_the_budget() {
    let mut setup = CommandBus::new(Document::new(W, H).expect("doc"));
    for _ in 1..10 {
        let top = setup.document.layer_count() - 1;
        setup.apply(Box::new(AddLayer::above(top))).expect("add");
    }
    let mut bus = CommandBus::new(setup.document);
    assert_eq!(bus.document.layer_count(), 10);

    // 40 strokes within 512 px, spread over the layers and the canvas.
    for i in 0..40u32 {
        let layer = (i % 10) as usize;
        let x = (i * 397 % (W - 600)) as f32 + 40.0;
        let y = (i * 211 % (H - 600)) as f32 + 40.0;
        let pencil = Pencil::new(Brush::new(24, Color::rgba(i as u8 * 6, 90, 200, 255), 1.0));
        let pts = [
            Point::new(x, y),
            Point::new(x + 480.0, y + 120.0),
            Point::new(x + 200.0, y + 470.0),
        ];
        let cmd = pencil
            .stroke(layer, &pts, &bus.document)
            .expect("on canvas");
        bus.apply(Box::new(cmd.with_stroke(u64::from(i) + 1)))
            .expect("stroke");
    }
    // 5 whole-layer adjustments.
    for layer in [0, 2, 4, 6, 8] {
        adjust(&mut bus, layer);
    }
    // 3 merge-downs.
    for index in [9, 7, 5] {
        bus.apply(Box::new(MergeDown::at(index))).expect("merge");
    }
    // 2 pastes of 1024² inside the canvas.
    for k in 0..2u8 {
        let data = vec![k * 100 + 50; 1024 * 1024 * 4];
        let px = ImageBuffer::from_raw(1024, 1024, data).expect("paste");
        bus.apply(Box::new(PasteLayer::at(
            px,
            300 + i32::from(k) * 1500,
            400,
            false,
        )))
        .expect("paste");
    }

    assert_eq!(bus.history.undo_depth(), 50);
    let reported = bus.history.stored_bytes();
    assert_eq!(reported, bus.history.recount_bytes(), "reported = stored");
    let total = document_bytes(&bus.document) + reported;
    println!(
        "document {} MiB + undo {} MiB = {} MiB",
        document_bytes(&bus.document) / MB,
        reported / MB,
        total / MB
    );
    assert!(reported <= 512 * MB, "undo data within the byte cap");
    assert!(total <= 1024 * MB, "document and undo within 1 GB");
}
