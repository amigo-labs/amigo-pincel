//! Live objects in the core (docs/specs/image-live-objects.md): begin,
//! edit, undo, commit and cancel of a live shape, line or gradient.

use pincel_image_core::codec::ora::{from_ora_bytes, to_ora_bytes};
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{
    AddLayer, BoxShape, Color, CommandBus, DashPattern, Document, DocumentError, Gradient,
    GradientChannel, GradientKind, ImageBuffer, LiveEdit, LiveGradient, LiveObject, LiveParams,
    LiveShape, LiveShapeKind, Point, Rect, RenderCache, SelectionMask, SetLayerLocked, SetPixels,
    SetSelection, Shape, ShapeMode, ShapeStyle, Shapes, UndoStack, compose,
};

const W: u32 = 64;
const H: u32 = 48;

/// A two-layer document whose bottom layer has an opaque pattern, so a
/// Transparency gradient has something to fade.
fn bus() -> CommandBus {
    let mut doc = Document::new(W, H).expect("doc");
    doc.add_layer("Top").expect("layer");
    let mut bus = CommandBus::new(doc);
    let mut patch = ImageBuffer::new_transparent(W, H);
    for y in 0..H {
        for x in 0..W {
            let c = Color::rgba((x * 4) as u8, (y * 5) as u8, 90, 255);
            patch.set_pixel(x, y, c);
        }
    }
    bus.apply(Box::new(SetPixels::new(0, Rect::new(0, 0, W, H), patch)))
        .expect("pattern");
    bus.history.clear();
    bus
}

fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn style() -> ShapeStyle {
    ShapeStyle {
        mode: ShapeMode::FillAndOutline,
        stroke_width: 3.0,
        stroke_color: Color::rgba(200, 10, 10, 255),
        fill_color: Color::rgba(10, 200, 10, 180),
        anti_alias: true,
        dash: DashPattern::Solid,
        blend: pincel_image_core::Blend::Normal,
    }
}

fn boxed(kind: BoxShape, a: Point, b: Point) -> LiveParams {
    LiveParams::Shape(LiveShape {
        kind: LiveShapeKind::Box(kind),
        a,
        b,
        style: style(),
        corner_radius: 4.0,
        sides: 6,
    })
}

fn line(a: Point, b: Point) -> LiveParams {
    LiveParams::Shape(LiveShape {
        kind: LiveShapeKind::Line,
        a,
        b,
        style: style(),
        corner_radius: 0.0,
        sides: 3,
    })
}

fn gradient(channel: GradientChannel, a: Point, b: Point) -> LiveParams {
    LiveParams::Gradient(LiveGradient {
        gradient: Gradient {
            kind: GradientKind::Linear,
            channel,
            from: Color::rgba(255, 0, 0, 255),
            to: Color::rgba(0, 0, 255, 255),
            opacity: 0.8,
            blend: pincel_image_core::Blend::Normal,
        },
        a,
        b,
    })
}

/// The pixels of `layer` after today's immediate draw command of `params`
/// on `doc` — built from the public tools, not from the live code.
fn immediate(doc: &Document, layer: usize, params: &LiveParams) -> ImageBuffer {
    let cmd = match params {
        LiveParams::Shape(s) => {
            let shape = match s.kind {
                LiveShapeKind::Line => Shape::Line { a: s.a, b: s.b },
                LiveShapeKind::Box(BoxShape::Rectangle) => Shape::Rectangle { a: s.a, b: s.b },
                LiveShapeKind::Box(BoxShape::RoundedRectangle) => Shape::RoundedRectangle {
                    a: s.a,
                    b: s.b,
                    radius: s.corner_radius,
                },
                LiveShapeKind::Box(BoxShape::Ellipse) => Shape::Ellipse { a: s.a, b: s.b },
                LiveShapeKind::Box(BoxShape::Polygon) => {
                    let (w, h) = ((s.b.x - s.a.x).abs(), (s.b.y - s.a.y).abs());
                    Shape::Polygon {
                        center: p((s.a.x + s.b.x) / 2.0, (s.a.y + s.b.y) / 2.0),
                        radius: w.min(h) / 2.0,
                        sides: s.sides,
                        rotation: 0.0,
                    }
                }
            };
            Shapes::new(shape, s.style).draw(layer, doc)
        }
        LiveParams::Gradient(g) => g.gradient.draw(layer, g.a, g.b, doc),
        LiveParams::Text(_) => unreachable!("tests/live_text.rs covers text"),
    };
    let mut bus = CommandBus::new(doc.clone());
    if let Some(cmd) = cmd {
        bus.apply(Box::new(cmd)).expect("immediate draw");
    }
    bus.document.layers()[layer].pixels.clone()
}

