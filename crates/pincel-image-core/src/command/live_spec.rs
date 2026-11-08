//! The wire form of a live object (docs/specs/image-live-objects.md): what
//! `pincel-image-wasm` reports to the editor so it can draw and hit-test
//! handles. The commands that begin, edit, commit and cancel one are
//! [`CommandSpec`](super::spec::CommandSpec) variants.

use serde::{Deserialize, Serialize};

use super::{BoxShape, CommandBus, LiveParams, LiveShapeKind};
use crate::color::Color;
use crate::tools::{
    DashPattern, GradientChannel, GradientKind, ShapeMode, TextAlign, TextLayout, TextStyle,
};

/// The live object as the editor sees it; field names and string values
/// match `begin_live_shape` / `begin_live_gradient`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LiveObjectSpec {
    /// A live Line or box shape.
    Shape {
        /// The layer it is drawn into.
        layer: usize,
        /// `line`, `rectangle`, `rounded_rectangle`, `ellipse` or `polygon`.
        shape: String,
        /// Endpoints or opposite box corners.
        points: [[f32; 2]; 2],
        corner_radius: f32,
        sides: u32,
        /// A star's points and inner radius ratio (kept for every kind).
        star_points: u32,
        inner_ratio: f32,
        mode: String,
        stroke_width: f32,
        stroke_color: [u8; 4],
        fill_color: [u8; 4],
        anti_alias: bool,
        dash: String,
        /// A line's control points between the endpoints, in order
        /// (docs/specs/image-shapes-curves.md); empty otherwise.
        control: Vec<[f32; 2]>,
        /// A line's caps: `round`, `flat` or `arrow`.
        start_cap: String,
        end_cap: String,
        /// The line's curve as the core flattens and draws it, for the
        /// editor's hit-testing of the stroke; empty for a box shape.
        polyline: Vec<[f32; 2]>,
        /// Where the line's move grip is anchored — the curve's midpoint by
        /// arc length — and the unit normal there (AC-22); zero for a box.
        grip_anchor: [f32; 2],
        grip_normal: [f32; 2],
    },
    /// A live text: `x`, `y` are the top-left of its box.
    Text {
        /// The layer it is drawn into.
        layer: usize,
        text: String,
        font_id: u32,
        x: f32,
        y: f32,
        size: f32,
        color: [u8; 4],
        bold: bool,
        italic: bool,
        anti_alias: bool,
        align: String,
        underline: bool,
        strikethrough: bool,
    },
    /// A live gradient.
    Gradient {
        /// The layer it is drawn into.
        layer: usize,
        kind: String,
        channel: String,
        /// Drag start and end.
        points: [[f32; 2]; 2],
        from: [u8; 4],
        to: [u8; 4],
        opacity: f32,
    },
}

/// The live object of `bus` in wire form, or `None`.
pub fn live_object_spec(bus: &CommandBus) -> Option<LiveObjectSpec> {
    let live = bus.live_object()?;
    let points = live.params.points().map(|p| [p.x, p.y]);
    Some(match live.params {
        LiveParams::Shape(s) => {
            let line = s.kind == LiveShapeKind::Line;
            let curve = s.curve();
            let (anchor, normal) = if line {
                curve.midpoint()
            } else {
                (
                    crate::geometry::Point::new(0.0, 0.0),
                    crate::geometry::Point::new(0.0, 0.0),
                )
            };
            let xy = |p: &crate::geometry::Point| [p.x, p.y];
            LiveObjectSpec::Shape {
                layer: live.layer,
                shape: live_shape_str(s.kind).to_string(),
                points,
                corner_radius: s.corner_radius,
                sides: s.sides,
                star_points: s.star_points,
                inner_ratio: s.inner_ratio,
                mode: shape_mode_str(s.style.mode).to_string(),
                stroke_width: s.style.stroke_width,
                stroke_color: bytes(s.style.stroke_color),
                fill_color: bytes(s.style.fill_color),
                anti_alias: s.style.anti_alias,
                dash: dash_str(s.style.dash).to_string(),
                control: s.control.iter().map(xy).collect(),
                start_cap: s.start_cap.name().to_string(),
                end_cap: s.end_cap.name().to_string(),
                polyline: if line {
                    curve.polyline().iter().map(xy).collect()
                } else {
                    Vec::new()
                },
                grip_anchor: xy(&anchor),
                grip_normal: xy(&normal),
            }
        }
        LiveParams::Text(t) => LiveObjectSpec::Text {
            layer: live.layer,
            text: t.text,
            font_id: t.font.id,
            x: t.left,
            y: t.top,
            size: t.style.size,
            color: bytes(t.style.color),
            bold: t.style.bold,
            italic: t.style.italic,
            anti_alias: t.style.anti_alias,
            align: match t.style.align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Right => "right",
            }
            .to_string(),
            underline: t.style.underline,
            strikethrough: t.style.strikethrough,
        },
        LiveParams::Gradient(g) => LiveObjectSpec::Gradient {
            layer: live.layer,
            kind: gradient_kind_str(g.gradient.kind).to_string(),
            channel: gradient_channel_str(g.gradient.channel).to_string(),
            points,
            from: bytes(g.gradient.from),
            to: bytes(g.gradient.to),
            opacity: g.gradient.opacity,
        },
    })
}

