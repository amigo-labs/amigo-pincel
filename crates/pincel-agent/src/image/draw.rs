//! Image edits: `draw.stroke`, `draw.shape`, `draw.text`, `draw.fill`,
//! `draw.gradient`, `select.*` and `adjust.apply` / `effect.apply`. Each
//! builds the [`CommandSpec`] the editor's tool sends, so the pixels match.

use pincel_effects::{EffectImage, EffectSpec};
use pincel_image_core::command::SetPixels;
use pincel_image_core::command::spec::CommandSpec;
use pincel_image_core::{DEFAULT_BRUSH_ANGLE, ImageBuffer, masked_region};
use serde_json::{Map, Value, json};

use super::{color_arg, core_err};
use crate::args::Args;
use crate::pixel::effects::{ADJUSTMENTS, catalog_summary, is_adjustment, wire_params};
use crate::pixel::{layer_prop, tool_schema};
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// The editor's default brush size, tolerance and shape stroke width.
pub(super) const BRUSH_SIZE: u64 = 8;
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

pub(super) fn num_or(args: &Args, key: &str, default: f64) -> f32 {
    args.get(key).and_then(Value::as_f64).unwrap_or(default) as f32
}

/// A percentage argument (0–100) as the core's 0–1 fraction.
pub(super) fn percent(args: &Args, key: &str) -> f32 {
    num_or(args, key, 100.0) / 100.0
}

pub(super) fn str_or<'a>(args: &'a Args, key: &str, default: &'a str) -> &'a str {
    args.str(key).unwrap_or(default)
}

fn number(description: &str) -> Value {
    json!({ "type": "number", "description": description })
}

fn percent_prop(description: &str) -> Value {
    json!({ "type": "number", "minimum": 0, "maximum": 100, "description": format!("{description} in %; default 100") })
}

pub(super) fn choice(options: &[&str], description: &str) -> Value {
    json!({ "type": "string", "enum": options, "description": description })
}

/// Schema of an Image edit: `doc`, `layer` and the tool's own properties.
pub(super) fn edit_schema(properties: Value, required: &[&str]) -> Value {
    let mut props = match properties {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    props.insert("layer".to_owned(), layer_prop());
    tool_schema(Value::Object(props), required)
}

/// A number argument within the Image editor's range `lo..=hi` (no upper
/// bound when `hi` is `None`), else `default`. The schemas carry no bounds
/// for arguments whose range differs per mode, so the message can name the
/// mode (mcp-parity-drawing AC-24).
fn image_ranged(
    args: &Args,
    key: &str,
    lo: f64,
    hi: Option<f64>,
    default: f64,
) -> Result<f32, AgentError> {
    let Some(v) = args.get(key) else {
        return Ok(default as f32);
    };
    match v.as_f64() {
        Some(n) if n >= lo && hi.is_none_or(|hi| n <= hi) => Ok(n as f32),
        _ => Err(AgentError::tool(match hi {
            Some(hi) => format!("`{key}` must be {lo}–{hi} on Image documents"),
            None => format!("`{key}` must be at least {lo} on Image documents"),
        })),
    }
}

/// Refuse arguments only the Pixel editor offers (mcp-parity-drawing
/// AC-18).
fn refuse_pixel_args(ctx: &Ctx<'_>, args: &Args, keys: &[&str]) -> Result<(), AgentError> {
    if let Some(k) = keys.iter().find(|k| args.get(k).is_some()) {
        let id = ctx.ws.resolve(args)?;
        return Err(AgentError::tool(format!(
            "argument `{k}` works on Pixel documents only; document `{id}` is an Image document"
        )));
    }
    Ok(())
}

/// A shared color argument: Image documents take `#RRGGBB` /
/// `#RRGGBBAA`, Pixel documents also `{"index": n}`.
fn shared_color(description: &str) -> Value {
    crate::args::color_schema(description)
}

/// Run the command `build` makes on the target layer; a call that
/// changes no pixel records no undo step:
/// `build` runs twice (a dry run on a copy decides), with the same stroke
/// id (mcp-parity-drawing-image AC-13).
pub(super) fn edit_checked(
    ctx: &mut Ctx<'_>,
    args: &Args,
    build: impl Fn(usize, u64) -> Result<CommandSpec, AgentError>,
) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let layer = doc.target(args)?;
    build(layer, 0)?;
    let stroke = doc.next_stroke();
    let changed = doc.run_if_changed(|| {
        build(layer, stroke).expect("the arguments were checked by the first build")
    })?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": doc.layer_id(layer),
        "changed": changed,
    })))
}

