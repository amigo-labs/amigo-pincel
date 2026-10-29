//! Image edits: `draw.stroke`, `draw.shape`, `draw.text`, `draw.fill`,
//! `draw.gradient`, `select.*` and `adjust.apply` / `effect.apply`. Each
//! builds the [`CommandSpec`] the editor's tool sends, so the pixels match.

use pincel_effects::{EffectImage, EffectSpec};
use pincel_image_core::command::SetPixels;
use pincel_image_core::command::spec::CommandSpec;
use pincel_image_core::{DEFAULT_BRUSH_ANGLE, ImageBuffer, masked_region};
use serde_json::{Map, Value, json};

use super::{ImageDoc, color_arg, color_prop, core_err};
use crate::args::Args;
use crate::pixel::effects::{ADJUSTMENTS, catalog_summary, is_adjustment, wire_params};
use crate::pixel::{layer_prop, tool_schema};
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// The editor's default brush size, tolerance and shape stroke width.
const BRUSH_SIZE: u64 = 8;
const TOLERANCE: u64 = 32;
const STROKE_WIDTH: f64 = 3.0;
const CORNER_RADIUS: f64 = 16.0;
const POLYGON_SIDES: u64 = 5;
const TEXT_SIZE: f64 = 48.0;

fn num(args: &Args, key: &str) -> Result<f32, AgentError> {
    args.get(key)
        .and_then(Value::as_f64)
        .map(|v| v as f32)
        .ok_or_else(|| AgentError::tool(format!("missing required argument `{key}`")))
}

fn num_or(args: &Args, key: &str, default: f64) -> f32 {
    args.get(key).and_then(Value::as_f64).unwrap_or(default) as f32
}

/// A percentage argument (0–100) as the core's 0–1 fraction.
fn percent(args: &Args, key: &str) -> f32 {
    num_or(args, key, 100.0) / 100.0
}

fn str_or<'a>(args: &'a Args, key: &str, default: &'a str) -> &'a str {
    args.str(key).unwrap_or(default)
}

fn number(description: &str) -> Value {
    json!({ "type": "number", "description": description })
}

fn percent_prop(description: &str) -> Value {
    json!({ "type": "number", "minimum": 0, "maximum": 100, "description": format!("{description} in %; default 100") })
}

fn choice(options: &[&str], description: &str) -> Value {
    json!({ "type": "string", "enum": options, "description": description })
}

/// Schema of an Image edit: `doc`, `layer` and the tool's own properties.
fn edit_schema(properties: Value, required: &[&str]) -> Value {
    let mut props = match properties {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    props.insert("layer".to_owned(), layer_prop());
    tool_schema(Value::Object(props), required)
}

/// Run `build(doc, layer)`'s command on the target layer as one undo step.
fn edit(
    ctx: &mut Ctx<'_>,
    args: &Args,
    build: impl FnOnce(&mut ImageDoc, usize) -> Result<CommandSpec, AgentError>,
) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let layer = doc.target(args)?;
    let spec = build(doc, layer)?;
    let changed = doc.run(spec)?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": doc.layer_id(layer),
        "changed": changed,
    })))
}

fn points_arg(args: &Args) -> Result<Vec<[f32; 2]>, AgentError> {
    let items = args
        .get("points")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentError::tool("missing required argument `points`"))?;
    if items.is_empty() {
        return Err(AgentError::tool("`points` needs at least one [x, y] point"));
    }
    items
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let xy = p.as_array().filter(|a| a.len() == 2);
            let get = |k: usize| xy.and_then(|a| a[k].as_f64()).map(|v| v as f32);
            match (get(0), get(1)) {
                (Some(x), Some(y)) => Ok([x, y]),
                _ => Err(AgentError::tool(format!(
                    "points[{i}] must be an [x, y] pair of numbers"
                ))),
            }
        })
        .collect()
}

const SHAPES: [&str; 3] = ["hard_round", "soft_round", "flat"];

