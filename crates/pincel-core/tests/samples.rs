//! The start screen's sprite sample (docs/specs/first-run-tutorial.md
//! AC-13, AC-16): 4 frames, a tag, 2 layers, a slice, at most 100 KB.

const SLIME: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../ui/public/samples/slime.aseprite"
));

#[test]
fn sprite_sample_has_frames_a_tag_layers_and_a_slice() {
    assert!(SLIME.len() <= 100 * 1024, "{} bytes", SLIME.len());
    let doc = pincel_core::read_aseprite(SLIME).expect("the sample reads");
    assert_eq!(doc.sprite.frames.len(), 4);
    assert!(!doc.sprite.tags.is_empty());
    assert_eq!(doc.sprite.layers.len(), 2);
    assert!(!doc.sprite.slices.is_empty());
}