fn pixels(bus: &CommandBus, layer: usize) -> &ImageBuffer {
    &bus.document.layers()[layer].pixels
}

fn live(bus: &CommandBus) -> Option<LiveObject> {
    bus.live_object()
}

fn moved(params: &LiveParams, dx: f32, dy: f32) -> LiveEdit {
    let [a, b] = params.points();
    LiveEdit {
        points: Some([p(a.x + dx, a.y + dy), p(b.x + dx, b.y + dy)]),
        ..LiveEdit::default()
    }
}

#[test]
fn begin_live_draws_the_immediate_draw_and_adds_one_step() {
    for params in [
        boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0)),
        boxed(BoxShape::Ellipse, p(4.0, 5.0), p(30.0, 20.0)),
        line(p(2.0, 3.0), p(50.0, 40.0)),
        gradient(GradientChannel::Color, p(0.0, 0.0), p(40.0, 10.0)),
    ] {
        let mut bus = bus();
        let pre = bus.document.clone();
        assert!(bus.begin_live(0, params.clone()).expect("begin"));
        assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &params));
        assert_eq!(compose(bus.document.layers()), {
            let mut layers = pre.layers().to_vec();
            layers[0].pixels = immediate(&pre, 0, &params);
            compose(&layers)
        });
        assert_eq!(bus.history.undo_depth(), 1);
        assert_eq!(live(&bus), Some(LiveObject { layer: 0, params }));
    }
}

#[test]
fn begin_live_off_canvas_is_live_without_changing_pixels() {
    let mut bus = bus();
    let pre = pixels(&bus, 0).clone();
    let params = boxed(BoxShape::Rectangle, p(-40.0, -40.0), p(-20.0, -20.0));
    assert!(bus.begin_live(0, params.clone()).expect("begin"));
    assert_eq!(pixels(&bus, 0), &pre);
    assert_eq!(bus.history.undo_depth(), 1);
    assert_eq!(live(&bus).map(|o| o.params), Some(params));
}

#[test]
fn edit_live_redraws_on_the_pre_object_pixels() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = boxed(BoxShape::Ellipse, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 9.0, 7.0), None)
        .expect("edit");
    let now = live(&bus).expect("live").params;
    assert_eq!(now.points(), [p(13.0, 12.0), p(39.0, 27.0)]);
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &now));
    assert_eq!(bus.history.undo_depth(), 2);
}

#[test]
fn edit_live_transparency_gradient_fades_the_pre_object_pixels_once() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = gradient(GradientChannel::Transparency, p(0.0, 0.0), p(40.0, 0.0));
    bus.begin_live(0, params.clone()).expect("begin");
    for i in 1..=10 {
        let edit = LiveEdit {
            points: Some([p(0.0, i as f32), p(40.0 + i as f32, 3.0)]),
            ..LiveEdit::default()
        };
        bus.edit_live(&edit, Some(i)).expect("edit");
    }
    let now = live(&bus).expect("live").params;
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &now));
}