pub(crate) const STROKE: Tool = Tool {
    name: "draw.stroke",
    description: "Image documents: paint a brush stroke through `points` ([[x, y], …], document pixels; one point is a single dab) like the editor's Brush, or erase to transparency with `tool: \"eraser\"`. size 1–500 px (default 8), hardness and opacity in % (default 100), `shape` hard_round / soft_round / flat. Every call is one undo step.",
    schema: || {
        edit_schema(
            json!({
                "points": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "array", "minItems": 2, "maxItems": 2, "items": { "type": "number" } },
                    "description": "Stroke path as [x, y] pairs",
                },
                "tool": choice(&["brush", "eraser"], "brush (default) or eraser"),
                "color": color_prop("Brush color; default #000000"),
                "size": { "type": "integer", "minimum": 1, "maximum": 500, "description": "Brush diameter in px; default 8" },
                "hardness": percent_prop("Edge hardness"),
                "opacity": percent_prop("Stroke opacity"),
                "shape": choice(&SHAPES, "Brush tip; default hard_round"),
            }),
            &["points"],
        )
    },
    run: |ctx, args| {
        edit(ctx, args, |doc, layer| {
            let points = points_arg(args)?;
            let size = args.i64("size").map_or(BRUSH_SIZE, |s| s as u64) as u32;
            let (opacity, hardness) = (percent(args, "opacity"), percent(args, "hardness"));
            let shape = str_or(args, "shape", "hard_round").to_owned();
            let stroke_id = doc.next_stroke();
            Ok(if args.str("tool") == Some("eraser") {
                CommandSpec::EraserStroke {
                    layer,
                    size,
                    opacity,
                    shape,
                    hardness,
                    mode: "to_transparent".to_owned(),
                    background: [255, 255, 255, 255],
                    points,
                    pressure: Vec::new(),
                    pressure_size: false,
                    pressure_opacity: false,
                    azimuth: Vec::new(),
                    angle: DEFAULT_BRUSH_ANGLE,
                    tilt_angle: false,
                    stroke_id,
                }
            } else {
                let color = match args.get("color") {
                    Some(_) => color_arg(args, "color")?,
                    None => [0, 0, 0, 255],
                };
                CommandSpec::PencilStroke {
                    layer,
                    size,
                    color,
                    opacity,
                    shape,
                    hardness,
                    points,
                    pressure: Vec::new(),
                    pressure_size: false,
                    pressure_opacity: false,
                    azimuth: Vec::new(),
                    angle: DEFAULT_BRUSH_ANGLE,
                    tilt_angle: false,
                    stroke_id,
                }
            })
        })
    },
};

pub(crate) const SHAPE: Tool = Tool {
    name: "draw.shape",
    description: "Image documents: draw a shape like the editor's shape tools. `shape`: rect, rounded (corner_radius, default 16), ellipse (inscribed in the box) and line take the corners / endpoints (x0, y0) and (x1, y1); polygon takes the center (cx, cy), radius, sides (default 5) and rotation in degrees. Give `stroke` (outline color, stroke_width default 3 px, dash solid / dashed / dotted), `fill` (fill color) or both. anti_alias defaults to true.",
    schema: || {
        edit_schema(
            json!({
                "shape": choice(&["rect", "rounded", "ellipse", "polygon", "line"], "Shape kind"),
                "x0": number("First corner / start x"),
                "y0": number("First corner / start y"),
                "x1": number("Opposite corner / end x"),
                "y1": number("Opposite corner / end y"),
                "corner_radius": { "type": "number", "minimum": 0, "description": "rounded: corner radius in px; default 16" },
                "cx": number("polygon: center x"),
                "cy": number("polygon: center y"),
                "radius": { "type": "number", "minimum": 0, "description": "polygon: circumradius in px" },
                "sides": { "type": "integer", "minimum": 3, "maximum": 100, "description": "polygon: number of sides; default 5" },
                "rotation": number("polygon: rotation in degrees; default 0"),
                "stroke": color_prop("Outline color"),
                "fill": color_prop("Fill color"),
                "stroke_width": { "type": "number", "minimum": 1, "maximum": 500, "description": "Outline width in px; default 3" },
                "dash": choice(&["solid", "dashed", "dotted"], "Outline dash pattern; default solid"),
                "anti_alias": { "type": "boolean", "description": "Smooth edges; default true" },
            }),
            &["shape"],
        )
    },
    run: |ctx, args| {
        edit(ctx, args, |_, layer| {
            let kind = args.req_str("shape")?;
            let stroke = args
                .get("stroke")
                .map(|_| color_arg(args, "stroke"))
                .transpose()?;
            let fill = args
                .get("fill")
                .map(|_| color_arg(args, "fill"))
                .transpose()?;
            let mode = match (stroke, fill) {
                (Some(_), Some(_)) => "fill_and_outline",
                (None, Some(_)) => "fill",
                (Some(_), None) => "outline",
                (None, None) => {
                    return Err(AgentError::tool(
                        "draw.shape needs a `stroke` color, a `fill` color or both",
                    ));
                }
            };
            let (shape, points, center, radius, sides, rotation) = if kind == "polygon" {
                let center = [num(args, "cx")?, num(args, "cy")?];
                let sides = args.i64("sides").map_or(POLYGON_SIDES, |s| s as u64) as u32;
                let rotation = num_or(args, "rotation", 0.0).to_radians();
                (
                    "polygon",
                    Vec::new(),
                    center,
                    num(args, "radius")?,
                    sides,
                    rotation,
                )
            } else {
                let core = match kind {
                    "rect" => "rectangle",
                    "rounded" => "rounded_rectangle",
                    other => other,
                };
                let pts = vec![
                    [num(args, "x0")?, num(args, "y0")?],
                    [num(args, "x1")?, num(args, "y1")?],
                ];
                (core, pts, [0.0, 0.0], 0.0, 0, 0.0)
            };
            Ok(CommandSpec::DrawShape {
                layer,
                shape: shape.to_owned(),
                points,
                corner_radius: num_or(args, "corner_radius", CORNER_RADIUS),
                center,
                radius,
                sides,
                rotation,
                mode: mode.to_owned(),
                stroke_width: num_or(args, "stroke_width", STROKE_WIDTH),
                stroke_color: stroke.unwrap_or_default(),
                fill_color: fill.unwrap_or_default(),
                anti_alias: args.bool("anti_alias").unwrap_or(true),
                dash: str_or(args, "dash", "solid").to_owned(),
            })
        })
    },
};

