//! End-to-end Aseprite round-trip exercised through the wasm surface.
//!
//! Closes the programmatic gap behind CLAUDE.md M6.7: we paint a few
//! recognisable pixels through `Document::apply_tool`, serialise the
//! result with `saveAseprite`, parse the bytes back with
//! `openAseprite`, and assert the painted pixels survive in the
//! composed RGBA frame. The browser demo + upstream-Aseprite
//! validation that completes M6.7 still requires a human, but this
//! test pins the byte-level promise the demo relies on.

use pincel_wasm::Document;

const W: u32 = 8;
const H: u32 = 8;
const RED: u32 = 0xff0000ff;
const GREEN: u32 = 0x00ff00ff;
const BLUE: u32 = 0x0000ffff;

fn pixel_at(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let off = ((y * width + x) * 4) as usize;
    [
        pixels[off],
        pixels[off + 1],
        pixels[off + 2],
        pixels[off + 3],
    ]
}

#[test]
fn paint_save_open_roundtrip_preserves_pixels() {
    let mut doc = Document::new(W, H).expect("dims");
    doc.apply_tool("pencil", 0, 0, RED).expect("paint red");
    doc.apply_tool("pencil", 3, 4, GREEN).expect("paint green");
    doc.apply_tool("pencil", 7, 7, BLUE).expect("paint blue");

    let bytes = doc.save_aseprite().expect("save ok");
    let reopened = Document::open_aseprite(&bytes).expect("open ok");

    assert_eq!(reopened.width(), W);
    assert_eq!(reopened.height(), H);
    assert_eq!(reopened.layer_count(), 1);
    assert_eq!(reopened.frame_count(), 1);

    let frame = reopened.compose(0, 1).expect("compose ok");
    assert_eq!(frame.width(), W);
    assert_eq!(frame.height(), H);
    let pixels = frame.pixels();

    assert_eq!(pixel_at(&pixels, W, 0, 0), [255, 0, 0, 255]);
    assert_eq!(pixel_at(&pixels, W, 3, 4), [0, 255, 0, 255]);
    assert_eq!(pixel_at(&pixels, W, 7, 7), [0, 0, 255, 255]);
    // Spot-check a few unpainted pixels stay fully transparent.
    assert_eq!(pixel_at(&pixels, W, 1, 0), [0, 0, 0, 0]);
    assert_eq!(pixel_at(&pixels, W, 0, 7), [0, 0, 0, 0]);
}

#[test]
fn paint_save_open_roundtrip_preserves_undo_target_state() {
    // After the round-trip the reopened document is a fresh editing
    // session: it has no undo history (the file format does not
    // carry one), but the painted bytes are the new baseline. A
    // follow-up paint must apply on top of them.
    let mut doc = Document::new(W, H).expect("dims");
    doc.apply_tool("pencil", 2, 2, RED).expect("paint red");

    let bytes = doc.save_aseprite().expect("save ok");
    let mut reopened = Document::open_aseprite(&bytes).expect("open ok");

    assert_eq!(reopened.undo_depth(), 0);
    assert_eq!(reopened.redo_depth(), 0);

    reopened
        .apply_tool("pencil", 5, 5, GREEN)
        .expect("paint green on reopened doc");

    let frame = reopened.compose(0, 1).expect("compose ok");
    let pixels = frame.pixels();
    assert_eq!(pixel_at(&pixels, W, 2, 2), [255, 0, 0, 255]);
    assert_eq!(pixel_at(&pixels, W, 5, 5), [0, 255, 0, 255]);
}
