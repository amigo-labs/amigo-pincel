//! The live selection transform (docs/specs/image-transform-handles.md):
//! the core side of the Move tools' handles — begin, edit, commit, cancel,
//! undo while live, and the result depending only on the final settings.

use pincel_effects::transform::Quality;
use pincel_image_core::command::{CommandBus, SetLayerLocked, SetLayerOpacity, SetSelection};
use pincel_image_core::geometry::Rect;
use pincel_image_core::{
    Color, Document, DocumentError, ImageBuffer, SelectionMask, TransformSettings, compose,
};

const W: u32 = 16;
const H: u32 = 12;

/// A 16 × 12 layer: a 4 × 4 block of distinct opaque colours at (4, 3),
/// grey elsewhere; the block is selected.
fn bus() -> CommandBus {
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            if (4..8).contains(&x) && (3..7).contains(&y) {
                data.extend_from_slice(&[(x * 40) as u8, (y * 30) as u8, 200, 255]);
            } else {
                data.extend_from_slice(&[90, 90, 90, 255]);
            }
        }
    }
    let mut doc =
        Document::from_pixels(ImageBuffer::from_raw(W, H, data).expect("pixels")).expect("doc");
    doc.selection = Some(SelectionMask::rectangle(W, H, Rect::new(4, 3, 4, 4)));
    CommandBus::new(doc)
}

/// The settings that leave the selection where it is.
fn start(bus: &CommandBus) -> TransformSettings {
    TransformSettings::for_selection(&bus.document).expect("a selection")
}

fn moved(s: TransformSettings, dx: f64, dy: f64) -> TransformSettings {
    TransformSettings {
        center: (s.center.0 + dx, s.center.1 + dy),
        ..s
    }
}

fn pixels(bus: &CommandBus) -> Vec<u8> {
    bus.document.layers()[0].pixels.data().to_vec()
}

fn mask(bus: &CommandBus) -> Vec<u8> {
    bus.document
        .selection
        .as_ref()
        .map(|m| m.data().to_vec())
        .unwrap_or_default()
}

fn at(bus: &CommandBus, x: u32, y: u32) -> Color {
    bus.document.layers()[0]
        .pixels
        .get_pixel(x, y)
        .expect("pixel")
}

// AC-02, AC-07: a translate moves content and outline; the hole is
// transparent.
#[test]
fn begin_transform_translate_moves_pixels_and_outline() {
    let mut bus = bus();
    let block = at(&bus, 4, 3);
    let s = moved(start(&bus), 3.0, 2.0);
    assert!(bus.begin_transform(0, true, s, Some(1)).expect("begins"));
    assert_eq!(at(&bus, 7, 5), block);
    assert_eq!(at(&bus, 4, 3), Color::TRANSPARENT);
    let sel = bus.document.selection.as_ref().expect("selection");
    assert_eq!(sel.bounding_box(), Some(Rect::new(7, 5, 4, 4)));
    assert!(bus.live_transform().is_some());
}

// AC-09: the drags of one transform become one undo step; Undo and Redo
// restore and reapply the result.
#[test]
fn commit_transform_folds_its_steps_into_one() {
    let mut bus = bus();
    let (before_px, before_mask, depth) = (pixels(&bus), mask(&bus), bus.history.undo_depth());
    let s = start(&bus);
    bus.begin_transform(0, true, moved(s, 1.0, 0.0), Some(1))
        .expect("begin");
    bus.edit_transform(moved(s, 2.0, 1.0), Some(2))
        .expect("edit");
    bus.edit_transform(moved(s, 5.0, 2.0), Some(3))
        .expect("edit");
    assert_eq!(bus.history.undo_depth(), depth + 3);
    let (after_px, after_mask) = (pixels(&bus), mask(&bus));
    bus.commit_transform().expect("commit");
    assert!(bus.live_transform().is_none());
    assert_eq!(bus.history.undo_depth(), depth + 1);
    assert_eq!(
        (pixels(&bus), mask(&bus)),
        (after_px.clone(), after_mask.clone())
    );
    assert!(bus.undo().expect("undo"));
    assert_eq!((pixels(&bus), mask(&bus)), (before_px, before_mask));
    assert!(bus.redo().expect("redo"));
    assert_eq!((pixels(&bus), mask(&bus)), (after_px, after_mask));
}

