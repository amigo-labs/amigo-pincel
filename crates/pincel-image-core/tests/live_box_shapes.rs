//! The ten box shapes as live objects (docs/specs/image-shapes-curves-shapes.md
//! AC-19, AC-20): conversion between any two, star options and box edits
//! of Block Arrow and Callout redraw the shape as one step.

use pincel_image_core::command::live_spec::{LiveObjectSpec, live_object_spec};
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{CommandBus, Document, DocumentError, ImageBuffer};

const W: u32 = 96;
const H: u32 = 72;
const BOXES: [&str; 10] = [
    "rectangle",
    "rounded_rectangle",
    "ellipse",
    "polygon",
    "triangle",
    "diamond",
    "star",
    "block_arrow",
    "heart",
    "callout",
];
const STYLE: &str = r#""mode":"fill_and_outline","stroke_width":3,"stroke_color":[200,0,0,255],"fill_color":[0,0,200,255],"anti_alias":true,"corner_radius":6,"sides":6,"star_points":7,"inner_ratio":0.5"#;

fn bus() -> CommandBus {
    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            data.extend_from_slice(&[(x * 2) as u8, (y * 3) as u8, 90, 255]);
        }
    }
    CommandBus::new(
        Document::from_pixels(ImageBuffer::from_raw(W, H, data).expect("buffer")).expect("doc"),
    )
}

fn run(bus: &mut CommandBus, json: &str) -> Result<(), DocumentError> {
    let spec: CommandSpec = serde_json::from_str(json).expect(json);
    run_command(bus, spec, &mut StrokeScratch::default(), &[])
}

fn pixels(bus: &CommandBus) -> Vec<u8> {
    bus.document.layers()[0].pixels.data().to_vec()
}

/// The pixels the box shape `kind` draws over `[a, b]` with the style above,
/// as a live begin (the immediate draw of those parameters).
fn begun(kind: &str, a: [f32; 2], b: [f32; 2]) -> Vec<u8> {
    let mut b2 = bus();
    run(
        &mut b2,
        &format!(r#"{{"type":"begin_live_shape","layer":0,"shape":"{kind}","points":[[{},{}],[{},{}]],{STYLE}}}"#, a[0], a[1], b[0], b[1]),
    )
    .expect("begin");
    pixels(&b2)
}

fn shape_of(bus: &CommandBus) -> String {
    match live_object_spec(bus) {
        Some(LiveObjectSpec::Shape { shape, .. }) => shape,
        other => panic!("not a live shape: {other:?}"),
    }
}

#[test]
fn every_box_shape_converts_to_every_other_keeping_box_and_style() {
    let (a, b) = ([12.0, 8.0], [80.0, 62.0]);
    for from in BOXES {
        for to in BOXES {
            if from == to {
                continue;
            }
            let mut bus = bus();
            run(
                &mut bus,
                &format!(r#"{{"type":"begin_live_shape","layer":0,"shape":"{from}","points":[[12,8],[80,62]],{STYLE}}}"#),
            )
            .expect("begin");
            let depth = bus.history.undo_depth();
            run(
                &mut bus,
                &format!(r#"{{"type":"edit_live","shape":"{to}"}}"#),
            )
            .expect("convert");
            assert_eq!(
                bus.history.undo_depth(),
                depth + 1,
                "{from} → {to}: one step"
            );
            assert_eq!(shape_of(&bus), to, "{from} → {to}: still live as {to}");
            assert_eq!(
                pixels(&bus),
                begun(to, a, b),
                "{from} → {to}: same box and style"
            );
        }
    }
}

#[test]
fn a_star_keeps_its_options_through_a_conversion_and_redraws_on_a_change() {
    let mut bus = bus();
    run(&mut bus, &format!(r#"{{"type":"begin_live_shape","layer":0,"shape":"star","points":[[12,8],[80,62]],{STYLE}}}"#))
        .expect("begin");
    run(&mut bus, r#"{"type":"edit_live","shape":"polygon"}"#).expect("to polygon");
    run(&mut bus, r#"{"type":"edit_live","shape":"star"}"#).expect("back");
    assert_eq!(
        pixels(&bus),
        begun("star", [12.0, 8.0], [80.0, 62.0]),
        "points 7, ratio 0.5 kept"
    );
    // Points and inner radius redraw the live star at once, one step each.
    let depth = bus.history.undo_depth();
    run(&mut bus, r#"{"type":"edit_live","star_points":11}"#).expect("points");
    assert_eq!(bus.history.undo_depth(), depth + 1);
    run(&mut bus, r#"{"type":"edit_live","inner_ratio":0.25}"#).expect("ratio");
    assert_eq!(bus.history.undo_depth(), depth + 2);
    let want = bus_with_star(11, 0.25);
    assert_eq!(pixels(&bus), pixels(&want));
    let Some(LiveObjectSpec::Shape {
        star_points,
        inner_ratio,
        ..
    }) = live_object_spec(&bus)
    else {
        panic!("live star")
    };
    assert_eq!((star_points, inner_ratio), (11, 0.25));
}

fn bus_with_star(points: u32, ratio: f32) -> CommandBus {
    let mut b = bus();
    run(
        &mut b,
        &format!(r#"{{"type":"begin_live_shape","layer":0,"shape":"star","points":[[12,8],[80,62]],"mode":"fill_and_outline","stroke_width":3,"stroke_color":[200,0,0,255],"fill_color":[0,0,200,255],"anti_alias":true,"star_points":{points},"inner_ratio":{ratio}}}"#),
    )
    .expect("begin");
    b
}

#[test]
fn box_edits_of_an_arrow_or_a_callout_reapply_start_corner_and_axis() {
    for kind in ["block_arrow", "callout"] {
        let mut bus = bus();
        run(&mut bus, &format!(r#"{{"type":"begin_live_shape","layer":0,"shape":"{kind}","points":[[10,20],[70,44]],{STYLE}}}"#))
            .expect("begin");
        // The start corner's edge dragged past the opposite one: the start
        // corner moves with it, and the box is now taller than wide.
        for (a, b) in [
            ([80.0, 20.0], [70.0, 44.0]),
            ([80.0, 4.0], [70.0, 68.0]),
            ([10.0, 60.0], [70.0, 10.0]),
        ] {
            let depth = bus.history.undo_depth();
            run(
                &mut bus,
                &format!(
                    r#"{{"type":"edit_live","gesture":1,"points":[[{},{}],[{},{}]]}}"#,
                    a[0], a[1], b[0], b[1]
                ),
            )
            .expect("edit");
            assert!(
                bus.history.undo_depth() <= depth + 1,
                "{kind}: one step per gesture"
            );
            assert_eq!(pixels(&bus), begun(kind, a, b), "{kind} {a:?} {b:?}");
            assert_eq!(shape_of(&bus), kind, "stays live");
        }
    }
}
