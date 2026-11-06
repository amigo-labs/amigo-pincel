//! The live text kind in the core (docs/specs/image-live-text.md): drawn at
//! its box, edited, merged, committed and cancelled like every live object.

use pincel_image_core::command::live_spec::{LiveObjectSpec, live_object_spec};
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{CommandBus, Document, DocumentError, ImageBuffer, TextAlign};

/// Liberation Sans, the editor's bundled face.
const FONT: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../pincel-core/tests/fixtures/LiberationSans-Regular.ttf"
));

fn run(bus: &mut CommandBus, json: &str) -> Result<(), DocumentError> {
    let spec: CommandSpec = serde_json::from_str(json).expect("json");
    run_command(bus, spec, &mut StrokeScratch::default(), &[FONT])
}

fn bus() -> CommandBus {
    CommandBus::new(Document::new(200, 80).expect("doc"))
}

fn pixels(bus: &CommandBus) -> &ImageBuffer {
    &bus.document.layers()[0].pixels
}

const BEGIN: &str = r#"{"type":"begin_live_text","layer":0,"font_id":0,"text":"Hi\nthere",
    "x":20,"y":10,"size":20,"color":[0,0,0,255],"align":"center","underline":true}"#;

#[test]
fn begin_live_text_draws_draw_text_at_the_box_anchor() {
    let mut live = bus();
    run(&mut live, BEGIN).expect("begin");
    let Some(LiveObjectSpec::Text { x, y, .. }) = live_object_spec(&live) else {
        panic!("a live text")
    };
    assert_eq!((x, y), (20.0, 10.0));
    let style = pincel_image_core::TextStyle {
        size: 20.0,
        align: TextAlign::Center,
        underline: true,
        anti_alias: false,
        ..pincel_image_core::TextStyle::default()
    };
    let width = pincel_image_core::Text::new(
        "Hi\nthere",
        pincel_image_core::Point::new(20.0, 10.0),
        style,
    )
    .layout_lines(FONT)
    .expect("layout")
    .width;
    let mut drawn = bus();
    run(
        &mut drawn,
        &format!(
            r#"{{"type":"draw_text","layer":0,"font_id":0,"text":"Hi\nthere","x":{},"y":10,
                "size":20,"color":[0,0,0,255],"align":"center","underline":true}}"#,
            20.0 + width / 2.0
        ),
    )
    .expect("draw");
    assert_eq!(pixels(&live), pixels(&drawn));
    assert_eq!(live.history.undo_depth(), 1);
}

#[test]
fn edit_live_text_keeps_the_box_left_edge_and_merges_typing() {
    let mut bus = bus();
    run(&mut bus, BEGIN).expect("begin");
    for text in ["Hi\nthere!", "Hi\nthere!!", "Hi\nthere!!!"] {
        let edit = format!(
            r#"{{"type":"edit_live","gesture":5,"text":"{}"}}"#,
            text.replace('\n', "\\n")
        );
        run(&mut bus, &edit).expect("type");
    }
    assert_eq!(bus.history.undo_depth(), 2, "one typing burst is one step");
    for size in [24, 28] {
        run(
            &mut bus,
            &format!(r#"{{"type":"edit_live","size":{size}}}"#),
        )
        .expect("size");
    }
    run(
        &mut bus,
        r#"{"type":"edit_live","bold":true,"strikethrough":true}"#,
    )
    .expect("style");
    assert_eq!(
        bus.history.undo_depth(),
        4,
        "sizes merge; two groups are a step"
    );
    let Some(LiveObjectSpec::Text {
        x,
        y,
        size,
        bold,
        text,
        ..
    }) = live_object_spec(&bus)
    else {
        panic!("a live text")
    };
    assert_eq!(
        (x, y, size, bold, text.as_str()),
        (20.0, 10.0, 28.0, true, "Hi\nthere!!!")
    );
    bus.undo().expect("undo");
    bus.undo().expect("undo");
    bus.undo().expect("undo");
    assert!(matches!(
        live_object_spec(&bus),
        Some(LiveObjectSpec::Text { ref text, .. }) if text == "Hi\nthere"
    ));
}

#[test]
fn live_text_refuses_shape_fields_and_moves_by_its_points() {
    let mut bus = bus();
    run(&mut bus, BEGIN).expect("begin");
    assert_eq!(
        run(&mut bus, r#"{"type":"edit_live","stroke_width":3}"#),
        Err(DocumentError::LiveEditMismatch)
    );
    run(
        &mut bus,
        r#"{"type":"edit_live","points":[[40,30],[40,30]]}"#,
    )
    .expect("move");
    let Some(LiveObjectSpec::Text { x, y, .. }) = live_object_spec(&bus) else {
        panic!("a live text")
    };
    assert_eq!((x, y), (40.0, 30.0));
}

#[test]
fn whitespace_or_off_canvas_text_commits_to_no_step() {
    for edit in [
        r#"{"type":"edit_live","text":"  \n "}"#,
        r#"{"type":"edit_live","points":[[900,10],[900,10]]}"#,
    ] {
        let mut bus = bus();
        run(&mut bus, BEGIN).expect("begin");
        run(&mut bus, edit).expect("edit");
        run(&mut bus, r#"{"type":"commit_live"}"#).expect("commit");
        assert_eq!(bus.history.undo_depth(), 0);
        assert!(pixels(&bus).data().iter().all(|&b| b == 0));
    }
}

#[test]
fn an_empty_text_begins_nothing_but_an_emptied_one_stays_live() {
    let mut bus = bus();
    run(
        &mut bus,
        r#"{"type":"begin_live_text","layer":0,"font_id":0,"text":"","x":5,"y":5}"#,
    )
    .expect("begin");
    assert_eq!(live_object_spec(&bus), None);
    run(&mut bus, BEGIN).expect("begin");
    run(&mut bus, r#"{"type":"edit_live","text":""}"#).expect("delete all");
    assert!(live_object_spec(&bus).is_some());
    assert!(pixels(&bus).data().iter().all(|&b| b == 0));
}

#[test]
fn text_layout_query_matches_the_drawn_box() {
    use pincel_image_core::command::live_spec::{TextLayoutRequest, text_layout};
    let req: TextLayoutRequest =
        serde_json::from_str(r#"{"text":"Hi\nthere","x":20,"y":10,"size":20,"align":"center"}"#)
            .expect("json");
    let layout = text_layout(&req, FONT).expect("layout");
    assert_eq!(layout.lines.len(), 2);
    let widest = layout.lines.iter().map(|l| l.width).fold(0.0, f32::max);
    assert!((layout.width - widest).abs() < 1e-3);
    // Center: every line is centred in the box that starts at x = 20.
    for line in &layout.lines {
        let centre = line.left + line.width / 2.0;
        assert!((centre - (20.0 + layout.width / 2.0)).abs() < 1e-3);
    }
    assert_eq!(layout.lines[1].carets.len(), "there".len() + 1);
}
