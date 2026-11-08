//! A live Line / Curve in the core (docs/specs/image-shapes-curves.md
//! AC-12, AC-13, AC-15, AC-16, AC-18): control points and caps on the live
//! line, its commit, and `draw_shape` lines with control points.

use pincel_image_core::command::live_spec::{LiveObjectSpec, live_object_spec};
use pincel_image_core::command::spec::{CommandSpec, StrokeScratch, run_command};
use pincel_image_core::{
    Color, CommandBus, Curve, CurveLine, DashPattern, Document, DocumentError, ImageBuffer,
    LineCap, Point, ShapeMode, ShapeStyle,
};

const W: u32 = 96;
const H: u32 = 64;

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

fn style(width: f32, color: [u8; 4], dash: DashPattern, aa: bool) -> ShapeStyle {
    ShapeStyle {
        mode: ShapeMode::Outline,
        stroke_width: width,
        stroke_color: Color::rgba(color[0], color[1], color[2], color[3]),
        anti_alias: aa,
        dash,
        ..ShapeStyle::default()
    }
}

/// The layer after drawing `curve` with `style` on a fresh document.
fn expected(points: &[[f32; 2]], start: LineCap, end: LineCap, style: ShapeStyle) -> Vec<u8> {
    let mut b = bus();
    let line = CurveLine::new(
        Curve {
            points: points.iter().map(|q| Point::new(q[0], q[1])).collect(),
            start_cap: start,
            end_cap: end,
        },
        style,
    );
    if let Some(cmd) = line.draw(0, &b.document) {
        b.apply(Box::new(cmd)).expect("apply");
    }
    pixels(&b)
}

const BEGIN: &str = r#"{"type":"begin_live_shape","layer":0,"shape":"line","points":[[8,50],[88,12]],"stroke_width":3,"stroke_color":[200,0,0,255],"anti_alias":true}"#;

/// AC-22: the move grip sits on the curve's midpoint by arc length, its
/// normal perpendicular to the curve there — on an asymmetric curve, where
/// that point is neither the control point nor the parameter midpoint.
#[test]
fn the_grip_sits_on_the_arc_length_midpoint_with_the_curve_normal() {
    let mut b = bus();
    run(&mut b, BEGIN).expect("begin");
    run(
        &mut b,
        r#"{"type":"edit_live","gesture":1,"control":[[20,10]]}"#,
    )
    .expect("add point");
    let Some(LiveObjectSpec::Shape {
        polyline,
        grip_anchor,
        grip_normal,
        ..
    }) = live_object_spec(&b)
    else {
        panic!("a live line")
    };
    let len = |(dx, dy): (f32, f32)| (dx * dx + dy * dy).sqrt();
    let segs: Vec<([f32; 2], (f32, f32))> = polyline
        .windows(2)
        .map(|w| (w[0], (w[1][0] - w[0][0], w[1][1] - w[0][1])))
        .collect();
    let total: f32 = segs.iter().map(|&(_, d)| len(d)).sum();
    // Walk half the arc length along the polyline.
    let mut left = total / 2.0;
    let mut mid = None;
    for &(a, d) in &segs {
        let l = len(d);
        if left <= l && l > 0.0 {
            let t = left / l;
            mid = Some(([a[0] + d.0 * t, a[1] + d.1 * t], (d.0 / l, d.1 / l)));
            break;
        }
        left -= l;
    }
    let (mid, dir) = mid.expect("a midpoint");
    let off = len((grip_anchor[0] - mid[0], grip_anchor[1] - mid[1]));
    assert!(
        off <= 0.5,
        "grip {grip_anchor:?} is {off} px from the arc midpoint {mid:?}"
    );
    assert!(
        (len((grip_normal[0], grip_normal[1])) - 1.0).abs() < 1e-3,
        "{grip_normal:?}"
    );
    let dot = grip_normal[0] * dir.0 + grip_normal[1] * dir.1;
    assert!(
        dot.abs() < 0.02,
        "the normal {grip_normal:?} is not perpendicular to {dir:?}"
    );
    // The curve is asymmetric: the parameter midpoint of its two spans is
    // the control point, well away from the arc midpoint.
    assert!(len((mid[0] - 20.0, mid[1] - 10.0)) > 5.0);
}