/// The text and settings a text-layout query lays out
/// (docs/specs/image-live-text.md AC-22): the box's top-left and what
/// changes the glyph positions.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TextLayoutRequest {
    pub text: String,
    /// Left edge of the text box.
    pub x: f32,
    /// Top of the text box.
    pub y: f32,
    pub size: f32,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// `left`, `center` or `right`.
    #[serde(default)]
    pub align: String,
}

/// Lays out `req` with `font` exactly as a live text with these settings is
/// drawn: line boxes and the caret x of every character boundary. `None`
/// when the font fails to parse.
pub fn text_layout(req: &TextLayoutRequest, font: &[u8]) -> Option<TextLayout> {
    let style = TextStyle {
        size: req.size,
        bold: req.bold,
        italic: req.italic,
        align: match req.align.as_str() {
            "center" => TextAlign::Center,
            "right" => TextAlign::Right,
            _ => TextAlign::Left,
        },
        ..TextStyle::default()
    };
    super::text_box(&req.text, req.x, req.y, &style, font).map(|(_, layout)| layout)
}

/// Maps a wire shape name to a live shape kind; `None` for an unknown one.
pub fn parse_live_shape(s: &str) -> Option<LiveShapeKind> {
    Some(match s {
        "line" => LiveShapeKind::Line,
        "rectangle" => LiveShapeKind::Box(BoxShape::Rectangle),
        "rounded_rectangle" => LiveShapeKind::Box(BoxShape::RoundedRectangle),
        "ellipse" => LiveShapeKind::Box(BoxShape::Ellipse),
        "polygon" => LiveShapeKind::Box(BoxShape::Polygon),
        "triangle" => LiveShapeKind::Box(BoxShape::Triangle),
        "diamond" => LiveShapeKind::Box(BoxShape::Diamond),
        "star" => LiveShapeKind::Box(BoxShape::Star),
        "block_arrow" => LiveShapeKind::Box(BoxShape::BlockArrow),
        "heart" => LiveShapeKind::Box(BoxShape::Heart),
        "callout" => LiveShapeKind::Box(BoxShape::Callout),
        _ => return None,
    })
}

fn live_shape_str(kind: LiveShapeKind) -> &'static str {
    match kind {
        LiveShapeKind::Line => "line",
        LiveShapeKind::Box(BoxShape::Rectangle) => "rectangle",
        LiveShapeKind::Box(BoxShape::RoundedRectangle) => "rounded_rectangle",
        LiveShapeKind::Box(BoxShape::Ellipse) => "ellipse",
        LiveShapeKind::Box(BoxShape::Polygon) => "polygon",
        LiveShapeKind::Box(BoxShape::Triangle) => "triangle",
        LiveShapeKind::Box(BoxShape::Diamond) => "diamond",
        LiveShapeKind::Box(BoxShape::Star) => "star",
        LiveShapeKind::Box(BoxShape::BlockArrow) => "block_arrow",
        LiveShapeKind::Box(BoxShape::Heart) => "heart",
        LiveShapeKind::Box(BoxShape::Callout) => "callout",
    }
}

fn shape_mode_str(mode: ShapeMode) -> &'static str {
    match mode {
        ShapeMode::Outline => "outline",
        ShapeMode::Fill => "fill",
        ShapeMode::FillAndOutline => "fill_and_outline",
    }
}

fn dash_str(dash: DashPattern) -> &'static str {
    match dash {
        DashPattern::Solid => "solid",
        DashPattern::Dashed => "dashed",
        DashPattern::Dotted => "dotted",
        DashPattern::DashDot => "dash_dot",
        DashPattern::DashDotDot => "dash_dot_dot",
    }
}

fn gradient_kind_str(kind: GradientKind) -> &'static str {
    match kind {
        GradientKind::Linear => "linear",
        GradientKind::Reflected => "reflected",
        GradientKind::Radial => "radial",
        GradientKind::Conical => "conical",
        GradientKind::Diamond => "diamond",
        GradientKind::SpiralCw => "spiral_cw",
        GradientKind::SpiralCcw => "spiral_ccw",
    }
}

fn gradient_channel_str(channel: GradientChannel) -> &'static str {
    match channel {
        GradientChannel::Color => "color",
        GradientChannel::Transparency => "transparency",
    }
}