// AC-10: Escape restores pixels, outline and the history depths.
#[test]
fn cancel_transform_restores_everything() {
    let mut bus = bus();
    let (px, m, undo, redo) = (
        pixels(&bus),
        mask(&bus),
        bus.history.undo_depth(),
        bus.history.redo_depth(),
    );
    let s = start(&bus);
    bus.begin_transform(0, true, moved(s, 4.0, 4.0), Some(1))
        .expect("begin");
    bus.edit_transform(TransformSettings { angle: 33.0, ..s }, Some(2))
        .expect("edit");
    bus.cancel_transform().expect("cancel");
    assert!(bus.live_transform().is_none());
    assert_eq!((pixels(&bus), mask(&bus)), (px, m));
    assert_eq!(
        (bus.history.undo_depth(), bus.history.redo_depth()),
        (undo, redo)
    );
}

// AC-11, AC-17: the result depends on the final settings only; a transform
// that ends where it started commits no step.
#[test]
fn round_trips_leave_no_loss_and_no_step() {
    for (there, quality) in [
        (TransformSettings::scaled_by, Quality::Bilinear),
        (TransformSettings::scaled_by, Quality::Bicubic),
    ] {
        let mut bus = bus();
        let (px, m, depth) = (pixels(&bus), mask(&bus), bus.history.undo_depth());
        let s = TransformSettings {
            quality,
            ..start(&bus)
        };
        bus.begin_transform(0, true, there(s, 0.25, 0.25), Some(1))
            .expect("begin");
        bus.edit_transform(s, Some(2)).expect("back");
        bus.commit_transform().expect("commit");
        assert_eq!((pixels(&bus), mask(&bus)), (px.clone(), m.clone()));
        assert_eq!(bus.history.undo_depth(), depth);
    }
    let mut bus = bus();
    let (px, depth) = (pixels(&bus), bus.history.undo_depth());
    let s = start(&bus);
    bus.begin_transform(0, true, TransformSettings { angle: 30.0, ..s }, Some(1))
        .expect("+30");
    bus.edit_transform(TransformSettings { angle: -30.0, ..s }, Some(2))
        .expect("-30");
    bus.edit_transform(s, Some(3)).expect("0");
    bus.commit_transform().expect("commit");
    assert_eq!(pixels(&bus), px);
    assert_eq!(bus.history.undo_depth(), depth);
}

// AC-13: content dragged past the canvas edge comes back unchanged.
#[test]
fn content_beyond_the_canvas_returns_unchanged() {
    let mut bus = bus();
    let s = start(&bus);
    bus.begin_transform(0, true, moved(s, 3.0, 0.0), Some(1))
        .expect("begin");
    let shown = pixels(&bus);
    bus.edit_transform(moved(s, 30.0, 0.0), Some(2))
        .expect("off");
    bus.edit_transform(moved(s, 3.0, 0.0), Some(3))
        .expect("back");
    assert_eq!(pixels(&bus), shown);
}

// AC-15: Move Selection changes only the outline.
#[test]
fn move_selection_transform_changes_only_the_outline() {
    let mut bus = bus();
    let px = pixels(&bus);
    let s = start(&bus);
    bus.begin_transform(
        0,
        false,
        TransformSettings {
            angle: 45.0,
            ..moved(s, 2.0, 1.0)
        },
        Some(1),
    )
    .expect("begin");
    bus.commit_transform().expect("commit");
    assert_eq!(pixels(&bus), px);
    assert_ne!(
        bus.document
            .selection
            .as_ref()
            .and_then(|m| m.bounding_box()),
        Some(Rect::new(4, 3, 4, 4))
    );
}

// AC-16: a locked layer refuses Move Selected Pixels, not Move Selection.
#[test]
fn locked_layer_refuses_pixels_but_not_the_outline() {
    let mut bus = bus();
    bus.apply(Box::new(SetLayerLocked::new(0, true)))
        .expect("lock");
    let s = moved(start(&bus), 1.0, 0.0);
    assert!(matches!(
        bus.begin_transform(0, true, s, Some(1)),
        Err(DocumentError::LayerLocked { .. })
    ));
    assert!(bus.begin_transform(0, false, s, Some(1)).expect("outline"));
}

// AC-17: an empty selection starts nothing.
#[test]
fn empty_selection_starts_nothing() {
    let mut bus = bus();
    bus.document.selection = Some(SelectionMask::new_empty(W, H));
    assert!(TransformSettings::for_selection(&bus.document).is_none());
    let s = TransformSettings::identity_at((5.0, 5.0), (2.0, 2.0));
    assert!(!bus.begin_transform(0, true, s, Some(1)).expect("nothing"));
    assert!(bus.live_transform().is_none());
}