pub(super) fn points_arg(args: &Args) -> Result<Vec<[f32; 2]>, AgentError> {
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

pub(crate) const STROKE: Tool = Tool {
    name: "draw.stroke",
    description: "Paint a stroke through `points` ([[x, y], …]; one point is a single stamp) with `tool` brush (default), or erase with `tool: \"eraser\"`; `color` default #000000; every call is one undo step. `layer` and `frame` (Pixel only) default to the paint target. Image documents (the Brush): `size` 1–500 px (default 8), `hardness` and `opacity` in % (default 100), `shape` hard_round (default) / soft_round / flat, `angle` 0–180 degrees of the tip (default 45); the eraser takes `eraser_mode` to_transparent (default) / to_background, which paints `background` (default #FFFFFF). Pixel documents (the Pencil): integer points, `size` 1–8 (default 1), `shape` square (default) / round, `pixel_perfect` (default false), `ink` simple (default) / shading with `ramp_from` / `ramp_to` (palette indexes, default the whole palette) and `step` 1 (default) / -1, symmetry `mirror` x / y / xy (default none) with `axis_x2` / `axis_y2` (half pixels, default the centre), tiled `wrap` x / y / xy (default none).",
    schema: || {
        edit_schema(
            json!({
                "points": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "array", "minItems": 2, "maxItems": 2, "items": { "type": "number" } },
                    "description": "Stroke path as [x, y] pairs",
                },
                "tool": choice(&["brush", "eraser"], "brush (default) or eraser; recoloring and smudging are the tools draw.recolor and draw.smudge"),
                "color": shared_color("Brush color; default #000000"),
                "size": { "type": "integer", "description": "Brush size in px: Image 1–500 (default 8), Pixel 1–8 (default 1)" },
                "hardness": percent_prop("Image: edge hardness"),
                "opacity": percent_prop("Image: stroke opacity"),
                "shape": choice(&["hard_round", "soft_round", "flat", "square", "round"], "Brush tip: Image hard_round (default) / soft_round / flat; Pixel square (default) / round"),
                "pixel_perfect": { "type": "boolean", "description": "Pixel: drop L-shaped corners of 1-px strokes; default false" },
                "ink": choice(&["simple", "shading"], "Pixel: simple (default) or shading"),
                "ramp_from": { "type": "integer", "minimum": 0, "description": "Pixel shading: first palette index of the ramp" },
                "ramp_to": { "type": "integer", "minimum": 0, "description": "Pixel shading: last palette index of the ramp (inclusive)" },
                "step": { "type": "integer", "description": "Pixel shading: 1 (toward the ramp's end, default) or -1" },
                "mirror": choice(&["x", "y", "xy"], "Pixel: symmetry left-right (x), top-bottom (y) or both"),
                "axis_x2": { "type": "integer", "description": "Pixel: vertical symmetry axis in half pixels from the left edge; default the canvas width (the centre)" },
                "axis_y2": { "type": "integer", "description": "Pixel: horizontal symmetry axis in half pixels from the top edge; default the canvas height" },
                "wrap": choice(&["x", "y", "xy"], "Pixel: tiled mode, wrap across the canvas edges"),
                "eraser_mode": choice(&["to_transparent", "to_background"], "Image eraser: erase to transparency (default) or paint `background`"),
                "background": shared_color("Image eraser with to_background: the color; default #FFFFFF"),
                "angle": angle_prop(),
                "frame": crate::pixel::frame_prop(),
            }),
            &["points"],
        )
    },
    run: |ctx, args| {
        if !ctx.ws.is_image(args)? {
            return crate::pixel::paint::stroke(ctx, args);
        }
        refuse_pixel_args(
            ctx,
            args,
            &[
                "pixel_perfect",
                "ink",
                "ramp_from",
                "ramp_to",
                "step",
                "mirror",
                "axis_x2",
                "axis_y2",
                "wrap",
            ],
        )?;
        if let Some(s @ ("square" | "round")) = args.str("shape") {
            let id = ctx.ws.resolve(args)?;
            return Err(AgentError::tool(format!(
                "argument `shape`: {s:?} works on Pixel documents only; document `{id}` is an Image document (hard_round, soft_round, flat)"
            )));
        }
        edit_checked(ctx, args, |layer, stroke_id| {
            let points = points_arg(args)?;
            let size = image_ranged(args, "size", 1.0, Some(500.0), BRUSH_SIZE as f64)? as u32;
            let (opacity, hardness) = (percent(args, "opacity"), percent(args, "hardness"));
            let shape = str_or(args, "shape", "hard_round").to_owned();
            Ok(if args.str("tool") == Some("eraser") {
                CommandSpec::EraserStroke {
                    layer,
                    size,
                    opacity,
                    shape,
                    hardness,
                    mode: str_or(args, "eraser_mode", "to_transparent").to_owned(),
                    background: match args.get("background") {
                        Some(_) => color_arg(args, "background")?,
                        None => [255, 255, 255, 255],
                    },
                    points,
                    pressure: Vec::new(),
                    pressure_size: false,
                    pressure_opacity: false,
                    azimuth: Vec::new(),
                    angle: num_or(args, "angle", f64::from(DEFAULT_BRUSH_ANGLE)),
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
                    angle: num_or(args, "angle", f64::from(DEFAULT_BRUSH_ANGLE)),
                    tilt_angle: false,
                    stroke_id,
                }
            })
        })
    },
};