#[test]
fn edit_live_of_one_gesture_is_one_step() {
    let mut bus = bus();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    for dx in 1..=5 {
        bus.edit_live(&moved(&params, dx as f32, 0.0), Some(7))
            .expect("edit");
    }
    assert_eq!(bus.history.undo_depth(), 2);
    bus.undo().expect("undo");
    assert_eq!(live(&bus).map(|o| o.params), Some(params));
}

#[test]
fn edit_live_gesture_ends_at_undo_and_at_other_edits() {
    let mut bus = bus();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 1.0, 0.0), Some(3))
        .expect("a");
    bus.edit_live(&moved(&params, 2.0, 0.0), Some(3))
        .expect("b");
    // Another edit ends the gesture: the same id after it is a new step.
    let width = LiveEdit {
        stroke_width: Some(5.0),
        ..LiveEdit::default()
    };
    bus.edit_live(&width, None).expect("width");
    bus.edit_live(&moved(&params, 3.0, 0.0), Some(3))
        .expect("c");
    assert_eq!(bus.history.undo_depth(), 4);
    // Undo ends it too.
    bus.undo().expect("undo");
    bus.edit_live(&moved(&params, 4.0, 0.0), Some(3))
        .expect("d");
    assert_eq!(bus.history.undo_depth(), 4);
}

#[test]
fn edit_live_consecutive_edits_of_one_group_merge() {
    let mut bus = bus();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    for w in [2.0, 3.0, 4.0] {
        let e = LiveEdit {
            stroke_width: Some(w),
            ..LiveEdit::default()
        };
        bus.edit_live(&e, None).expect("width");
    }
    assert_eq!(bus.history.undo_depth(), 2, "stroke widths merge");
    // Stroke color then fill color: one colors group.
    let stroke = LiveEdit {
        stroke_color: Some(Color::rgba(1, 2, 3, 128)),
        ..LiveEdit::default()
    };
    let fill = LiveEdit {
        fill_color: Some(Color::rgba(4, 5, 6, 255)),
        ..LiveEdit::default()
    };
    bus.edit_live(&stroke, None).expect("stroke");
    bus.edit_live(&fill, None).expect("fill");
    assert_eq!(bus.history.undo_depth(), 3, "colors merge");
    // A different group starts a new step.
    let mode = LiveEdit {
        mode: Some(ShapeMode::Outline),
        ..LiveEdit::default()
    };
    bus.edit_live(&mode, None).expect("mode");
    assert_eq!(bus.history.undo_depth(), 4);
    // An edit of two groups never merges.
    let both = LiveEdit {
        mode: Some(ShapeMode::Fill),
        dash: Some(DashPattern::Dashed),
        ..LiveEdit::default()
    };
    bus.edit_live(&both, None).expect("both");
    bus.edit_live(&both.clone(), None).expect("same again");
    let both2 = LiveEdit {
        mode: Some(ShapeMode::Outline),
        dash: Some(DashPattern::Dotted),
        ..LiveEdit::default()
    };
    bus.edit_live(&both2, None).expect("both2");
    assert_eq!(bus.history.undo_depth(), 6);
}

#[test]
fn undo_and_redo_walk_the_live_steps() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 6.0, 2.0), None)
        .expect("edit");
    let edited = live(&bus).expect("live").params;

    bus.undo().expect("undo edit");
    assert_eq!(live(&bus).map(|o| o.params), Some(params.clone()));
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &params));
    bus.redo().expect("redo edit");
    assert_eq!(live(&bus).map(|o| o.params), Some(edited.clone()));
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &edited));

    bus.undo().expect("undo edit");
    bus.undo().expect("undo begin");
    assert_eq!(live(&bus), None);
    assert_eq!(pixels(&bus, 0), &pre.layers()[0].pixels);
    bus.redo().expect("redo begin");
    assert_eq!(live(&bus).map(|o| o.params), Some(params));
}

