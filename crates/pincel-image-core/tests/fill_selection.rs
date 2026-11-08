//! Edit ▸ Fill Selection through the editor's command path (`run_command`
//! with `fill_selection`): undo, no-op and lock rules, and that it touches
//! only the target layer (docs/specs/image-edit-extras.md AC-05 – AC-09).

use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{Color, CommandBus, Document, Rect, SelectionMask, SetPixels};

const FG: [u8; 4] = [10, 200, 30, 255];

/// Two layers with a distinct pattern each; layer 1 is the fill target.
fn bus() -> CommandBus {
    let mut bus = CommandBus::new(Document::new(6, 5).expect("doc"));
    run(&mut bus, CommandSpec::AddLayer { active: 0 });
    for (layer, base) in [(0usize, 40u32), (1, 90)] {
        let mut patch = bus.document.layers()[layer].pixels.clone();
        for y in 0..5 {
            for x in 0..6 {
                let v = (base + x * 13 + y * 7) as u8;
                patch.set_pixel(x, y, Color::rgba(v, 255 - v, v / 2, 255));
            }
        }
        bus.apply(Box::new(SetPixels::new(
            layer,
            Rect::new(0, 0, 6, 5),
            patch,
        )))
        .expect("pattern");
    }
    bus.history.clear();
    bus
}

fn run(bus: &mut CommandBus, spec: CommandSpec) {
    run_command(bus, spec, &mut StrokeScratch::default(), &[]).expect("runs");
}

fn fill(layer: usize, color: [u8; 4]) -> CommandSpec {
    CommandSpec::FillSelection { layer, color }
}

fn feathered(w: u32, h: u32) -> SelectionMask {
    let mut mask = SelectionMask::rectangle(w, h, Rect::new(1, 1, 3, 2));
    mask.set(4, 1, 100);
    mask
}

#[test]
fn fill_selection_is_one_undo_step_that_undoes_and_redoes_and_keeps_the_selection() {
    let mut bus = bus();
    bus.document.selection = Some(feathered(6, 5));
    let before = bus.document.clone();
    run(&mut bus, fill(1, FG));
    assert_eq!(bus.history.undo_depth(), 1, "one undo step");
    let filled = bus.document.clone();
    assert_ne!(filled.layers()[1].pixels, before.layers()[1].pixels);
    assert_eq!(filled.selection, before.selection, "selection unchanged");
    assert!(bus.undo().expect("undo"));
    assert_eq!(bus.document.layers()[1].pixels, before.layers()[1].pixels);
    assert_eq!(bus.document.selection, before.selection);
    assert!(bus.redo().expect("redo"));
    assert_eq!(bus.document.layers()[1].pixels, filled.layers()[1].pixels);
    assert_eq!(bus.document.selection, before.selection);
}

#[test]
fn fill_selection_that_changes_nothing_adds_no_undo_step() {
    let mut bus = bus();
    bus.document.selection = Some(SelectionMask::rectangle(6, 5, Rect::new(0, 0, 2, 2)));
    run(&mut bus, fill(1, FG));
    assert_eq!(bus.history.undo_depth(), 1);
    let after = bus.document.clone();
    run(&mut bus, fill(1, FG));
    assert_eq!(bus.history.undo_depth(), 1, "second fill adds nothing");
    assert_eq!(bus.document, after);
}

#[test]
fn fill_selection_with_a_selection_that_covers_nothing_changes_nothing() {
    let mut bus = bus();
    bus.document.selection = Some(SelectionMask::new_empty(6, 5));
    let before = bus.document.clone();
    run(&mut bus, fill(1, FG));
    assert_eq!(bus.history.undo_depth(), 0);
    assert_eq!(bus.document, before);
}

#[test]
fn fill_selection_on_a_locked_layer_is_refused() {
    let mut bus = bus();
    run(
        &mut bus,
        CommandSpec::SetLayerLocked {
            index: 1,
            locked: true,
        },
    );
    assert!(fill(1, FG).refused(&bus.document));
    assert!(!fill(0, FG).refused(&bus.document));
}

#[test]
fn fill_selection_changes_only_the_target_layer_also_when_hidden_and_translucent() {
    for selected in [false, true] {
        let mut bus = bus();
        run(
            &mut bus,
            CommandSpec::SetLayerVisible {
                index: 1,
                visible: false,
            },
        );
        run(
            &mut bus,
            CommandSpec::SetLayerOpacity {
                index: 1,
                opacity: 0.4,
            },
        );
        if selected {
            bus.document.selection = Some(feathered(6, 5));
        }
        let before = bus.document.clone();
        run(&mut bus, fill(1, FG));
        let doc = &bus.document;
        assert_ne!(doc.layers()[1].pixels, before.layers()[1].pixels, "filled");
        assert_eq!(doc.layers()[0], before.layers()[0], "other layer");
        let (mut a, mut b) = (doc.layers()[1].clone(), before.layers()[1].clone());
        a.pixels = b.pixels.clone();
        b.pixels = a.pixels.clone();
        assert_eq!(a, b, "layer properties");
        assert!(!doc.layers()[1].visible);
        assert!((doc.layers()[1].opacity - 0.4).abs() < f32::EPSILON);
        assert_eq!(doc.canvas, before.canvas);
        assert_eq!(doc.selection, before.selection);
        assert_eq!(doc.layers().len(), before.layers().len());
    }
}
