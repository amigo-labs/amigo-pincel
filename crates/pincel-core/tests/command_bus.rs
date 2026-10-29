//! Integration tests for the command bus and the M2 commands. See
//! `docs/specs/pincel.md` §6.

use pincel_core::{
    AddFrame, AddLayer, AnyCommand, Bus, Cel, CelMap, ColorMode, Frame, FrameIndex, Layer, LayerId,
    PixelBuffer, Rgba, SetPixel, Sprite,
};

fn doc_with_layer_and_frame() -> (Sprite, CelMap) {
    let sprite = Sprite::builder(4, 4)
        .add_layer(Layer::image(LayerId::new(1), "bg"))
        .add_frame(Frame::new(100))
        .build()
        .expect("sprite builds");
    let mut cels = CelMap::new();
    cels.insert(Cel::image(
        LayerId::new(1),
        FrameIndex::new(0),
        PixelBuffer::empty(4, 4, ColorMode::Rgba),
    ));
    (sprite, cels)
}

#[test]
fn set_pixel_undo_restores_buffer() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    let cmd = SetPixel::new(LayerId::new(1), FrameIndex::new(0), 1, 2, Rgba::WHITE);
    bus.execute(AnyCommand::SetPixel(cmd), &mut sprite, &mut cels)
        .expect("execute");

    assert_eq!(bus.undo_depth(), 1);
    assert!(bus.undo(&mut sprite, &mut cels));
    assert_eq!(bus.undo_depth(), 0);
    assert_eq!(bus.redo_depth(), 1);

    let cel = cels
        .get(LayerId::new(1), FrameIndex::new(0))
        .expect("cel still exists");
    let pincel_core::CelData::Image(buf) = &cel.data else {
        panic!("expected image cel");
    };
    assert!(buf.data.iter().all(|&b| b == 0));
}

#[test]
fn redo_replays_undone_command() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 0, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("execute");

    assert!(bus.undo(&mut sprite, &mut cels));
    assert!(bus.redo(&mut sprite, &mut cels).expect("redo ok"));

    let cel = cels.get(LayerId::new(1), FrameIndex::new(0)).unwrap();
    let pincel_core::CelData::Image(buf) = &cel.data else {
        unreachable!()
    };
    assert_eq!(&buf.data[..4], &[255, 255, 255, 255]);
}

#[test]
fn execute_clears_redo_stack() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 0, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("execute 1");
    bus.undo(&mut sprite, &mut cels);
    assert_eq!(bus.redo_depth(), 1);

    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 1, 1, Rgba::BLACK).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("execute 2");
    assert_eq!(bus.redo_depth(), 0);
}

#[test]
fn add_layer_and_undo_restores_layer_count() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    bus.execute(
        AddLayer::on_top(Layer::image(LayerId::new(2), "fg")).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("execute");
    assert_eq!(sprite.layers.len(), 2);
    bus.undo(&mut sprite, &mut cels);
    assert_eq!(sprite.layers.len(), 1);
    bus.redo(&mut sprite, &mut cels).expect("redo");
    assert_eq!(sprite.layers.len(), 2);
    assert_eq!(sprite.layers[1].id, LayerId::new(2));
}

#[test]
fn add_frame_and_undo_restores_frame_count() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    bus.execute(
        AddFrame::append(Frame::new(40)).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("execute");
    assert_eq!(sprite.frames.len(), 2);
    assert_eq!(sprite.frames[1].duration_ms, 40);

    bus.undo(&mut sprite, &mut cels);
    assert_eq!(sprite.frames.len(), 1);
}

#[test]
fn history_cap_drops_oldest_entries() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::with_capacity(2);

    // Seal between executes so each SetPixel stays its own entry (they
    // would otherwise merge into one stroke).
    for x in 0..3 {
        bus.seal();
        bus.execute(
            SetPixel::new(LayerId::new(1), FrameIndex::new(0), x, 0, Rgba::WHITE).into(),
            &mut sprite,
            &mut cels,
        )
        .expect("execute");
    }
    assert_eq!(bus.undo_depth(), 2);

    // Two undos should leave the bus empty.
    assert!(bus.undo(&mut sprite, &mut cels));
    assert!(bus.undo(&mut sprite, &mut cels));
    assert!(!bus.undo(&mut sprite, &mut cels));
}

#[test]
fn execute_merges_consecutive_set_pixels_into_one_undo_entry() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    for x in 0..4 {
        bus.execute(
            SetPixel::new(LayerId::new(1), FrameIndex::new(0), x, 0, Rgba::WHITE).into(),
            &mut sprite,
            &mut cels,
        )
        .expect("execute");
    }
    assert_eq!(bus.undo_depth(), 1, "one stroke, one entry");

    // Undoing the single entry restores every pixel of the stroke.
    assert!(bus.undo(&mut sprite, &mut cels));
    let cel = cels.get(LayerId::new(1), FrameIndex::new(0)).unwrap();
    let pincel_core::CelData::Image(buf) = &cel.data else {
        unreachable!()
    };
    assert!(buf.data.iter().all(|&b| b == 0), "stroke fully undone");
}