#[test]
fn commit_live_folds_the_steps_into_one() {
    let mut bus = bus();
    bus.apply(Box::new(AddLayer::above(1))).expect("below");
    let pre = pixels(&bus, 0).clone();
    let params = line(p(2.0, 3.0), p(50.0, 40.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 1.0, 1.0), None)
        .expect("edit");
    bus.edit_live(
        &LiveEdit {
            stroke_width: Some(6.0),
            ..LiveEdit::default()
        },
        None,
    )
    .expect("width");
    let shown = pixels(&bus, 0).clone();

    bus.commit_live().expect("commit");
    assert_eq!(pixels(&bus, 0), &shown);
    assert_eq!(live(&bus), None);
    assert_eq!(bus.history.undo_depth(), 2);
    bus.undo().expect("undo");
    assert_eq!(pixels(&bus, 0), &pre);
    assert_eq!(live(&bus), None);
    bus.redo().expect("redo");
    assert_eq!(pixels(&bus, 0), &shown);
    assert_eq!(live(&bus), None);
}

#[test]
fn cancel_live_restores_the_pre_object_state() {
    let mut bus = bus();
    bus.apply(Box::new(AddLayer::above(1))).expect("below");
    let pre = pixels(&bus, 0).clone();
    let params = boxed(BoxShape::Ellipse, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 1.0, 1.0), None)
        .expect("edit");
    bus.edit_live(&moved(&params, 2.0, 1.0), Some(1))
        .expect("edit");
    bus.undo().expect("undo one edit");
    bus.cancel_live().expect("cancel");
    assert_eq!(pixels(&bus, 0), &pre);
    assert_eq!(live(&bus), None);
    assert_eq!(bus.history.undo_depth(), 1);
    assert_eq!(bus.history.redo_depth(), 0);
}

#[test]
fn edit_live_converts_between_box_shapes_only() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(40.0, 30.0));
    bus.begin_live(0, params.clone()).expect("begin");
    let to = |kind| LiveEdit {
        shape: Some(kind),
        ..LiveEdit::default()
    };
    bus.edit_live(&to(LiveShapeKind::Box(BoxShape::Polygon)), None)
        .expect("polygon");
    let now = live(&bus).expect("live").params;
    let LiveParams::Shape(s) = &now else {
        panic!("shape")
    };
    assert_eq!((s.sides, s.corner_radius, s.style), (6, 4.0, style()));
    assert_eq!(now.points(), params.points());
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &now));
    assert_eq!(bus.history.undo_depth(), 2);

    let shown = pixels(&bus, 0).clone();
    assert_eq!(
        bus.edit_live(&to(LiveShapeKind::Line), None),
        Err(DocumentError::LiveEditMismatch)
    );
    let gradient_field = LiveEdit {
        gradient_kind: Some(GradientKind::Radial),
        ..LiveEdit::default()
    };
    assert_eq!(
        bus.edit_live(&gradient_field, None),
        Err(DocumentError::LiveEditMismatch)
    );
    assert_eq!(pixels(&bus, 0), &shown);
    assert_eq!(live(&bus).map(|o| o.params), Some(now));
    assert_eq!(bus.history.undo_depth(), 2);

    let mut bus = self::bus();
    bus.begin_live(0, line(p(1.0, 1.0), p(9.0, 9.0)))
        .expect("line");
    assert_eq!(
        bus.edit_live(&to(LiveShapeKind::Box(BoxShape::Rectangle)), None),
        Err(DocumentError::LiveEditMismatch)
    );
    let mut bus = self::bus();
    bus.begin_live(
        0,
        gradient(GradientChannel::Color, p(1.0, 1.0), p(9.0, 9.0)),
    )
    .expect("gradient");
    assert_eq!(
        bus.edit_live(&to(LiveShapeKind::Box(BoxShape::Rectangle)), None),
        Err(DocumentError::LiveEditMismatch)
    );
}

