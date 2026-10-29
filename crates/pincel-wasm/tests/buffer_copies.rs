//! Byte buffers handed to JS are fresh copies (docs/specs/wasm-npm-api.md AC-09).
//!
//! Every getter that returns bytes must give the caller its own buffer:
//! mutating one result must change neither the document nor the next read.
//! The JS-side counterpart lives in `ui/e2e/wasm-npm-api.spec.ts`.

use pincel_wasm::Document;

const RED: u32 = 0xff0000ff;

fn painted_doc() -> Document {
    let mut doc = Document::new(4, 4).expect("dims");
    doc.apply_tool("pencil", 1, 2, RED).expect("paint");
    doc.end_stroke();
    doc
}

/// Overwrites every byte so any shared storage would show the change.
fn scribble(bytes: &mut [u8]) {
    bytes.iter_mut().for_each(|b| *b = b.wrapping_add(0x5a));
}

#[test]
fn compose_frame_pixels_mutating_a_read_leaves_later_reads_unchanged() {
    let doc = painted_doc();
    let frame = doc.compose(0, 1).expect("compose");
    let original = frame.pixels();

    let mut first = frame.pixels();
    scribble(&mut first);

    assert_eq!(frame.pixels(), original);
    assert_eq!(doc.compose(0, 1).expect("compose").pixels(), original);
}

#[test]
fn save_aseprite_mutating_the_bytes_leaves_the_document_unchanged() {
    let doc = painted_doc();
    let original = doc.save_aseprite().expect("save");

    let mut first = doc.save_aseprite().expect("save");
    scribble(&mut first);

    assert_eq!(doc.save_aseprite().expect("save"), original);
}

#[test]
fn export_png_mutating_the_bytes_leaves_the_document_unchanged() {
    let doc = painted_doc();
    let original = doc.export_png(0).expect("png");

    let mut first = doc.export_png(0).expect("png");
    scribble(&mut first);

    assert_eq!(doc.export_png(0).expect("png"), original);
}

#[test]
fn atlas_export_png_mutating_a_read_leaves_later_reads_unchanged() {
    let doc = painted_doc();
    let atlas = doc.export_atlas(1, None, None).expect("atlas");
    let original = atlas.png();

    let mut first = atlas.png();
    scribble(&mut first);

    assert_eq!(atlas.png(), original);
    assert_eq!(
        doc.export_atlas(1, None, None).expect("atlas").png(),
        original
    );
}