fn angle_prop() -> Value {
    json!({ "type": "number", "minimum": 0, "maximum": 180, "description": "Image: flat-nib angle in degrees 0–180; default 45 (a round tip ignores it)" })
}

/// The brush arguments `draw.recolor` and `draw.smudge` share with Image
/// `draw.stroke`.
pub(super) fn brush_props() -> Value {
    json!({
        "points": {
            "type": "array",
            "minItems": 1,
            "items": { "type": "array", "minItems": 2, "maxItems": 2, "items": { "type": "number" } },
            "description": "Stroke path as [x, y] pairs",
        },
        "size": { "type": "integer", "minimum": 1, "maximum": 500, "description": "Brush size in px 1–500; default 8" },
        "hardness": percent_prop("Edge hardness"),
        "opacity": percent_prop("Stroke opacity"),
        "shape": choice(&["hard_round", "soft_round", "flat"], "Brush tip; default hard_round"),
        "angle": angle_prop(),
    })
}

pub(super) fn with_props(mut base: Value, extra: Value) -> Value {
    if let (Some(b), Value::Object(e)) = (base.as_object_mut(), extra) {
        b.extend(e);
    }
    base
}

pub(crate) const RECOLOR: Tool = Tool {
    name: "draw.recolor",
    description: "Image documents: the editor's Recolor tool along `points`: pixels within `tolerance` (0–255, default 32) of the color `from` become `color` (default #000000), with the brush arguments of draw.stroke (size default 8, hardness, opacity, shape, angle). Respects the selection. One undo step.",
    schema: || {
        edit_schema(
            with_props(
                brush_props(),
                json!({
                    "from": color_prop_image("The color to replace"),
                    "color": color_prop_image("The replacement; default #000000"),
                    "tolerance": { "type": "integer", "minimum": 0, "maximum": 255, "description": "Color distance still replaced; default 32" },
                }),
            ),
            &["points", "from"],
        )
    },
    run: |ctx, args| {
        edit_checked(ctx, args, |layer, stroke_id| {
            let points = points_arg(args)?;
            let from = color_arg(args, "from")?;
            let to = match args.get("color") {
                Some(_) => color_arg(args, "color")?,
                None => [0, 0, 0, 255],
            };
            Ok(CommandSpec::RecolorStroke {
                layer,
                size: args.i64("size").map_or(BRUSH_SIZE, |s| s as u64) as u32,
                opacity: percent(args, "opacity"),
                shape: str_or(args, "shape", "hard_round").to_owned(),
                hardness: percent(args, "hardness"),
                from,
                to,
                tolerance: args.i64("tolerance").map_or(TOLERANCE, |t| t as u64) as u8,
                points,
                pressure: Vec::new(),
                pressure_size: false,
                pressure_opacity: false,
                azimuth: Vec::new(),
                angle: num_or(args, "angle", f64::from(DEFAULT_BRUSH_ANGLE)),
                tilt_angle: false,
                stroke_id,
            })
        })
    },
};