#[test]
fn other_document_changes_commit_the_live_object_first() {
    let mut bus = bus();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 2.0, 2.0), None)
        .expect("edit");
    let shown = pixels(&bus, 0).clone();
    bus.apply(Box::new(AddLayer::above(1))).expect("add layer");
    assert_eq!(live(&bus), None);
    assert_eq!(bus.history.undo_depth(), 2, "committed step + new layer");
    bus.undo().expect("undo layer");
    assert_eq!(pixels(&bus, 0), &shown, "the committed object stays");
    assert_eq!(live(&bus), None);

    // A command through `run_command` (the editor's and MCP's path) sees
    // the object's pixels.
    let mut bus = self::bus();
    bus.begin_live(0, params.clone()).expect("begin");
    let shown = pixels(&bus, 0).clone();
    let stroke: CommandSpec = serde_json::from_str(
        r#"{"type":"pencil_stroke","layer":0,"size":1,"color":[0,0,0,255],"opacity":1,
            "points":[[60,40],[61,40]],"stroke_id":1}"#,
    )
    .expect("json");
    run_command(&mut bus, stroke, &mut StrokeScratch::default(), &[]).expect("stroke");
    assert_eq!(live(&bus), None);
    assert_eq!(bus.history.undo_depth(), 2);
    assert_eq!(
        pixels(&bus, 0).get_pixel(10, 10),
        shown.get_pixel(10, 10),
        "the stroke drew on the object's pixels"
    );

    // Beginning another object commits the first.
    let mut bus = self::bus();
    bus.begin_live(0, params.clone()).expect("begin");
    bus.begin_live(1, line(p(1.0, 1.0), p(9.0, 9.0)))
        .expect("second");
    assert_eq!(bus.history.undo_depth(), 2);
    assert_eq!(live(&bus).map(|o| o.layer), Some(1));
}

#[test]
fn set_active_layer_commits_only_on_a_change() {
    let mut bus = bus();
    bus.set_active_layer(0).expect("active");
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.set_active_layer(0).expect("same layer");
    assert!(live(&bus).is_some());
    assert!(bus.set_active_layer(9).is_err());
    assert!(live(&bus).is_some(), "a refused change commits nothing");
    bus.set_active_layer(1).expect("other layer");
    assert_eq!(live(&bus), None);
    assert_eq!(bus.document.active_layer_index(), 1);
    assert_eq!(bus.history.undo_depth(), 1);
}

#[test]
fn undo_and_redo_never_commit() {
    let mut bus = bus();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 2.0, 2.0), None)
        .expect("edit");
    bus.undo().expect("undo");
    assert!(live(&bus).is_some());
    assert_eq!(bus.history.redo_depth(), 1);
    bus.redo().expect("redo");
    assert!(live(&bus).is_some());
    assert_eq!(bus.history.undo_depth(), 2);
}

#[test]
fn begin_live_is_refused_on_locked_group_and_missing_layers() {
    let mut bus = bus();
    bus.apply(Box::new(SetLayerLocked::new(1, true)))
        .expect("lock");
    bus.history.clear();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    let err = bus.begin_live(1, params.clone()).unwrap_err();
    assert_eq!(err.to_string(), "layer is locked");
    assert!(matches!(
        bus.begin_live(7, params.clone()),
        Err(DocumentError::LayerIndexOutOfBounds { .. })
    ));
    let mut doc = Document::new(8, 8).expect("doc");
    doc.insert_layer(1, pincel_image_core::Layer::new_group("G"))
        .expect("group");
    let mut group_bus = CommandBus::new(doc);
    assert_eq!(
        group_bus.begin_live(1, params.clone()),
        Err(DocumentError::LayerLocked { index: 1 })
    );
    assert_eq!(bus.history.undo_depth(), 0);
    assert_eq!(live(&bus), None);
}