pub(crate) const TEXT: Tool = Tool {
    name: "draw.text",
    description: "Image documents: write `text` (newlines start new lines) with the editor's Text tool and its bundled font (Liberation Sans). (x, y) is the top of the first line: its left edge, center or right edge by `align`. size in px (default 48), color (default #000000), bold, italic, anti_alias (default true).",
    schema: || {
        edit_schema(
            json!({
                "text": { "type": "string", "description": "The text" },
                "x": number("Anchor x (see align)"),
                "y": number("Top of the first line"),
                "size": { "type": "number", "minimum": 1, "maximum": 999, "description": "Font size in px; default 48" },
                "color": color_prop("Text color; default #000000"),
                "align": choice(&["left", "center", "right"], "Horizontal alignment; default left"),
                "bold": { "type": "boolean" },
                "italic": { "type": "boolean" },
                "anti_alias": { "type": "boolean", "description": "Smooth glyph edges; default true" },
            }),
            &["text", "x", "y"],
        )
    },
    run: |ctx, args| {
        edit(ctx, args, |_, layer| {
            let color = match args.get("color") {
                Some(_) => color_arg(args, "color")?,
                None => [0, 0, 0, 255],
            };
            Ok(CommandSpec::DrawText {
                layer,
                font_id: 0,
                text: args.req_str("text")?.to_owned(),
                x: num(args, "x")?,
                y: num(args, "y")?,
                size: num_or(args, "size", TEXT_SIZE),
                color,
                bold: args.bool("bold").unwrap_or(false),
                italic: args.bool("italic").unwrap_or(false),
                anti_alias: args.bool("anti_alias").unwrap_or(true),
                align: str_or(args, "align", "left").to_owned(),
            })
        })
    },
};

/// `draw.fill` on an Image document: the editor's Bucket tool.
pub(crate) fn fill(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    edit(ctx, args, |_, layer| {
        let tolerance = args.i64("tolerance").map_or(TOLERANCE, |t| t as u64);
        Ok(CommandSpec::FillBucket {
            layer,
            color: color_arg(args, "color")?,
            opacity: percent(args, "opacity"),
            tolerance: u8::try_from(tolerance)
                .map_err(|_| AgentError::tool("tolerance must be 0–255"))?,
            contiguous: args.bool("contiguous").unwrap_or(true),
            sample: "current_layer".to_owned(),
            x: num(args, "x")?,
            y: num(args, "y")?,
        })
    })
}

