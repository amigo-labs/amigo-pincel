//! The live selection transform (docs/specs/image-transform-handles.md):
//! the core side of the Move tools' handles — begin, edit, commit, cancel,
//! undo while live, and the result depending only on the final settings.

use pincel_effects::transform::Quality;
use pincel_image_core::command::{CommandBus, SetLayerLocked, SetSelection};
use pincel_image_core::geometry::Rect;
use pincel_image_core::{
    Color, Document, DocumentError, ImageBuffer, SelectionMask, TransformSettings,
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