pub(crate) const SMUDGE: Tool = Tool {
    name: "draw.smudge",
    description: "Image documents: the editor's Smudge tool along `points`: drags color along the path with `strength` 1–100 % (default 60) and the brush arguments of draw.stroke (size default 8, hardness, opacity, shape, angle). Respects the selection. One undo step.",
    schema: || {
        edit_schema(
            with_props(
                brush_props(),
                json!({
                    "strength": { "type": "number", "minimum": 1, "maximum": 100, "description": "How much color is carried, in %; default 60" },
                }),
            ),
            &["points"],
        )
    },
    run: |ctx, args| {
        edit_checked(ctx, args, |layer, stroke_id| {
            let points = points_arg(args)?;
            Ok(CommandSpec::SmudgeStroke {
                layer,
                size: args.i64("size").map_or(BRUSH_SIZE, |s| s as u64) as u32,
                opacity: percent(args, "opacity"),
                shape: str_or(args, "shape", "hard_round").to_owned(),
                hardness: percent(args, "hardness"),
                strength: num_or(args, "strength", 60.0) / 100.0,
                points,
                pressure: Vec::new(),
                pressure_size: false,
                pressure_opacity: false,
                azimuth: Vec::new(),
                angle: num_or(args, "angle", f64::from(DEFAULT_BRUSH_ANGLE)),
                tilt_angle: false,
                stroke_id,
            })
        })
    },
};

/// An Image-only color argument: `#RRGGBB` or `#RRGGBBAA` (the schema
/// also admits the Pixel `{index}` form, so its use gets a named error;
/// the description names only the forms Image documents take).
fn color_prop_image(description: &str) -> Value {
    let mut schema = crate::args::color_schema(description);
    schema["description"] = json!(format!("{description}: \"#RRGGBB\" or \"#RRGGBBAA\""));
    schema
}