// AC-21: Undo while live undoes one step and keeps the transform live.
#[test]
fn undo_while_live_steps_back_one_drag() {
    let mut bus = bus();
    let (px, m) = (pixels(&bus), mask(&bus));
    let s = start(&bus);
    let first = moved(s, 2.0, 0.0);
    bus.begin_transform(0, true, first, Some(1)).expect("begin");
    let after_first = pixels(&bus);
    bus.edit_transform(moved(s, 5.0, 3.0), Some(2))
        .expect("edit");
    assert!(bus.undo().expect("undo"));
    assert_eq!(bus.live_transform().map(|t| t.settings), Some(first));
    assert_eq!(pixels(&bus), after_first);
    assert!(bus.undo().expect("undo"));
    assert!(bus.live_transform().is_none());
    assert_eq!((pixels(&bus), mask(&bus)), (px, m));
    assert!(bus.redo().expect("redo"));
    assert_eq!(bus.live_transform().map(|t| t.settings), Some(first));
}

// Edge case: a 90° turn with Nearest equals the patch turned clockwise.
#[test]
fn nearest_quarter_turn_matches_rotate_90() {
    let mut bus = bus();
    let block = bus.document.layers()[0]
        .pixels
        .copy_region(Rect::new(4, 3, 4, 4))
        .expect("block");
    let s = TransformSettings {
        angle: 90.0,
        quality: Quality::Nearest,
        ..start(&bus)
    };
    bus.begin_transform(0, true, s, Some(1)).expect("begin");
    let turned = bus.document.layers()[0]
        .pixels
        .copy_region(Rect::new(4, 3, 4, 4))
        .expect("turned");
    let expected = pincel_image_core::transform::rotate_90_cw(&block);
    assert_eq!(turned.data(), expected.data());
}

// AC-22: more drags than the history holds, then Escape: as before.
#[test]
fn capacity_folds_live_steps_and_cancel_still_restores() {
    let mut bus = bus();
    let (px, m) = (pixels(&bus), mask(&bus));
    let s = start(&bus);
    bus.begin_transform(0, true, moved(s, 1.0, 0.0), Some(0))
        .expect("begin");
    for i in 1..150u64 {
        bus.edit_transform(moved(s, (i % 7) as f64, (i % 5) as f64), Some(i))
            .expect("edit");
    }
    bus.cancel_transform().expect("cancel");
    assert_eq!((pixels(&bus), mask(&bus)), (px, m));
}

// AC-08: any other command commits the transform first.
#[test]
fn another_command_commits_first() {
    let mut bus = bus();
    let depth = bus.history.undo_depth();
    let s = start(&bus);
    bus.begin_transform(0, true, moved(s, 3.0, 0.0), Some(1))
        .expect("begin");
    bus.edit_transform(moved(s, 4.0, 0.0), Some(2))
        .expect("edit");
    let moved_px = pixels(&bus);
    bus.apply(Box::new(SetSelection::clear()))
        .expect("deselect");
    assert!(bus.live_transform().is_none());
    assert_eq!(bus.history.undo_depth(), depth + 2);
    assert_eq!(pixels(&bus), moved_px);
}

// AC-08: a change of the active layer commits the transform on its own
// layer first; picking the same layer again keeps it live.
#[test]
fn set_active_layer_commits_a_live_transform_first() {
    let mut bus = bus();
    bus.document.add_layer("Top").expect("layer");
    bus.set_active_layer(0).expect("active");
    let depth = bus.history.undo_depth();
    let top = bus.document.layers()[1].pixels.data().to_vec();
    let s = start(&bus);
    bus.begin_transform(0, true, moved(s, 3.0, 0.0), Some(1))
        .expect("begin");
    bus.edit_transform(moved(s, 4.0, 1.0), Some(2))
        .expect("edit");
    let (moved_px, moved_mask) = (pixels(&bus), mask(&bus));
    bus.set_active_layer(0).expect("same layer");
    assert!(bus.live_transform().is_some());
    bus.set_active_layer(1).expect("other layer");
    assert!(bus.live_transform().is_none());
    assert_eq!(bus.document.active_layer_index(), 1);
    assert_eq!(bus.history.undo_depth(), depth + 1);
    assert_eq!((pixels(&bus), mask(&bus)), (moved_px, moved_mask));
    assert_eq!(bus.document.layers()[1].pixels.data(), &top[..]);
}