fn bytes(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::spec::{CommandSpec, StrokeScratch, run_command};
    use crate::document::Document;

    fn run(bus: &mut CommandBus, json: &str) -> Result<(), crate::DocumentError> {
        let spec: CommandSpec = serde_json::from_str(json).expect("json");
        run_command(bus, spec, &mut StrokeScratch::default(), &[])
    }

    #[test]
    fn live_object_spec_reports_the_wire_parameters_of_a_live_shape() {
        let mut bus = CommandBus::new(Document::new(32, 32).expect("doc"));
        run(
            &mut bus,
            r#"{"type":"begin_live_shape","layer":0,"shape":"rectangle",
                "points":[[2,3],[20,12]],"sides":5,"mode":"fill",
                "stroke_width":2,"stroke_color":[1,2,3,255],
                "fill_color":[4,5,6,255],"anti_alias":true,"dash":"dotted"}"#,
        )
        .expect("begin");
        run(
            &mut bus,
            r#"{"type":"edit_live","gesture":4,"shape":"polygon","points":[[3,3],[21,12]]}"#,
        )
        .expect("edit");
        assert_eq!(
            live_object_spec(&bus),
            Some(LiveObjectSpec::Shape {
                layer: 0,
                shape: "polygon".into(),
                points: [[3.0, 3.0], [21.0, 12.0]],
                corner_radius: 0.0,
                sides: 5,
                star_points: 5,
                inner_ratio: 0.38,
                mode: "fill".into(),
                stroke_width: 2.0,
                stroke_color: [1, 2, 3, 255],
                fill_color: [4, 5, 6, 255],
                anti_alias: true,
                dash: "dotted".into(),
                control: vec![],
                start_cap: "round".into(),
                end_cap: "round".into(),
                polyline: vec![],
                grip_anchor: [0.0, 0.0],
                grip_normal: [0.0, 0.0],
            })
        );
        run(&mut bus, r#"{"type":"commit_live"}"#).expect("commit");
        assert_eq!(live_object_spec(&bus), None);
        assert_eq!(
            run(&mut bus, r#"{"type":"cancel_live"}"#),
            Err(crate::DocumentError::NoLiveObject)
        );
    }

    #[test]
    fn begin_live_on_a_locked_layer_is_refused_like_a_draw() {
        let mut doc = Document::new(8, 8).expect("doc");
        doc.active_layer_mut().locked = true;
        let begin: CommandSpec = serde_json::from_str(
            r#"{"type":"begin_live_gradient","layer":0,"x0":0,"y0":0,"x1":5,"y1":5}"#,
        )
        .expect("json");
        assert!(begin.refused(&doc), "the editor's locked-layer notice");
        let mut bus = CommandBus::new(doc);
        let err = run_command(&mut bus, begin, &mut StrokeScratch::default(), &[]).unwrap_err();
        assert_eq!(err.to_string(), "layer is locked");
        assert_eq!(bus.history.undo_depth(), 0);
    }

    #[test]
    fn live_object_spec_reports_a_live_gradient_and_refuses_shape_fields() {
        let mut bus = CommandBus::new(Document::new(32, 32).expect("doc"));
        run(
            &mut bus,
            r#"{"type":"begin_live_gradient","layer":0,"kind":"radial",
                "channel":"transparency","x0":1,"y0":2,"x1":20,"y1":9,
                "from":[0,0,0,255],"to":[255,255,255,255],"opacity":0.5}"#,
        )
        .expect("begin");
        assert_eq!(
            run(&mut bus, r#"{"type":"edit_live","stroke_width":3}"#),
            Err(crate::DocumentError::LiveEditMismatch)
        );
        run(&mut bus, r#"{"type":"edit_live","kind":"diamond"}"#).expect("kind");
        assert_eq!(
            live_object_spec(&bus),
            Some(LiveObjectSpec::Gradient {
                layer: 0,
                kind: "diamond".into(),
                channel: "transparency".into(),
                points: [[1.0, 2.0], [20.0, 9.0]],
                from: [0, 0, 0, 255],
                to: [255, 255, 255, 255],
                opacity: 0.5,
            })
        );
        run(&mut bus, r#"{"type":"cancel_live"}"#).expect("cancel");
        assert_eq!(bus.history.undo_depth(), 0);
    }

    #[test]
    fn live_object_spec_reports_the_spiral_kinds_by_their_wire_names() {
        // docs/specs/image-palette-gradient-spiral.md: the options bar shows
        // the live gradient's kind, so the spiral names must round-trip.
        let mut bus = CommandBus::new(Document::new(32, 32).expect("doc"));
        run(
            &mut bus,
            r#"{"type":"begin_live_gradient","layer":0,"kind":"spiral_cw",
                "x0":4,"y0":4,"x1":20,"y1":9}"#,
        )
        .expect("begin");
        let kind = |bus: &CommandBus| match live_object_spec(bus) {
            Some(LiveObjectSpec::Gradient { kind, .. }) => kind,
            other => panic!("not a live gradient: {other:?}"),
        };
        assert_eq!(kind(&bus), "spiral_cw");
        run(&mut bus, r#"{"type":"edit_live","kind":"spiral_ccw"}"#).expect("kind");
        assert_eq!(kind(&bus), "spiral_ccw");
    }
}
