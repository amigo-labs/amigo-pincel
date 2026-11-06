//! Layer ▸ Rotate / Zoom through the editor's effect entry points
//! (docs/specs/image-transform-handles-rotate-zoom.md AC-03, AC-05): one
//! undo step that Undo reverts, and no step when no pixel changes.

use super::*;

/// A 5 × 4 document whose layer has a distinct colour per pixel.
fn painted() -> u32 {
    let mut doc = Document::new(5, 4).expect("doc");
    let px = &mut doc.active_layer_mut().pixels;
    for y in 0..4 {
        for x in 0..5 {
            px.set_pixel(x, y, Color::rgba((x * 50) as u8, (y * 60) as u8, 7, 200));
        }
    }
    insert(CommandBus::new(doc))
}

fn pixels_and_depth(handle: u32) -> (Vec<u8>, usize) {
    with_bus(handle, |bus| {
        Ok((
            bus.document.layers()[0].pixels.data().to_vec(),
            bus.history.undo_depth(),
        ))
    })
    .expect("read")
}

/// Both commit paths: the job worker's region, and the direct apply.
fn apply_both_ways(params: &[f64]) -> [(u32, bool); 2] {
    let a = painted();
    let changed_a = apply_effect(a, 0, "rotate_zoom", params).expect("apply");
    let b = painted();
    let region = effect_region_bytes(b, 0, "rotate_zoom", params).expect("region");
    let changed_b = apply_effect_region(b, 0, "rotate_zoom", params, &region).expect("commit");
    [(a, changed_a), (b, changed_b)]
}

#[test]
fn apply_rotate_zoom_is_one_undo_step_that_undo_reverts() {
    let (before, depth) = pixels_and_depth(painted());
    for (handle, changed) in apply_both_ways(&[33.0, 10.0, -5.0, 1.3, 2.0]) {
        assert!(changed);
        let (after, after_depth) = pixels_and_depth(handle);
        assert_ne!(after, before);
        assert_eq!(after_depth, depth + 1, "exactly one undo step");
        assert!(undo(handle).expect("undo"));
        assert_eq!(pixels_and_depth(handle), (before.clone(), depth));
    }
}

#[test]
fn apply_rotate_zoom_identity_adds_no_undo_step() {
    let (before, depth) = pixels_and_depth(painted());
    for quality in [0.0, 1.0, 2.0] {
        for (handle, changed) in apply_both_ways(&[0.0, 0.0, 0.0, 1.0, quality]) {
            assert!(!changed, "quality {quality}");
            assert_eq!(pixels_and_depth(handle), (before.clone(), depth));
        }
    }
}

#[test]
fn apply_rotate_zoom_on_a_transparent_layer_adds_no_undo_step() {
    let handle = insert(CommandBus::new(Document::new(6, 6).expect("doc")));
    let before = pixels_and_depth(handle);
    assert!(!apply_effect(handle, 0, "rotate_zoom", &[45.0, 20.0, 0.0, 2.0, 1.0]).expect("apply"));
    assert_eq!(pixels_and_depth(handle), before);
}

#[test]
fn rotate_zoom_step_is_labelled_rotate_zoom() {
    use pincel_image_core::command::Command;
    let spec = EffectSpec::from_params("rotate_zoom", &[33.0, 10.0, -5.0, 1.3, 2.0]).expect("spec");
    let step = effect_step(
        0,
        Rect::new(0, 0, 1, 1),
        ImageBuffer::new_transparent(1, 1),
        &spec,
    );
    assert_eq!(step.label(), "Rotate / Zoom");
}