// AC-08: beginning another live object commits the transform first, and the
// object draws on the committed result.
#[test]
fn begin_live_commits_a_live_transform_first() {
    use pincel_image_core::{Gradient, LiveGradient, LiveParams, Point};
    let mut bus = bus();
    let depth = bus.history.undo_depth();
    bus.begin_transform(0, true, moved(start(&bus), 3.0, 2.0), Some(1))
        .expect("begin");
    let (moved_px, moved_mask) = (pixels(&bus), mask(&bus));
    let gradient = LiveParams::Gradient(LiveGradient {
        gradient: Gradient::default(),
        a: Point::new(0.0, 0.0),
        b: Point::new(15.0, 0.0),
    });
    assert!(bus.begin_live(0, gradient).expect("live object"));
    assert!(bus.live_transform().is_none());
    assert!(bus.live_object().is_some());
    assert_eq!(bus.history.undo_depth(), depth + 2);
    // Undoing the object leaves the committed transform, not a live one.
    assert!(bus.undo().expect("undo"));
    assert!(bus.live_object().is_none());
    assert!(bus.live_transform().is_none());
    assert_eq!((pixels(&bus), mask(&bus)), (moved_px, moved_mask));
}

// AC-07: consecutive quality changes form one step.
#[test]
fn consecutive_quality_changes_merge() {
    let mut bus = bus();
    let depth = bus.history.undo_depth();
    let s = TransformSettings {
        angle: 20.0,
        ..start(&bus)
    };
    bus.begin_transform(0, true, s, Some(1)).expect("begin");
    for quality in [Quality::Nearest, Quality::Bicubic, Quality::Bilinear] {
        bus.edit_transform(TransformSettings { quality, ..s }, None)
            .expect("quality");
    }
    assert_eq!(bus.history.undo_depth(), depth + 2);
}

// The editor's wire form: begin, edit and commit through `CommandSpec`,
// and the locked-layer gate (AC-16) refuses only the pixel transform.
#[test]
fn command_spec_drives_the_transform() {
    use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
    let mut bus = bus();
    let depth = bus.history.undo_depth();
    let run = |bus: &mut CommandBus, json: &str| {
        let spec: CommandSpec = serde_json::from_str(json).expect("spec");
        run_command(bus, spec, &mut StrokeScratch::default(), &[]).expect("runs");
    };
    run(
        &mut bus,
        r#"{"type":"begin_transform","layer":0,"pixels":true,"center_x":9,"center_y":5,
            "width":4,"height":4,"angle":0,"pivot_x":0.5,"pivot_y":0.5,"quality":1,"gesture":1}"#,
    );
    let live = pincel_image_core::command::live_spec::live_transform_spec(&bus).expect("live");
    assert_eq!(live.bounds, [4, 3, 4, 4]);
    assert_eq!(live.center, [9.0, 5.0]);
    run(
        &mut bus,
        r#"{"type":"edit_transform","center_x":10,"center_y":6,"width":4,"height":4,
            "angle":0,"pivot_x":0.5,"pivot_y":0.5,"quality":0,"gesture":2}"#,
    );
    run(&mut bus, r#"{"type":"commit_transform"}"#);
    assert!(bus.live_transform().is_none());
    assert_eq!(bus.history.undo_depth(), depth + 1);

    let locked: CommandSpec = serde_json::from_str(
        r#"{"type":"begin_transform","layer":0,"pixels":true,"center_x":9,"center_y":5,
            "width":4,"height":4,"angle":0,"pivot_x":0.5,"pivot_y":0.5,"quality":1}"#,
    )
    .expect("spec");
    let outline: CommandSpec = serde_json::from_str(
        r#"{"type":"begin_transform","layer":0,"pixels":false,"center_x":9,"center_y":5,
            "width":4,"height":4,"angle":0,"pivot_x":0.5,"pivot_y":0.5,"quality":1}"#,
    )
    .expect("spec");
    bus.apply(Box::new(SetLayerLocked::new(0, true)))
        .expect("lock");
    assert!(locked.refused(&bus.document));
    assert!(!outline.refused(&bus.document));
}

