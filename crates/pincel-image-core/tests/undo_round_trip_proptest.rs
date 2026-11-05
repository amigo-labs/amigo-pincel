//! Undo / redo restore the document byte-identically after random command
//! sequences (docs/specs/performance-image-undo-memory.md AC-01, AC-04).
//!
//! Canvases are larger than one undo tile so edits span several tiles and
//! the tile grid's clipped edge tiles.

use pincel_image_core::{
    AddLayer, Anchor, Brush, Color, Command, CommandBus, CropToSelection, Document, DuplicateLayer,
    Fill, FillOptions, FlattenImage, ImageBuffer, Interpolation, MergeDown, MoveLayer, PasteLayer,
    Pencil, Point, Rect, RemoveLayer, ResizeCanvas, ScaleImage, SelectionMask, SetLayerOpacity,
    SetPixels, SetSelection,
};
use proptest::prelude::*;

/// One command; coordinates and indices are reduced to the document.
#[derive(Debug, Clone)]
enum Op {
    /// A pencil drag of `segments` merged commands from `(x, y)`.
    Stroke {
        layer: usize,
        x: u16,
        y: u16,
        segments: u8,
        size: u8,
        shade: u8,
    },
    Fill {
        layer: usize,
        x: u16,
        y: u16,
        shade: u8,
    },
    /// A whole-layer adjustment (invert), as effects commit them.
    Invert(usize),
    Paste {
        x: i16,
        y: i16,
        w: u16,
        h: u16,
        expand: bool,
    },
    MergeDown(usize),
    Flatten,
    AddLayer(usize),
    Remove(usize),
    Duplicate(usize),
    Move(usize, usize),
    Opacity(usize, u8),
    Select(u16, u16, u16, u16),
    Crop,
    Scale(u16, u16),
    Resize(u16, u16, u8),
}

fn op() -> impl Strategy<Value = Op> {
    let i = 0usize..8;
    let c = 0u16..400;
    prop_oneof![
        6 => (i.clone(), c.clone(), c.clone(), 1u8..6, 1u8..40, any::<u8>()).prop_map(
            |(layer, x, y, segments, size, shade)| Op::Stroke { layer, x, y, segments, size, shade }
        ),
        2 => (i.clone(), c.clone(), c.clone(), any::<u8>())
            .prop_map(|(layer, x, y, shade)| Op::Fill { layer, x, y, shade }),
        2 => i.clone().prop_map(Op::Invert),
        2 => (-50i16..300, -50i16..300, 1u16..200, 1u16..200, any::<bool>())
            .prop_map(|(x, y, w, h, expand)| Op::Paste { x, y, w, h, expand }),
        2 => i.clone().prop_map(Op::MergeDown),
        1 => Just(Op::Flatten),
        2 => i.clone().prop_map(Op::AddLayer),
        1 => i.clone().prop_map(Op::Remove),
        1 => i.clone().prop_map(Op::Duplicate),
        1 => (i.clone(), i.clone()).prop_map(|(a, b)| Op::Move(a, b)),
        1 => (i, any::<u8>()).prop_map(|(a, o)| Op::Opacity(a, o)),
        1 => (c.clone(), c.clone(), 1u16..200, 1u16..200)
            .prop_map(|(x, y, w, h)| Op::Select(x, y, w, h)),
        1 => Just(Op::Crop),
        1 => (20u16..320, 20u16..320).prop_map(|(w, h)| Op::Scale(w, h)),
        1 => (20u16..320, 20u16..320, 0u8..9).prop_map(|(w, h, a)| Op::Resize(w, h, a)),
    ]
}

fn shade(v: u8) -> Color {
    Color::rgba(v, v.wrapping_mul(7), v.wrapping_mul(13), 128 + v / 2)
}