#[test]
fn begin_live_with_degenerate_geometry_begins_nothing() {
    for params in [
        line(p(5.0, 5.0), p(5.0, 5.0)),
        gradient(GradientChannel::Color, p(5.0, 5.0), p(5.0, 5.0)),
        boxed(BoxShape::Rectangle, p(5.0, 5.0), p(5.0, 30.0)),
        boxed(BoxShape::Ellipse, p(5.0, 5.0), p(30.0, 5.0)),
    ] {
        let mut bus = bus();
        let pre = pixels(&bus, 0).clone();
        assert!(!bus.begin_live(0, params).expect("not refused"));
        assert_eq!(bus.history.undo_depth(), 0);
        assert_eq!(live(&bus), None);
        assert_eq!(pixels(&bus, 0), &pre);
    }
}

#[test]
fn live_object_moved_off_canvas_and_back_is_drawn_whole() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = boxed(BoxShape::Ellipse, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, -200.0, -100.0), None)
        .expect("off");
    assert_eq!(pixels(&bus, 0), &pre.layers()[0].pixels);
    bus.edit_live(&moved(&params, -20.0, 3.0), None)
        .expect("partly back");
    let now = live(&bus).expect("live").params;
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &now));
    assert_eq!(now.points()[0], p(-16.0, 8.0));
}

#[test]
fn edit_live_beyond_the_reach_is_refused() {
    let mut bus = bus();
    let params = boxed(BoxShape::Ellipse, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    for (dx, dy) in [(-32_800.0, 0.0), (0.0, 32_800.0 + H as f32)] {
        assert_eq!(
            bus.edit_live(&moved(&params, dx, dy), None),
            Err(DocumentError::LiveGeometryOutOfRange)
        );
    }
    bus.edit_live(&moved(&params, -32_700.0, 0.0), None)
        .expect("within the reach");
    bus.undo().expect("undo");
    assert_eq!(live(&bus).map(|o| o.params), Some(params));
    assert_eq!(bus.history.undo_depth(), 1);
}

#[test]
fn live_object_is_clipped_by_a_feathered_selection() {
    let mut bus = bus();
    let mut mask = SelectionMask::rectangle(W, H, Rect::new(10, 8, 20, 12));
    for x in 6..34 {
        mask.set(x, 7, 128);
    }
    bus.apply(Box::new(SetSelection::replace(mask)))
        .expect("select");
    let pre = bus.document.clone();
    for params in [
        boxed(BoxShape::Rectangle, p(2.0, 2.0), p(40.0, 30.0)),
        gradient(GradientChannel::Color, p(0.0, 0.0), p(40.0, 10.0)),
    ] {
        bus.begin_live(0, params.clone()).expect("begin");
        bus.edit_live(&moved(&params, 1.0, 1.0), None)
            .expect("edit");
        let now = live(&bus).expect("live").params;
        assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &now));
        bus.cancel_live().expect("cancel");
    }
}

#[test]
fn commit_live_that_changes_no_pixel_leaves_no_step() {
    let mut bus = bus();
    bus.apply(Box::new(AddLayer::above(1))).expect("below");
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, -300.0, 0.0), None)
        .expect("off");
    bus.commit_live().expect("commit");
    assert_eq!(bus.history.undo_depth(), 1);

    // Degenerate after an edit, and outside an empty selection.
    bus.begin_live(0, params.clone()).expect("begin");
    let flat = LiveEdit {
        points: Some([p(4.0, 5.0), p(30.0, 5.0)]),
        ..LiveEdit::default()
    };
    bus.edit_live(&flat, None).expect("degenerate");
    bus.commit_live().expect("commit");
    assert_eq!(bus.history.undo_depth(), 1);

    let empty = SelectionMask::new_empty(W, H);
    bus.apply(Box::new(SetSelection::replace(empty)))
        .expect("empty");
    bus.begin_live(0, params).expect("begin");
    bus.commit_live().expect("commit");
    assert_eq!(bus.history.undo_depth(), 2, "only the selection step");
}