#[test]
fn seal_prevents_merge_across_strokes() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 0, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("stroke 1");
    bus.seal();
    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 1, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("stroke 2");
    assert_eq!(bus.undo_depth(), 2, "sealed strokes stay separate");
}

#[test]
fn undo_seals_history_against_merging() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 0, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("stroke 1");
    bus.seal();
    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 1, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("stroke 2");
    assert!(bus.undo(&mut sprite, &mut cels));
    assert!(bus.redo(&mut sprite, &mut cels).expect("redo"));
    // A paint right after undo/redo must not coalesce into the restored
    // stroke-2 entry.
    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 2, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("stroke 3");
    assert_eq!(bus.undo_depth(), 3);
}

#[test]
fn failed_execute_adds_no_entry_and_keeps_redo_stack() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();

    for x in 0..2 {
        bus.seal();
        bus.execute(
            SetPixel::new(LayerId::new(1), FrameIndex::new(0), x, 0, Rgba::WHITE).into(),
            &mut sprite,
            &mut cels,
        )
        .expect("execute");
    }
    assert!(bus.undo(&mut sprite, &mut cels));
    assert_eq!((bus.undo_depth(), bus.redo_depth()), (1, 1));

    // A pixel outside the 4x4 canvas.
    let outside = bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 9, 9, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    );
    assert!(outside.is_err(), "out-of-canvas write fails");
    // A write to a layer that does not exist.
    let unknown = bus.execute(
        SetPixel::new(LayerId::new(42), FrameIndex::new(0), 0, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    );
    assert!(unknown.is_err(), "unknown-layer write fails");

    assert_eq!(bus.undo_depth(), 1, "failed edits add no entry");
    assert_eq!(bus.redo_depth(), 1, "failed edits keep the redo history");
    assert!(bus.redo(&mut sprite, &mut cels).expect("redo ok"));
    assert_eq!(bus.undo_depth(), 2);
}

/// Paints stroke 1 at (0, 0), seals, then stroke 2 at (1, 0). Stroke 2 is
/// left unsealed so only `undo` / `redo` can keep the next edit apart.
fn two_sealed_strokes(bus: &mut Bus, sprite: &mut Sprite, cels: &mut CelMap) {
    for x in 0..2 {
        if x > 0 {
            bus.seal();
        }
        bus.execute(
            SetPixel::new(LayerId::new(1), FrameIndex::new(0), x, 0, Rgba::WHITE).into(),
            sprite,
            cels,
        )
        .expect("stroke");
    }
}

fn alpha_at(cels: &CelMap, x: usize) -> u8 {
    let cel = cels.get(LayerId::new(1), FrameIndex::new(0)).unwrap();
    let pincel_core::CelData::Image(buf) = &cel.data else {
        unreachable!()
    };
    buf.data[x * 4 + 3]
}

#[test]
fn edit_after_undo_does_not_merge_into_the_restored_top() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();
    two_sealed_strokes(&mut bus, &mut sprite, &mut cels);
    assert!(bus.undo(&mut sprite, &mut cels));

    // Stroke 1 is the top again; the next paint must not join it.
    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 2, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("edit");
    assert_eq!(bus.undo_depth(), 2, "edit is its own entry");
    assert!(bus.undo(&mut sprite, &mut cels));
    assert_eq!(alpha_at(&cels, 2), 0, "edit undone");
    assert_eq!(alpha_at(&cels, 0), 255, "stroke 1 untouched");
}

#[test]
fn edit_after_redo_does_not_merge_into_the_restored_entry() {
    let (mut sprite, mut cels) = doc_with_layer_and_frame();
    let mut bus = Bus::new();
    two_sealed_strokes(&mut bus, &mut sprite, &mut cels);
    assert!(bus.undo(&mut sprite, &mut cels));
    assert!(bus.redo(&mut sprite, &mut cels).expect("redo ok"));

    // Stroke 2 was just restored; the next paint must not join it.
    bus.execute(
        SetPixel::new(LayerId::new(1), FrameIndex::new(0), 2, 0, Rgba::WHITE).into(),
        &mut sprite,
        &mut cels,
    )
    .expect("edit");
    assert_eq!(bus.undo_depth(), 3, "edit is its own entry");
    assert!(bus.undo(&mut sprite, &mut cels));
    assert_eq!(alpha_at(&cels, 2), 0, "edit undone");
    assert_eq!(alpha_at(&cels, 1), 255, "restored stroke 2 untouched");
}
