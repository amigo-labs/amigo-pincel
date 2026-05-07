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

    for x in 0..3 {
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