// AC-12: with Nearest and a hard-edged selection, every committed pixel is
// a lifted colour, an untouched layer colour, or transparent.
#[test]
fn nearest_uses_only_existing_colours() {
    let mut bus = bus();
    let mut allowed: std::collections::HashSet<[u8; 4]> = bus.document.layers()[0]
        .pixels
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .collect();
    allowed.insert([0, 0, 0, 0]);
    let s = TransformSettings {
        angle: 37.0,
        quality: Quality::Nearest,
        ..start(&bus).scaled_by(1.7, 0.6)
    };
    bus.begin_transform(0, true, s, Some(1)).expect("begin");
    bus.commit_transform().expect("commit");
    for c in bus.document.layers()[0].pixels.data().as_chunks::<4>().0 {
        assert!(allowed.contains(c), "{c:?}");
    }
}

// AC-14: the outline follows the committed content, so a second transform
// carries the moved patch on.
#[test]
fn a_second_transform_carries_the_moved_patch() {
    let mut bus = bus();
    let block = at(&bus, 4, 3);
    bus.begin_transform(0, true, moved(start(&bus), 3.0, 0.0), Some(1))
        .expect("first");
    bus.commit_transform().expect("commit");
    assert_eq!(at(&bus, 7, 3), block);
    bus.begin_transform(0, true, moved(start(&bus), 0.0, 4.0), Some(2))
        .expect("second");
    bus.commit_transform().expect("commit");
    assert_eq!(at(&bus, 7, 7), block);
    assert_eq!(at(&bus, 7, 3), Color::TRANSPARENT);
}

// AC-19: live and committed steps carry the Move tools' history kinds.
#[test]
fn steps_carry_the_move_history_kinds() {
    use pincel_image_core::command::HistoryKind;
    let mut bus = bus();
    let s = start(&bus);
    bus.begin_transform(0, true, moved(s, 1.0, 0.0), Some(1))
        .expect("begin");
    assert_eq!(
        bus.history.undo_kinds().last(),
        Some(HistoryKind::MoveSelection)
    );
    bus.commit_transform().expect("commit");
    assert_eq!(
        bus.history.undo_kinds().last(),
        Some(HistoryKind::MoveSelection)
    );
    bus.begin_transform(0, false, moved(start(&bus), 1.0, 0.0), Some(2))
        .expect("outline");
    bus.commit_transform().expect("commit");
    assert_eq!(
        bus.history.undo_kinds().last(),
        Some(HistoryKind::MoveSelectionMask)
    );
}

// AC-18: a snapshot or project saved while live holds the applied pixels
// and outline and no transform; saving changes no history.
#[test]
fn saving_while_live_keeps_the_transform_and_writes_plain_pixels() {
    use pincel_image_core::codec::snapshot::{from_snapshot_bytes, to_snapshot_bytes};
    let mut bus = bus();
    bus.begin_transform(0, true, moved(start(&bus), 3.0, 2.0), Some(1))
        .expect("begin");
    let depth = bus.history.undo_depth();
    let snapshot = to_snapshot_bytes(&bus.document);
    assert!(bus.live_transform().is_some());
    assert_eq!(bus.history.undo_depth(), depth);
    let back = from_snapshot_bytes(&snapshot).expect("snapshot");
    assert_eq!(
        back.layers()[0].pixels.data(),
        bus.document.layers()[0].pixels.data()
    );
    assert_eq!(back.selection, bus.document.selection);
    assert!(CommandBus::new(back).live_transform().is_none());
}

// AC-18: a project saved while live holds the applied pixels, reopens with
// no live transform, and the save neither commits nor touches the history.
#[test]
fn ora_saved_while_live_reopens_with_plain_pixels() {
    use pincel_image_core::codec::ora::{from_ora_bytes, to_ora_bytes};
    let mut bus = bus();
    let s = TransformSettings {
        angle: 30.0,
        ..moved(start(&bus), 2.0, 1.0)
    };
    bus.begin_transform(0, true, s, Some(1)).expect("begin");
    let depths = (bus.history.undo_depth(), bus.history.redo_depth());
    let shown = compose(bus.document.layers());
    let ora = to_ora_bytes(&bus.document, 1).expect("save");
    assert!(bus.live_transform().is_some());
    assert_eq!((bus.history.undo_depth(), bus.history.redo_depth()), depths);
    let reopened = CommandBus::new(from_ora_bytes(&ora).expect("open"));
    assert!(reopened.live_transform().is_none());
    assert_eq!(reopened.history.undo_depth(), 0);
    assert_eq!(
        reopened.document.layers()[0].pixels.data(),
        &pixels(&bus)[..]
    );
    assert_eq!(compose(reopened.document.layers()), shown);
}