pub(crate) const GRADIENT: Tool = Tool {
    name: "draw.gradient",
    description: "Image documents: paint a gradient along the drag (x0, y0) → (x1, y1) from color `from` to color `to`, like the editor's Gradient tool. kind linear (default) / reflected / radial / conical / diamond; channel color (default) or transparency (fades existing alpha); opacity in %. Respects the selection.",
    schema: || {
        edit_schema(
            json!({
                "x0": number("Start x"),
                "y0": number("Start y"),
                "x1": number("End x"),
                "y1": number("End y"),
                "from": color_prop("Start color"),
                "to": color_prop("End color"),
                "kind": choice(&["linear", "reflected", "radial", "conical", "diamond"], "Gradient shape; default linear"),
                "channel": choice(&["color", "transparency"], "What the gradient paints; default color"),
                "opacity": percent_prop("Gradient opacity"),
            }),
            &["x0", "y0", "x1", "y1", "from", "to"],
        )
    },
    run: |ctx, args| {
        edit(ctx, args, |_, layer| {
            Ok(CommandSpec::DrawGradient {
                layer,
                kind: str_or(args, "kind", "linear").to_owned(),
                channel: str_or(args, "channel", "color").to_owned(),
                x0: num(args, "x0")?,
                y0: num(args, "y0")?,
                x1: num(args, "x1")?,
                y1: num(args, "y1")?,
                from: color_arg(args, "from")?,
                to: color_arg(args, "to")?,
                opacity: percent(args, "opacity"),
            })
        })
    },
};

const MODES: [&str; 4] = ["replace", "add", "subtract", "intersect"];

fn selection_schema(wand: bool) -> Value {
    let mut props = if wand {
        json!({
            "x": number("Seed x"),
            "y": number("Seed y"),
            "tolerance": { "type": "integer", "minimum": 0, "maximum": 255, "description": "Color distance still selected; default 32" },
            "contiguous": { "type": "boolean", "description": "Only the connected area; default true" },
            "sample": choice(&["current_layer", "all_layers"], "Color source; default current_layer"),
            "layer": layer_prop(),
        })
    } else {
        json!({
            "x": { "type": "integer", "description": "Left" },
            "y": { "type": "integer", "description": "Top" },
            "width": { "type": "integer", "minimum": 1 },
            "height": { "type": "integer", "minimum": 1 },
            "feather": { "type": "integer", "minimum": 0, "maximum": 250, "description": "Soft edge radius in px; default 0" },
        })
    };
    props["mode"] = choice(
        &MODES,
        "How it combines with the current selection; default replace",
    );
    let required: &[&str] = if wand {
        &["x", "y"]
    } else {
        &["x", "y", "width", "height"]
    };
    tool_schema(props, required)
}

/// Run a selection command (an undo step in Image mode, as in the editor).
fn select(
    ctx: &mut Ctx<'_>,
    args: &Args,
    build: impl FnOnce(&ImageDoc) -> Result<CommandSpec, AgentError>,
) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let spec = build(doc)?;
    doc.run(spec)?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "selection": doc.info(&id)["selection"] }),
    ))
}

fn box_selection(args: &Args, ellipse: bool) -> Result<CommandSpec, AgentError> {
    let (x, y) = (args.req_i32("x")?, args.req_i32("y")?);
    let (w, h) = (args.req_u32("width")?, args.req_u32("height")?);
    let mode = str_or(args, "mode", "replace").to_owned();
    let feather = args.u32("feather")?.unwrap_or(0);
    Ok(if ellipse {
        CommandSpec::SelectEllipse {
            x,
            y,
            w,
            h,
            mode,
            feather,
        }
    } else {
        CommandSpec::SelectRectangle {
            x,
            y,
            w,
            h,
            mode,
            feather,
        }
    })
}

pub(crate) const SELECT_RECT: Tool = Tool {
    name: "select.rect",
    description: "Image documents: select a rectangle like the editor's Rectangle Select (`feather` softens the edge; `mode` combines with the current selection). While a selection exists, draw.fill, draw.gradient, adjust.apply and effect.apply change only the selected pixels, blending feathered edges by coverage. One undo step.",
    schema: || selection_schema(false),
    run: |ctx, args| select(ctx, args, |_| box_selection(args, false)),
};

pub(crate) const SELECT_ELLIPSE: Tool = Tool {
    name: "select.ellipse",
    description: "Image documents: select the ellipse inscribed in the rectangle (x, y, width, height) like the editor's Ellipse Select; `feather` and `mode` as for select.rect. One undo step.",
    schema: || selection_schema(false),
    run: |ctx, args| select(ctx, args, |_| box_selection(args, true)),
};