pub(crate) const SHAPE: Tool = Tool {
    name: "draw.shape",
    description: "Draw a shape like the editor's shape tools. Give `stroke` (outline color), `fill` (fill color) or both. `layer` and `frame` (Pixel only) default to the paint target. Image documents: `shape` rect, rounded, ellipse or line takes the corners / endpoints (x0, y0) and (x1, y1) (the ellipse inscribed in the box), with `corner_radius` (default 16) for rounded; polygon takes the center (`cx`, `cy`), `radius`, `sides` 3–100 (default 5) and `rotation` in degrees (default 0); `stroke_width` 1–500 (default 3), `dash` solid (default) / dashed / dotted, `anti_alias` (default true). Pixel documents: `shape` rect, rounded, ellipse or polygon over the integer corners (x0, y0)–(x1, y1), with `corner_radius` 0–256 (default 4) for rounded and `sides` 3–64 (default 6) for the polygon (inscribed in the box, first vertex up); `stroke_width` 1–64 (default 1); no line (use draw.line), rotation, dash or anti-aliasing.",
    schema: || {
        edit_schema(
            json!({
                "shape": choice(&["rect", "rounded", "ellipse", "polygon", "line"], "Shape kind"),
                "x0": number("First corner / start x"),
                "y0": number("First corner / start y"),
                "x1": number("Opposite corner / end x"),
                "y1": number("Opposite corner / end y"),
                "corner_radius": { "type": "number", "description": "rounded: corner radius in px; Image at least 0 (default 16), Pixel 0–256 (default 4)" },
                "cx": number("polygon: center x"),
                "cy": number("polygon: center y"),
                "radius": { "type": "number", "minimum": 0, "description": "polygon: circumradius in px" },
                "sides": { "type": "integer", "description": "polygon: number of sides; Image 3–100 (default 5), Pixel 3–64 (default 6)" },
                "rotation": number("polygon: rotation in degrees; default 0"),
                "stroke": shared_color("Outline color"),
                "fill": shared_color("Fill color"),
                "stroke_width": { "type": "number", "description": "Outline width in px; Image 1–500 (default 3), Pixel 1–64 (default 1)" },
                "dash": choice(&["solid", "dashed", "dotted"], "Image: outline dash pattern; default solid"),
                "anti_alias": { "type": "boolean", "description": "Image: smooth edges; default true" },
                "frame": crate::pixel::frame_prop(),
            }),
            &["shape"],
        )
    },
    run: |ctx, args| {
        if !ctx.ws.is_image(args)? {
            return crate::pixel::paint::shape(ctx, args);
        }
        edit_checked(ctx, args, |layer, _| {
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
                let sides =
                    image_ranged(args, "sides", 3.0, Some(100.0), POLYGON_SIDES as f64)? as u32;
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
            let stroke_width = image_ranged(args, "stroke_width", 1.0, Some(500.0), STROKE_WIDTH)?;
            let corner_radius = if kind == "rounded" {
                image_ranged(args, "corner_radius", 0.0, None, CORNER_RADIUS)?
            } else {
                num_or(args, "corner_radius", CORNER_RADIUS)
            };
            Ok(CommandSpec::DrawShape {
                layer,
                shape: shape.to_owned(),
                points,
                corner_radius,
                center,
                radius,
                sides,
                rotation,
                mode: mode.to_owned(),
                stroke_width,
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
    description: "Write `text` (newlines start new lines) with the editor's Text tool and its bundled font (Liberation Sans). (x, y) is the top of the first line: its left edge, center or right edge by `align` left (default) / center / right. `color` default #000000, `bold` and `italic` (default false). `layer` and `frame` (Pixel only) default to the paint target. Image documents: `underline` and `strikethrough` (default false); `size` 1–999 px (default 48), `anti_alias` (default true). Pixel documents: integer x, y, `size` 1–2000 (default 12), `anti_alias` (default false); the text is clipped to the selection.",
    schema: || {
        edit_schema(
            json!({
                "text": { "type": "string", "description": "The text" },
                "x": number("Anchor x (see align)"),
                "y": number("Top of the first line"),
                "size": { "type": "number", "description": "Font size in px; Image 1–999 (default 48), Pixel 1–2000 (default 12)" },
                "color": shared_color("Text color; default #000000"),
                "align": choice(&["left", "center", "right"], "Horizontal alignment; default left"),
                "bold": { "type": "boolean" },
                "italic": { "type": "boolean" },
                "anti_alias": { "type": "boolean", "description": "Smooth glyph edges; default true (Image) / false (Pixel)" },
                "underline": { "type": "boolean", "description": "Image: a stroke below each line; default false" },
                "strikethrough": { "type": "boolean", "description": "Image: a stroke through each line; default false" },
                "frame": crate::pixel::frame_prop(),
            }),
            &["text", "x", "y"],
        )
    },
    run: |ctx, args| {
        if !ctx.ws.is_image(args)? {
            return crate::pixel::paint::text(ctx, args);
        }
        let size = image_ranged(args, "size", 1.0, Some(999.0), TEXT_SIZE)?;
        edit_checked(ctx, args, |layer, _| {
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
                size,
                color,
                bold: args.bool("bold").unwrap_or(false),
                italic: args.bool("italic").unwrap_or(false),
                anti_alias: args.bool("anti_alias").unwrap_or(true),
                align: str_or(args, "align", "left").to_owned(),
                underline: args.bool("underline").unwrap_or(false),
                strikethrough: args.bool("strikethrough").unwrap_or(false),
            })
        })
    },
};

/// `draw.fill` on an Image document: the editor's Bucket tool.
pub(crate) fn fill(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    edit_checked(ctx, args, |layer, _| {
        let tolerance = args.i64("tolerance").map_or(TOLERANCE, |t| t as u64);
        Ok(CommandSpec::FillBucket {
            layer,
            color: color_arg(args, "color")?,
            opacity: percent(args, "opacity"),
            tolerance: u8::try_from(tolerance)
                .map_err(|_| AgentError::tool("tolerance must be 0–255"))?,
            contiguous: args.bool("contiguous").unwrap_or(true),
            sample: str_or(args, "sample", "current_layer").to_owned(),
            x: num(args, "x")?,
            y: num(args, "y")?,
        })
    })
}

pub(crate) const GRADIENT: Tool = Tool {
    name: "draw.gradient",
    description: "Paint a gradient along the drag (x0, y0) → (x1, y1) from color `from` to color `to`, like the editor's Gradient tool; respects the selection. `layer` and `frame` (Pixel only) default to the paint target. Image documents: `kind` linear (default) / reflected / radial / conical / diamond; `channel` color (default) / transparency (fades existing alpha); `opacity` in % (default 100). Pixel documents (RGBA only): integer corners, `kind` linear (default) / radial, `dither` 0 (two hard bands), 2, 4 (default) or 8 (Bayer matrix size).",
    schema: || {
        edit_schema(
            json!({
                "x0": number("Start x"),
                "y0": number("Start y"),
                "x1": number("End x"),
                "y1": number("End y"),
                "from": shared_color("Start color"),
                "to": shared_color("End color"),
                "kind": choice(&["linear", "reflected", "radial", "conical", "diamond"], "Gradient shape; default linear (Pixel: linear / radial)"),
                "channel": choice(&["color", "transparency"], "Image: what the gradient paints; default color"),
                "opacity": percent_prop("Image: gradient opacity"),
                "dither": { "type": "integer", "description": "Pixel: Bayer matrix size 0, 2, 4 (default) or 8" },
                "frame": crate::pixel::frame_prop(),
            }),
            &["x0", "y0", "x1", "y1", "from", "to"],
        )
    },
    run: |ctx, args| {
        if !ctx.ws.is_image(args)? {
            return crate::pixel::paint::gradient(ctx, args);
        }
        refuse_pixel_args(ctx, args, &["dither"])?;
        edit_checked(ctx, args, |layer, _| {
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

/// The `pincel_effects` spec for `id` and the named `params`, checked
/// against the catalog ranges.
fn effect_spec(args: &Args) -> Result<EffectSpec, AgentError> {
    let id = args.req_str("id")?;
    let empty = Map::new();
    let params = args
        .get("params")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let wire = wire_params(id, params, true)?;
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
    let selection = document.selection.as_ref();
    let out = spec.run_in(&src, |x, y| selection.is_none_or(|m| m.coverage(x, y) > 0));
    let after = ImageBuffer::from_raw(w, h, out.into_raw()).map_err(core_err)?;
    let changed = match masked_region(before, &after, selection) {
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
    description: "Apply an adjustment from the editor's Adjust menu (brightness_contrast, hue_saturation, curves, levels, color_balance, invert, grayscale, posterize, threshold, auto_level, exposure, highlights_shadows, invert_alpha, sepia, temperature_tint) to the target layer, exactly as the editor does; respects the selection. `params` maps parameter names to values (see pincel://effects); omitted ones use their defaults. Effects (blur, sharpen, …) go through effect.apply.",
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