#[test]
fn save_and_composite_while_live_neither_commit_nor_touch_history() {
    let mut bus = bus();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 1.0, 0.0), None)
        .expect("edit");
    let shown = compose(bus.document.layers());
    let ora = to_ora_bytes(&bus.document, 1).expect("save");
    assert!(live(&bus).is_some());
    assert_eq!((bus.history.undo_depth(), bus.history.redo_depth()), (2, 0));
    let reopened = CommandBus::new(from_ora_bytes(&ora).expect("open"));
    assert_eq!(compose(reopened.document.layers()), shown);
    assert_eq!(reopened.live_object(), None);
}

#[test]
fn capacity_drops_steps_below_the_object_first() {
    let mut bus = bus();
    bus.history = UndoStack::with_capacity(10);
    for _ in 0..5 {
        bus.apply(Box::new(AddLayer::above(0))).expect("layer");
    }
    let pre = pixels(&bus, 0).clone();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    for i in 0..7 {
        bus.edit_live(&moved(&params, i as f32, 0.0), Some(i))
            .expect("edit");
    }
    // 5 layers + begin + 7 edits = 13 > 10: three layer steps dropped.
    assert_eq!(bus.history.undo_depth(), 10);
    for i in 7..20 {
        bus.edit_live(&moved(&params, i as f32, 1.0), Some(i))
            .expect("edit");
    }
    // Only the object is left; its oldest edits folded into the begin.
    assert_eq!(bus.history.undo_depth(), 10);
    assert!(live(&bus).is_some());
    for _ in 0..9 {
        bus.undo().expect("undo edit");
    }
    assert!(live(&bus).is_some(), "the begin step is still there");
    bus.undo().expect("undo begin");
    assert_eq!(live(&bus), None, "the bottom step began the object");
    assert_eq!(pixels(&bus, 0), &pre);
    for _ in 0..10 {
        bus.redo().expect("redo");
    }
    bus.cancel_live().expect("cancel");
    assert_eq!(pixels(&bus, 0), &pre);
    assert_eq!(bus.history.undo_depth(), 0);
}

#[test]
fn render_cache_matches_a_fresh_compose_after_every_live_operation() {
    let mut bus = bus();
    let mut cache = RenderCache::new(&bus.document);
    let full = Rect::new(0, 0, W, H);
    let mut check = |bus: &mut CommandBus, what: &str| {
        let damage = bus.take_damage();
        cache.update(&bus.document, damage);
        assert_eq!(
            cache.region(&bus.document, full, W, H),
            compose(bus.document.layers()),
            "{what}"
        );
    };
    check(&mut bus, "start");
    let params = boxed(BoxShape::Ellipse, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    check(&mut bus, "begin");
    bus.edit_live(&moved(&params, 12.0, 9.0), None)
        .expect("edit");
    check(&mut bus, "edit");
    bus.undo().expect("undo");
    check(&mut bus, "undo");
    bus.redo().expect("redo");
    check(&mut bus, "redo");
    bus.commit_live().expect("commit");
    check(&mut bus, "commit");
    let g = gradient(GradientChannel::Transparency, p(0.0, 0.0), p(30.0, 5.0));
    bus.begin_live(0, g.clone()).expect("begin gradient");
    check(&mut bus, "begin gradient");
    bus.edit_live(&moved(&g, 5.0, 5.0), None)
        .expect("edit gradient");
    check(&mut bus, "edit gradient");
    bus.cancel_live().expect("cancel");
    check(&mut bus, "cancel");
    bus.undo().expect("undo commit");
    check(&mut bus, "undo commit");
}

#[test]
fn live_operations_without_a_live_object_are_refused() {
    let mut bus = bus();
    let pre = pixels(&bus, 0).clone();
    let e = LiveEdit {
        stroke_width: Some(2.0),
        ..LiveEdit::default()
    };
    assert_eq!(bus.edit_live(&e, None), Err(DocumentError::NoLiveObject));
    assert_eq!(bus.commit_live(), Err(DocumentError::NoLiveObject));
    assert_eq!(bus.cancel_live(), Err(DocumentError::NoLiveObject));
    assert_eq!(pixels(&bus, 0), &pre);
    assert_eq!(bus.history.undo_depth(), 0);
}

#[test]
fn edit_live_clamps_like_the_draw_command() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    let e = LiveEdit {
        stroke_width: Some(9_999.0),
        ..LiveEdit::default()
    };
    bus.edit_live(&e, None).expect("wide");
    let now = live(&bus).expect("live").params;
    let LiveParams::Shape(s) = &now else {
        panic!("shape")
    };
    assert_eq!(s.style.stroke_width, 500.0);
    let mut unclamped = *s;
    unclamped.style.stroke_width = 9_999.0;
    assert_eq!(
        pixels(&bus, 0),
        &immediate(&pre, 0, &LiveParams::Shape(unclamped))
    );
}

