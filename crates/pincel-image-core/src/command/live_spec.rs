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
        mode: String,
        stroke_width: f32,
        stroke_color: [u8; 4],
        fill_color: [u8; 4],
        anti_alias: bool,
        dash: String,
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
        LiveParams::Shape(s) => LiveObjectSpec::Shape {
            layer: live.layer,
            shape: live_shape_str(s.kind).to_string(),
            points,
            corner_radius: s.corner_radius,
            sides: s.sides,
            mode: shape_mode_str(s.style.mode).to_string(),
            stroke_width: s.style.stroke_width,
            stroke_color: bytes(s.style.stroke_color),
            fill_color: bytes(s.style.fill_color),
            anti_alias: s.style.anti_alias,
            dash: dash_str(s.style.dash).to_string(),
        },
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
    }
}

fn gradient_kind_str(kind: GradientKind) -> &'static str {
    match kind {
        GradientKind::Linear => "linear",
        GradientKind::Reflected => "reflected",
        GradientKind::Radial => "radial",
        GradientKind::Conical => "conical",
        GradientKind::Diamond => "diamond",
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
                mode: "fill".into(),
                stroke_width: 2.0,
                stroke_color: [1, 2, 3, 255],
                fill_color: [4, 5, 6, 255],
                anti_alias: true,
                dash: "dotted".into(),
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
}
