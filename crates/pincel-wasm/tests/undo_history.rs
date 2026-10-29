//! Pixel-mode undo history as seen through the wasm surface.
//! See `docs/specs/pixel-undo-history.md`.

use pincel_wasm::Document;

const RED: u32 = 0xff0000ff;

fn alpha_at(doc: &Document, x: u32, y: u32) -> u8 {
    let frame = doc.compose(0, 1).expect("compose ok");
    let off = ((y * doc.width() + x) * 4 + 3) as usize;
    frame.pixels()[off]
}

/// Paint one single-pixel stroke and close the gesture.
fn stroke(doc: &mut Document, x: i32, y: i32) {
    doc.apply_tool("pencil", x, y, RED).expect("pencil");
    doc.end_stroke();
}

#[test]
fn document_history_holds_100_entries_and_the_101st_drops_the_oldest() {
    let mut doc = Document::new(16, 16).expect("dims");
    for i in 0..101 {
        stroke(&mut doc, i % 16, i / 16);
    }
    assert_eq!(doc.undo_depth(), 100, "history is capped at 100");

    for _ in 0..100 {
        assert!(doc.undo());
    }
    assert!(!doc.undo(), "only the remaining 100 entries can be undone");
    assert_eq!(alpha_at(&doc, 0, 0), 255, "the oldest stroke was dropped");
    assert_eq!(alpha_at(&doc, 1, 0), 0, "the second stroke was undone");
}

#[test]
fn frame_layer_and_onion_skin_changes_add_no_history_entry() {
    let mut doc = Document::new(4, 4).expect("dims");
    doc.add_frame(100).expect("add frame");
    let layer = doc.add_layer("fg").expect("add layer");
    stroke(&mut doc, 0, 0);
    assert!(doc.undo());
    assert_eq!((doc.undo_depth(), doc.redo_depth()), (2, 1));

    doc.set_current_frame(1).expect("switch frame");
    doc.set_current_frame(0).expect("switch back");
    doc.set_active_layer(layer);
    doc.set_onion_skin(true, 1, 1, 76, 76);
    doc.set_onion_skin(false, 0, 0, 0, 0);

    assert_eq!(doc.undo_depth(), 2, "no entry added");
    assert_eq!(doc.redo_depth(), 1, "redo history untouched");
}

#[test]
fn selection_changes_add_no_history_entry() {
    let mut doc = Document::new(8, 8).expect("dims");
    stroke(&mut doc, 0, 0);
    stroke(&mut doc, 1, 0);
    assert!(doc.undo());
    assert_eq!((doc.undo_depth(), doc.redo_depth()), (1, 1));

    doc.set_selection(1, 1, 4, 4);
    doc.select_rect(2, 2, 3, 3, "replace").expect("replace");
    doc.clear_selection();

    assert_eq!(doc.undo_depth(), 1, "no entry added");
    assert_eq!(doc.redo_depth(), 1, "redo history untouched");
}

#[test]
fn gpl_import_is_one_undoable_entry() {
    let mut doc = Document::new(4, 4).expect("dims");
    let gpl = "GIMP Palette\nName: Two\n#\n255 0 0 red\n0 255 0 green\n";
    assert_eq!(doc.import_gpl(gpl).expect("import"), 2);
    assert_eq!(doc.undo_depth(), 1);
    assert!(doc.undo());
    assert_eq!(doc.palette_count(), 32, "DB32 restored");
}

#[test]
fn failed_edits_add_no_entry_and_keep_redo_history() {
    let mut doc = Document::new(4, 4).expect("dims");
    stroke(&mut doc, 0, 0);
    stroke(&mut doc, 1, 0);
    assert!(doc.undo());
    assert_eq!((doc.undo_depth(), doc.redo_depth()), (1, 1));

    assert!(
        doc.apply_tool("pencil", 9, 9, RED).is_err(),
        "pixel outside the canvas"
    );
    assert!(doc.rename_layer(999, "ghost").is_err(), "unknown layer");

    assert_eq!(doc.undo_depth(), 1, "failed edits add no entry");
    assert_eq!(doc.redo_depth(), 1, "failed edits keep the redo history");
    assert!(doc.redo().expect("redo ok"));
    assert_eq!(alpha_at(&doc, 1, 0), 255);
}

#[test]
fn save_keeps_history_and_edits_before_it_stay_undoable() {
    let mut doc = Document::new(4, 4).expect("dims");
    stroke(&mut doc, 0, 0);
    stroke(&mut doc, 1, 0);

    doc.save_aseprite().expect("save ok");

    assert_eq!(doc.undo_depth(), 2, "history kept across save");
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(alpha_at(&doc, 0, 0), 0, "pre-save edit undone");
    assert_eq!(alpha_at(&doc, 1, 0), 0, "pre-save edit undone");
}