pub(crate) const SELECT_WAND: Tool = Tool {
    name: "select.wand",
    description: "Image documents: select the area around the seed (x, y) within `tolerance` (default 32) of its color, like the editor's Magic Wand (on the target `layer` or all layers with `sample`). On a uniform layer it selects the whole canvas. One undo step.",
    schema: || selection_schema(true),
    run: |ctx, args| {
        select(ctx, args, |doc| {
            let layer = match args.u32("layer")? {
                Some(id) => doc.layer_index(id)?,
                None => doc.document().active_layer_index(),
            };
            let tolerance = args.i64("tolerance").map_or(TOLERANCE, |t| t as u64);
            Ok(CommandSpec::SelectWand {
                layer,
                x: num(args, "x")?,
                y: num(args, "y")?,
                tolerance: u8::try_from(tolerance)
                    .map_err(|_| AgentError::tool("tolerance must be 0–255"))?,
                contiguous: args.bool("contiguous").unwrap_or(true),
                sample: str_or(args, "sample", "current_layer").to_owned(),
                mode: str_or(args, "mode", "replace").to_owned(),
            })
        })
    },
};

pub(crate) const SELECT_CLEAR: Tool = Tool {
    name: "select.clear",
    description: "Image documents: drop the selection (the editor's Deselect); edits act on the whole layer again. One undo step.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| select(ctx, args, |_| Ok(CommandSpec::Deselect)),
};

/// The `pincel_effects` spec for `id` and the named `params`, checked
/// against the catalog ranges.
fn effect_spec(args: &Args) -> Result<EffectSpec, AgentError> {
    let id = args.req_str("id")?;
    let empty = Map::new();
    let params = args
        .get("params")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let wire = wire_params(id, params)?;
    EffectSpec::from_params(id, &wire).map_err(|e| AgentError::tool(e.to_string()))
}

/// `effect.apply` / `adjust.apply` on an Image document: the editor's
/// `apply_effect` — run on the target layer, keep the result inside the
/// selection by coverage, commit as one `SetPixels`.
pub(crate) fn effect(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let spec = effect_spec(args)?;
    let (id, doc) = ctx.ws.image_mut(args)?;
    let layer = doc.target(args)?;
    let document = doc.document();
    let before = &document.layers()[layer].pixels;
    let (w, h) = (before.width(), before.height());
    let src = EffectImage::from_rgba8(w, h, before.data().to_vec())
        .map_err(|e| AgentError::tool(e.to_string()))?;
    let after = ImageBuffer::from_raw(w, h, spec.run(&src).into_raw()).map_err(core_err)?;
    let changed = match masked_region(before, &after, document.selection.as_ref()) {
        Some((rect, pixels)) => {
            let cmd = SetPixels::new(layer, rect, pixels).with_label(spec.label());
            doc.apply(vec![Box::new(cmd)])?
        }
        None => false,
    };
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": doc.layer_id(layer),
        "changed": changed,
    })))
}

pub(crate) const ADJUST_APPLY: Tool = Tool {
    name: "adjust.apply",
    description: "Apply an adjustment from the editor's Adjust menu (brightness_contrast, hue_saturation, curves, levels, color_balance, invert, grayscale, posterize, threshold) to the target layer, exactly as the editor does; respects the selection. `params` maps parameter names to values (see pincel://effects); omitted ones use their defaults. Effects (blur, sharpen, …) go through effect.apply.",
    schema: || {
        let mut props = json!({
            "id": { "type": "string", "description": format!("Adjustment id: {}", ADJUSTMENTS.join(", ")) },
            "params": { "type": "object", "description": format!("Named parameters. Catalog: {}", catalog_summary()) },
            "layer": layer_prop(),
        });
        props["frame"] = crate::pixel::frame_prop();
        tool_schema(props, &["id"])
    },
    run: |ctx, args| {
        let id = args.req_str("id")?;
        if !is_adjustment(id) {
            return Err(AgentError::tool(format!(
                "`{id}` is not an adjustment (adjustments: {}); use effect.apply for effects",
                ADJUSTMENTS.join(", ")
            )));
        }
        if ctx.ws.is_image(args)? {
            effect(ctx, args)
        } else {
            crate::pixel::apply_effect(ctx, args)
        }
    },
};