/// A document with three patterned layers, larger than one undo tile.
fn document() -> Document {
    let (w, h) = (300u32, 170u32);
    let mut base = ImageBuffer::new_transparent(w, h);
    for y in 0..h {
        for x in 0..w {
            base.set_pixel(x, y, shade(((x / 9 + y / 7) % 251) as u8));
        }
    }
    let mut bus = CommandBus::new(Document::from_pixels(base).expect("doc"));
    for k in 0..2u8 {
        bus.apply(Box::new(AddLayer::above(bus.document.layer_count() - 1)))
            .expect("add");
        let top = bus.document.layer_count() - 1;
        let patch = ImageBuffer::from_raw(
            120,
            90,
            (0..120 * 90)
                .flat_map(|i| {
                    let c = shade((i as u8).wrapping_add(k * 40));
                    [c.r, c.g, c.b, c.a]
                })
                .collect(),
        )
        .expect("patch");
        bus.apply(Box::new(SetPixels::new(
            top,
            Rect::new(40 + 90 * i32::from(k), 30, 120, 90),
            patch,
        )))
        .expect("paint");
    }
    bus.document
}

/// Applies `op` to the bus; a refused command leaves the document as it was.
fn run(bus: &mut CommandBus, op: &Op, stroke_id: &mut u64) {
    let doc = &bus.document;
    let n = doc.layer_count();
    let (w, h) = (doc.canvas.width(), doc.canvas.height());
    let at = |v: u16, max: u32| f32::from(v) % max as f32;
    match op {
        Op::Stroke {
            layer,
            x,
            y,
            segments,
            size,
            shade: s,
        } => {
            *stroke_id += 1;
            let layer = layer % n;
            let pencil = Pencil::new(Brush::new(u32::from(*size), shade(*s), 0.8));
            let mut from = Point::new(at(*x, w), at(*y, h));
            for k in 0..*segments {
                let to = Point::new(
                    (from.x + 37.0 * f32::from(k + 1)) % w as f32,
                    (from.y + 23.0 * f32::from(k + 2)) % h as f32,
                );
                if let Some(cmd) = pencil.stroke(layer, &[from, to], &bus.document) {
                    let _ = bus.apply(Box::new(cmd.with_stroke(*stroke_id)));
                }
                from = to;
            }
        }
        Op::Fill {
            layer,
            x,
            y,
            shade: s,
        } => {
            let fill = Fill::new(shade(*s), FillOptions::default());
            let p = Point::new(at(*x, w), at(*y, h));
            if let Some(cmd) = fill.fill(layer % n, p, &bus.document) {
                let _ = bus.apply(Box::new(cmd));
            }
        }
        Op::Invert(layer) => {
            let layer = layer % n;
            let px = &bus.document.layers()[layer].pixels;
            if px.width() == 0 {
                return;
            }
            let inverted: Vec<u8> = px
                .data()
                .chunks(4)
                .flat_map(|p| [255 - p[0], 255 - p[1], 255 - p[2], p[3]])
                .collect();
            let after = ImageBuffer::from_raw(px.width(), px.height(), inverted).expect("buf");
            let region = Rect::new(0, 0, px.width(), px.height());
            let _ = bus.apply(Box::new(SetPixels::new(layer, region, after)));
        }
        Op::Paste {
            x,
            y,
            w: pw,
            h: ph,
            expand,
        } => {
            let mut px = ImageBuffer::new_transparent(u32::from(*pw), u32::from(*ph));
            for yy in 0..u32::from(*ph) {
                for xx in 0..u32::from(*pw) {
                    px.set_pixel(xx, yy, shade((xx * 3 + yy) as u8));
                }
            }
            let cmd = PasteLayer::at(px, i32::from(*x), i32::from(*y), *expand);
            let _ = bus.apply(Box::new(cmd));
        }
        Op::MergeDown(i) => {
            let _ = bus.apply(Box::new(MergeDown::at(i % n)));
        }
        Op::Flatten => {
            let _ = bus.apply(Box::new(FlattenImage::new()));
        }
        Op::AddLayer(i) => {
            let _ = bus.apply(Box::new(AddLayer::above(i % n)));
        }
        Op::Remove(i) => {
            let _ = bus.apply(Box::new(RemoveLayer::at(i % n)));
        }
        Op::Duplicate(i) => {
            let _ = bus.apply(Box::new(DuplicateLayer::at(i % n)));
        }
        Op::Move(a, b) => {
            let _ = bus.apply(Box::new(MoveLayer::new(a % n, b % n)));
        }
        Op::Opacity(i, o) => {
            let _ = bus.apply(Box::new(SetLayerOpacity::new(i % n, f32::from(*o) / 255.0)));
        }
        Op::Select(x, y, sw, sh) => {
            let r = Rect::new(
                i32::from(*x) % w as i32,
                i32::from(*y) % h as i32,
                u32::from(*sw),
                u32::from(*sh),
            );
            let mask = SelectionMask::rectangle(w, h, r);
            let _ = bus.apply(Box::new(SetSelection::replace(mask)));
        }
        Op::Crop => {
            let _ = bus.apply(Box::new(CropToSelection::new()));
        }
        Op::Scale(sw, sh) => {
            let cmd = ScaleImage::new(u32::from(*sw), u32::from(*sh), Interpolation::Bilinear);
            let _ = bus.apply(Box::new(cmd));
        }
        Op::Resize(rw, rh, a) => {
            let anchor = [
                Anchor::TopLeft,
                Anchor::TopCenter,
                Anchor::TopRight,
                Anchor::CenterLeft,
                Anchor::Center,
                Anchor::CenterRight,
                Anchor::BottomLeft,
                Anchor::BottomCenter,
                Anchor::BottomRight,
            ][usize::from(*a) % 9];
            let cmd = ResizeCanvas::new(u32::from(*rw), u32::from(*rh)).with_anchor(anchor);
            let _ = bus.apply(Box::new(cmd));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn undo_all_then_redo_all_is_byte_identical(ops in prop::collection::vec(op(), 1..14)) {
        let original = document();
        let mut bus = CommandBus::new(original.clone());
        let mut stroke_id = 0;
        for op in &ops {
            run(&mut bus, op, &mut stroke_id);
            prop_assert_eq!(bus.history.stored_bytes(), bus.history.recount_bytes());
        }
        let last = bus.document.clone();
        let steps = bus.history.undo_depth();
        while bus.undo().expect("undo") {}
        prop_assert!(bus.document == original, "undo of {} steps restores the original", steps);
        while bus.redo().expect("redo") {}
        prop_assert!(bus.document == last, "redo of {} steps restores the final document", steps);
        prop_assert_eq!(bus.history.stored_bytes(), bus.history.recount_bytes());
    }
}

/// A paste that grows the canvas undoes to the prior size, layers and
/// pixels (AC-04).
#[test]
fn undo_of_expanding_paste_restores_size_and_pixels() {
    let original = document();
    let mut bus = CommandBus::new(original.clone());
    let px = ImageBuffer::new_transparent(200, 200);
    bus.apply(Box::new(PasteLayer::at(px, 250, 100, true)))
        .expect("paste");
    assert_eq!(
        (bus.document.canvas.width(), bus.document.canvas.height()),
        (450, 300)
    );
    let grown = bus.document.clone();
    assert!(bus.undo().expect("undo"));
    assert!(bus.document == original);
    assert!(bus.redo().expect("redo"));
    assert!(bus.document == grown);
}

/// Every canvas-size change undoes to the exact prior state (AC-04).
#[test]
fn undo_of_canvas_size_changes_restores_the_document() {
    let original = document();
    let cmds: Vec<Box<dyn Command>> = vec![
        Box::new(ScaleImage::new(97, 61, Interpolation::Bicubic)),
        Box::new(ResizeCanvas::new(333, 90).with_anchor(Anchor::Center)),
        Box::new(SetSelection::replace(SelectionMask::rectangle(
            300,
            170,
            Rect::new(10, 20, 150, 100),
        ))),
        Box::new(CropToSelection::new()),
    ];
    for cmd in cmds {
        let mut bus = CommandBus::new(original.clone());
        if cmd.label() == "Crop to Selection" {
            bus.apply(Box::new(SetSelection::replace(SelectionMask::rectangle(
                300,
                170,
                Rect::new(10, 20, 150, 100),
            ))))
            .expect("select");
        }
        let before = bus.document.clone();
        bus.apply(cmd).expect("apply");
        assert!(bus.document != before);
        assert!(bus.undo().expect("undo"));
        assert!(bus.document == before);
    }
}