/// The composite's pixel at `(x, y)`.
fn shown(doc: &Document, x: u32, y: u32) -> Color {
    compose(doc.layers()).get_pixel(x, y).expect("pixel")
}

// AC-23: a drag's preview gets the lifted patch and the document with the
// hole the lift leaves; the layer, outline and history stay as they were.
#[test]
fn with_transform_lifted_shows_the_hole_and_puts_the_layer_back() {
    let mut bus = bus();
    let block = at(&bus, 4, 3);
    let (px, m) = (pixels(&bus), mask(&bus));
    bus.take_damage();
    let (lift, (hole, grey, damage)) = bus
        .with_transform_lifted(0, true, |doc, damage| {
            (shown(doc, 4, 3), shown(doc, 0, 0), damage)
        })
        .expect("lifts")
        .expect("a selection");
    assert_eq!(hole, Color::TRANSPARENT);
    assert_eq!(grey, Color::rgba(90, 90, 90, 255));
    assert!(!damage.is_none(), "the hole is damage for the preview");
    assert_eq!(lift.bounds, Rect::new(4, 3, 4, 4));
    assert_eq!(lift.opacity, 1.0);
    let patch = lift.patch.expect("Move Selected Pixels lifts pixels");
    assert_eq!((patch.width(), patch.height()), (4, 4));
    assert_eq!(&patch.data()[..4], &[block.r, block.g, block.b, block.a]);
    assert_eq!((pixels(&bus), mask(&bus)), (px, m));
    assert_eq!(bus.history.undo_depth(), 0);
    assert!(bus.live_transform().is_none());
    assert!(!bus.take_damage().is_none(), "the restored rect is damage");
}

// AC-23: with a transform live, the preview hides the moved content too and
// hands over the original patch; the transform stays as it was.
#[test]
fn with_transform_lifted_hides_a_live_transforms_content() {
    let mut bus = bus();
    let block = at(&bus, 4, 3);
    let s = moved(start(&bus), 3.0, 2.0);
    bus.begin_transform(0, true, s, Some(1)).expect("begin");
    let (px, m) = (pixels(&bus), mask(&bus));
    let (lift, (moved_away, hole)) = bus
        .with_transform_lifted(0, true, |doc, _| (shown(doc, 7, 5), shown(doc, 4, 3)))
        .expect("lifts")
        .expect("live");
    assert_eq!((moved_away, hole), (Color::TRANSPARENT, Color::TRANSPARENT));
    assert_eq!(lift.bounds, Rect::new(4, 3, 4, 4));
    let patch = lift.patch.expect("pixels");
    assert_eq!(&patch.data()[..4], &[block.r, block.g, block.b, block.a]);
    assert_eq!((pixels(&bus), mask(&bus)), (px, m));
    assert_eq!(bus.history.undo_depth(), 1);
    assert_eq!(bus.live_transform().map(|t| t.settings), Some(s));
}

// AC-16, AC-17, AC-23: a locked layer refuses a pixel preview, not an
// outline one; no selection lifts nothing; the layer's opacity comes along.
#[test]
fn with_transform_lifted_refuses_a_locked_layer_and_needs_a_selection() {
    let mut bus = bus();
    bus.apply(Box::new(SetLayerOpacity::new(0, 0.5)))
        .expect("opacity");
    let lift = bus
        .with_transform_lifted(0, true, |_, _| ())
        .expect("lifts")
        .expect("a selection");
    assert_eq!(lift.0.opacity, 0.5);
    bus.apply(Box::new(SetLayerLocked::new(0, true)))
        .expect("lock");
    assert!(matches!(
        bus.with_transform_lifted(0, true, |_, _| ()),
        Err(DocumentError::LayerLocked { .. })
    ));
    let (outline, ()) = bus
        .with_transform_lifted(0, false, |_, _| ())
        .expect("outline only")
        .expect("a selection");
    assert!(outline.patch.is_none());
    bus.document.selection = Some(SelectionMask::new_empty(W, H));
    assert!(
        bus.with_transform_lifted(0, false, |_, _| ())
            .expect("nothing")
            .is_none()
    );
}