#[test]
fn edit_live_to_degenerate_geometry_stays_live_and_draws_nothing() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = line(p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    let collapse = LiveEdit {
        points: Some([p(4.0, 5.0), p(4.0, 5.0)]),
        ..LiveEdit::default()
    };
    bus.edit_live(&collapse, Some(1)).expect("collapse");
    assert!(live(&bus).is_some());
    assert_eq!(pixels(&bus, 0), &pre.layers()[0].pixels);
    bus.edit_live(&moved(&params, 0.0, 1.0), Some(1))
        .expect("out again");
    let now = live(&bus).expect("live").params;
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &now));
}

#[test]
fn cancel_live_gesture_drops_the_drag_step_without_a_redo() {
    let mut bus = bus();
    let pre = bus.document.clone();
    let params = boxed(BoxShape::Rectangle, p(4.0, 5.0), p(30.0, 20.0));
    bus.begin_live(0, params.clone()).expect("begin");
    bus.edit_live(&moved(&params, 1.0, 0.0), Some(9))
        .expect("drag");
    bus.edit_live(&moved(&params, 5.0, 3.0), Some(9))
        .expect("drag");
    assert!(bus.cancel_live_gesture(9).expect("cancel drag"));
    assert_eq!(live(&bus).map(|o| o.params), Some(params.clone()));
    assert_eq!(pixels(&bus, 0), &immediate(&pre, 0, &params));
    assert_eq!((bus.history.undo_depth(), bus.history.redo_depth()), (1, 0));
    // Another gesture, or one that left no step, drops nothing.
    assert!(!bus.cancel_live_gesture(9).expect("nothing to drop"));
    bus.edit_live(&moved(&params, 2.0, 0.0), Some(10))
        .expect("drag");
    assert!(!bus.cancel_live_gesture(11).expect("other gesture"));
    assert_eq!(bus.history.undo_depth(), 2);
}

/// The byte-capped undo history (performance-image-undo-memory AC-03,
/// AC-05) keeps its count exact through live objects: begin, joined edits,
/// a cancelled drag, commit, cancel, undo and redo.
#[test]
fn live_object_steps_keep_the_undo_byte_count_exact() {
    let mut bus = bus();
    let exact = |bus: &CommandBus, at: &str| {
        assert_eq!(
            bus.history.stored_bytes(),
            bus.history.recount_bytes(),
            "{at}"
        );
    };
    let params = line(p(2.0, 3.0), p(50.0, 40.0));
    bus.begin_live(1, params.clone()).expect("begin");
    exact(&bus, "begin");
    bus.edit_live(&moved(&params, 1.0, 1.0), Some(7))
        .expect("drag");
    bus.edit_live(&moved(&params, 2.0, 2.0), Some(7))
        .expect("drag on");
    exact(&bus, "joined edits");
    assert!(bus.cancel_live_gesture(7).expect("cancel drag"));
    exact(&bus, "cancelled drag");
    bus.commit_live().expect("commit");
    exact(&bus, "commit");
    bus.undo().expect("undo");
    exact(&bus, "undo");
    bus.redo().expect("redo");
    exact(&bus, "redo");
    bus.begin_live(1, params).expect("begin again");
    bus.cancel_live().expect("cancel");
    exact(&bus, "cancel");
}