#[test]
fn control_points_and_caps_edit_the_live_line_and_the_commit_uses_them() {
    let mut b = bus();
    run(&mut b, BEGIN).expect("begin");
    run(
        &mut b,
        r#"{"type":"edit_live","gesture":1,"control":[[40,10]]}"#,
    )
    .expect("add point");
    run(
        &mut b,
        r#"{"type":"edit_live","gesture":1,"control":[[44,6]]}"#,
    )
    .expect("drag it");
    run(
        &mut b,
        r#"{"type":"edit_live","start_cap":"arrow","end_cap":"flat"}"#,
    )
    .expect("caps");
    run(&mut b, r#"{"type":"edit_live","dash":"dash_dot"}"#).expect("dash");
    run(&mut b, r#"{"type":"edit_live","stroke_width":5}"#).expect("width");
    run(
        &mut b,
        r#"{"type":"edit_live","stroke_color":[0,0,220,255]}"#,
    )
    .expect("color");
    run(&mut b, r#"{"type":"edit_live","anti_alias":false}"#).expect("aa");
    let pts = [[8.0, 50.0], [44.0, 6.0], [88.0, 12.0]];
    let want = expected(
        &pts,
        LineCap::Arrow,
        LineCap::Flat,
        style(5.0, [0, 0, 220, 255], DashPattern::DashDot, false),
    );
    assert_eq!(pixels(&b), want, "the live line shows every change at once");
    let Some(LiveObjectSpec::Shape {
        control,
        start_cap,
        end_cap,
        dash,
        polyline,
        grip_anchor,
        ..
    }) = live_object_spec(&b)
    else {
        panic!("a live line")
    };
    assert_eq!(control, vec![[44.0, 6.0]]);
    assert_eq!(
        (start_cap.as_str(), end_cap.as_str(), dash.as_str()),
        ("arrow", "flat", "dash_dot")
    );
    assert!(polyline.len() > 3, "the curve's polyline for hit-testing");
    assert!(grip_anchor != [0.0, 0.0]);

    run(&mut b, r#"{"type":"commit_live"}"#).expect("commit");
    assert_eq!(pixels(&b), want, "the commit uses the edited line");
    assert!(live_object_spec(&b).is_none());
    // One step, of the Line's kind.
    assert_eq!(b.history.undo_depth(), 1);
    assert_eq!(
        b.history.undo_kinds().collect::<Vec<_>>(),
        vec![pincel_image_core::command::HistoryKind::Line]
    );
    b.undo().expect("undo");
    assert_eq!(pixels(&b), pixels(&bus()), "one Undo restores the layer");
    assert!(live_object_spec(&b).is_none());
    b.redo().expect("redo");
    assert_eq!(pixels(&b), want, "Redo paints the same curve");
    assert!(live_object_spec(&b).is_none());
}

#[test]
fn undo_while_live_takes_back_the_last_edit_and_escape_cancels() {
    let mut b = bus();
    run(&mut b, BEGIN).expect("begin");
    run(
        &mut b,
        r#"{"type":"edit_live","gesture":1,"control":[[40,10]]}"#,
    )
    .expect("add point");
    let with_point = pixels(&b);
    run(&mut b, r#"{"type":"edit_live","end_cap":"arrow"}"#).expect("cap");
    b.undo().expect("undo the cap");
    assert_eq!(pixels(&b), with_point);
    let Some(LiveObjectSpec::Shape {
        control, end_cap, ..
    }) = live_object_spec(&b)
    else {
        panic!("still live")
    };
    assert_eq!((control, end_cap.as_str()), (vec![[40.0, 10.0]], "round"));
    b.undo().expect("undo the point");
    let Some(LiveObjectSpec::Shape { control, .. }) = live_object_spec(&b) else {
        panic!("still live")
    };
    assert!(control.is_empty());
    run(
        &mut b,
        r#"{"type":"edit_live","gesture":2,"control":[[30,20],[60,40]]}"#,
    )
    .expect("points");
    run(&mut b, r#"{"type":"cancel_live"}"#).expect("cancel");
    assert_eq!(
        pixels(&b),
        pixels(&bus()),
        "Escape leaves the layer as before the line"
    );
    assert_eq!(b.history.undo_depth(), 0, "none of its steps remain");
}

#[test]
fn a_line_holds_at_most_64_points() {
    let mut b = bus();
    run(&mut b, BEGIN).expect("begin");
    let points = |n: usize| {
        let list: Vec<String> = (0..n)
            .map(|i| format!("[{},{}]", 10 + i, 20 + i % 7))
            .collect();
        format!(r#"{{"type":"edit_live","control":[{}]}}"#, list.join(","))
    };
    run(&mut b, &points(62)).expect("64 points in all");
    assert_eq!(
        run(&mut b, &points(63)),
        Err(DocumentError::LiveEditMismatch)
    );
    let Some(LiveObjectSpec::Shape { control, .. }) = live_object_spec(&b) else {
        panic!("still live")
    };
    assert_eq!(control.len(), 62, "the refused edit changed nothing");
}

#[test]
fn a_live_line_on_one_spot_with_a_flat_cap_commits_without_a_step() {
    let mut b = bus();
    run(&mut b, BEGIN).expect("begin");
    run(
        &mut b,
        r#"{"type":"edit_live","points":[[20,20],[20,20]],"control":[[20,20]],"start_cap":"flat"}"#,
    )
    .expect("collapse");
    assert_eq!(pixels(&b), pixels(&bus()), "draws nothing");
    run(&mut b, r#"{"type":"commit_live"}"#).expect("commit");
    assert_eq!(b.history.undo_depth(), 0);
}

#[test]
fn control_points_and_caps_only_fit_a_line() {
    let mut b = bus();
    assert_eq!(
        run(
            &mut b,
            r#"{"type":"begin_live_shape","layer":0,"shape":"rectangle","points":[[8,8],[40,30]],"control":[[20,20]]}"#
        ),
        Err(DocumentError::LiveEditMismatch)
    );
    run(&mut b, r#"{"type":"begin_live_shape","layer":0,"shape":"ellipse","points":[[8,8],[40,30]],"stroke_width":2}"#)
        .expect("begin");
    assert_eq!(
        run(&mut b, r#"{"type":"edit_live","end_cap":"arrow"}"#),
        Err(DocumentError::LiveEditMismatch)
    );
    assert_eq!(
        run(&mut b, r#"{"type":"edit_live","control":[]}"#),
        Err(DocumentError::LiveEditMismatch)
    );
}

#[test]
fn draw_shape_draws_a_line_through_its_control_points_with_caps() {
    let mut b = bus();
    run(
        &mut b,
        r#"{"type":"draw_shape","layer":0,"shape":"line","points":[[8,50],[30,8],[60,58],[88,12]],"start_cap":"arrow","end_cap":"flat","dash":"dash_dot_dot","stroke_width":4,"stroke_color":[0,120,0,255],"anti_alias":true}"#,
    )
    .expect("draw");
    let pts = [[8.0, 50.0], [30.0, 8.0], [60.0, 58.0], [88.0, 12.0]];
    assert_eq!(
        pixels(&b),
        expected(
            &pts,
            LineCap::Arrow,
            LineCap::Flat,
            style(4.0, [0, 120, 0, 255], DashPattern::DashDotDot, true)
        )
    );
    assert_eq!(b.history.undo_depth(), 1);
    // A two-point line without caps draws today's line.
    let mut c = bus();
    run(&mut c, r#"{"type":"draw_shape","layer":0,"shape":"line","points":[[8,50],[88,12]],"stroke_width":3,"stroke_color":[200,0,0,255],"anti_alias":true}"#)
        .expect("draw");
    assert_eq!(
        pixels(&c),
        expected(
            &[[8.0, 50.0], [88.0, 12.0]],
            LineCap::Round,
            LineCap::Round,
            style(3.0, [200, 0, 0, 255], DashPattern::Solid, true)
        )
    );
}
