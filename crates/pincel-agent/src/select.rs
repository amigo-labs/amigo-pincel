//! `select.*`: the selection tools of both editors, plus `select.get` and
//! `select.delete`. Pixel selections are transient editor state (no undo
//! step); Image selection changes are undo steps, as in each editor. See
//! docs/specs/mcp-parity-selection-transform.md.

use pincel_core::{
    ClearRegion, Rect as PxRect, SelectionMask as PxMask, SelectionMode as PxMode,
    composite_canvas, layer_canvas,
};
use pincel_image_core::command::spec::{CommandSpec, parse_sample};
use pincel_image_core::{
    Color, Point, Rect as ImRect, SelectionMask as ImMask, SelectionMode as ImMode, apply_mode,
    magic_wand,
};
use serde_json::{Value, json};

use crate::args::{Args, parse_region};
use crate::image::ImageDoc;
use crate::pixel::{READ_MAX, frame_prop, layer_prop, tool_schema};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// Every selection tool, in `tools/list` order.
pub(crate) static TOOLS: &[Tool] = &[
    RECT, ELLIPSE, POLYGON, WAND, ALL, CLEAR, INVERT, EXPAND, CONTRACT, FEATHER, GET, DELETE,
];

const MODES: [&str; 4] = ["replace", "add", "subtract", "intersect"];
/// The Image editor's Magic Wand tolerance default.
const IMAGE_TOLERANCE: u8 = 32;

fn mode_prop() -> Value {
    json!({ "type": "string", "enum": MODES, "description": "How the shape combines with the current selection; default replace" })
}

fn feather_prop() -> Value {
    json!({ "type": "integer", "minimum": 0, "maximum": 250, "description": "Image documents: soften the edge by this many px (0–250, default 0); Pixel documents have no feather" })
}

fn radius_schema(description: &str) -> Value {
    tool_schema(
        json!({ "radius": { "type": "integer", "minimum": 1, "maximum": 250, "description": description } }),
        &["radius"],
    )
}

fn box_schema() -> Value {
    tool_schema(
        json!({
            "x": { "type": "integer", "description": "Left edge in canvas px" },
            "y": { "type": "integer", "description": "Top edge in canvas px" },
            "width": { "type": "integer", "minimum": 1 },
            "height": { "type": "integer", "minimum": 1 },
            "mode": mode_prop(),
            "feather": feather_prop(),
        }),
        &["x", "y", "width", "height"],
    )
}

/// The Pixel selection's mask over the canvas, if any.
fn px_mask(doc: &Doc) -> Option<PxMask> {
    doc.sprite.selection_as_mask().filter(|m| !m.is_empty())
}

fn px_count(doc: &Doc) -> usize {
    px_mask(doc).map_or(0, |m| m.data().iter().filter(|&&v| v != 0).count())
}

fn im_count(doc: &ImageDoc) -> usize {
    doc.document()
        .selection
        .as_ref()
        .map_or(0, ImMask::selected_count)
}

/// The result of every selection tool (AC-11).
fn selection_out(ctx: &Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, info, count) = if ctx.ws.is_image(args)? {
        let (id, doc) = ctx.ws.image(args)?;
        (id.clone(), doc.info(&id), im_count(doc))
    } else {
        let (id, doc) = ctx.ws.doc(args)?;
        (id.clone(), doc.info(&id), px_count(doc))
    };
    Ok(ToolOutput::json(json!({
        "doc": id,
        "selection": info["selection"],
        "count": count,
    })))
}

fn mode_arg(args: &Args) -> Result<(PxMode, ImMode), AgentError> {
    match args.str("mode").unwrap_or("replace") {
        "replace" => Ok((PxMode::Replace, ImMode::Replace)),
        "add" => Ok((PxMode::Add, ImMode::Add)),
        "subtract" => Ok((PxMode::Subtract, ImMode::Subtract)),
        "intersect" => Ok((PxMode::Intersect, ImMode::Intersect)),
        other => Err(AgentError::tool(format!(
            "argument `mode`: unknown mode {other:?} (replace, add, subtract, intersect)"
        ))),
    }
}

/// Pixel documents have no feather (AC-13).
fn refuse_pixel_feather(id: &str, args: &Args) -> Result<(), AgentError> {
    if args.u32("feather")?.unwrap_or(0) > 0 {
        return Err(AgentError::tool(format!(
            "argument `feather`: document `{id}` is a Pixel document, which has no feather; `feather` works on Image documents only"
        )));
    }
    Ok(())
}

/// Run an Image selection command as one undo step when `predicted`
/// differs from the current selection; otherwise record nothing (AC-11).
fn run_image_selection(
    doc: &mut ImageDoc,
    predicted: Option<ImMask>,
    spec: CommandSpec,
) -> Result<(), AgentError> {
    if predicted == doc.document().selection {
        return Ok(());
    }
    doc.run(spec)?;
    Ok(())
}

fn label_mode(mode: ImMode) -> &'static str {
    match mode {
        ImMode::Replace => "replace",
        ImMode::Add => "add",
        ImMode::Subtract => "subtract",
        ImMode::Intersect => "intersect",
    }
}

/// `select.rect` / `select.ellipse` on either mode.
fn select_box(ctx: &mut Ctx<'_>, args: &Args, ellipse: bool) -> Result<ToolOutput, AgentError> {
    let (x, y) = (args.req_i32("x")?, args.req_i32("y")?);
    let (w, h) = (args.req_u32("width")?, args.req_u32("height")?);
    let (px_mode, im_mode) = mode_arg(args)?;
    if ctx.ws.is_image(args)? {
        let (_, doc) = ctx.ws.image_mut(args)?;
        let feather = args.u32("feather")?.unwrap_or(0);
        let (cw, ch) = (doc.width(), doc.height());
        let r = ImRect::new(x, y, w, h);
        let mut shape = if ellipse {
            ImMask::ellipse(cw, ch, r)
        } else {
            ImMask::rectangle(cw, ch, r)
        };
        shape.feather(feather);
        let predicted = apply_mode(doc.document().selection.as_ref(), shape, im_mode);
        let mode = label_mode(im_mode).to_owned();
        let spec = if ellipse {
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
        };
        run_image_selection(doc, Some(predicted), spec)?;
    } else {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        refuse_pixel_feather(&id, args)?;
        let (cw, ch) = (doc.sprite.width, doc.sprite.height);
        let r = PxRect::new(x, y, w, h);
        let shape = if ellipse {
            PxMask::ellipse(cw, ch, r)
        } else {
            PxMask::rect(cw, ch, r)
        };
        doc.sprite.combine_selection(shape, px_mode);
    }
    selection_out(ctx, args)
}

pub(crate) const RECT: Tool = Tool {
    name: "select.rect",
    description: "Select a rectangle like the editor's Rectangle Select; `mode` combines it with the current selection (replace, add, subtract, intersect). Image documents also take `feather`. While a selection exists, draw.fill, adjust.apply, effect.apply and select.delete act only on the selected pixels. Returns the selection's bounds {x, y, w, h} (or null) and the selected-pixel count. Image documents: one undo step; Pixel documents: no undo step.",
    schema: box_schema,
    run: |ctx, args| select_box(ctx, args, false),
};

pub(crate) const ELLIPSE: Tool = Tool {
    name: "select.ellipse",
    description: "Select the ellipse inscribed in the rectangle (x, y, width, height) like the editor's Ellipse Select; `mode` and (Image) `feather` as for select.rect.",
    schema: box_schema,
    run: |ctx, args| select_box(ctx, args, true),
};

pub(crate) const POLYGON: Tool = Tool {
    name: "select.polygon",
    description: "Select the polygon through `points` (at least three [x, y] pairs, closed automatically) like the editor's Lasso / Polygonal Lasso. Pixel documents take integer points; Image documents fractional ones and `feather`. `mode` as for select.rect.",
    schema: || {
        tool_schema(
            json!({
                "points": {
                    "type": "array",
                    "minItems": 3,
                    "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 },
                    "description": "Polygon corners as [x, y] pairs in canvas px",
                },
                "mode": mode_prop(),
                "feather": feather_prop(),
            }),
            &["points"],
        )
    },
    run: select_polygon,
};

/// The `points` argument as `(x, y)` pairs; `integer` requires whole
/// numbers (Pixel documents, AC-14).
fn points_arg(args: &Args, integer: bool) -> Result<Vec<(f64, f64)>, AgentError> {
    let points = args
        .get("points")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentError::tool("missing required argument `points`"))?;
    if points.len() < 3 {
        return Err(AgentError::tool(format!(
            "argument `points` needs at least 3 points (got {})",
            points.len()
        )));
    }
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let pair = p.as_array().filter(|a| a.len() == 2);
            let xy = pair.and_then(|a| Some((a[0].as_f64()?, a[1].as_f64()?)));
            let Some((x, y)) = xy else {
                return Err(AgentError::tool(format!(
                    "argument `points[{i}]` must be an [x, y] pair of numbers (got {p})"
                )));
            };
            if integer && (x.fract() != 0.0 || y.fract() != 0.0) {
                return Err(AgentError::tool(format!(
                    "argument `points[{i}]`: Pixel documents take integer coordinates (got {p})"
                )));
            }
            Ok((x, y))
        })
        .collect()
}

fn select_polygon(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (px_mode, im_mode) = mode_arg(args)?;
    if ctx.ws.is_image(args)? {
        let points = points_arg(args, false)?;
        let (_, doc) = ctx.ws.image_mut(args)?;
        let feather = args.u32("feather")?.unwrap_or(0);
        let pts: Vec<Point> = points
            .iter()
            .map(|&(x, y)| Point::new(x as f32, y as f32))
            .collect();
        let mut shape = ImMask::polygon(doc.width(), doc.height(), &pts);
        shape.feather(feather);
        let predicted = apply_mode(doc.document().selection.as_ref(), shape, im_mode);
        let spec = CommandSpec::SelectPolygon {
            points: pts.iter().map(|p| [p.x, p.y]).collect(),
            mode: label_mode(im_mode).to_owned(),
            feather,
        };
        run_image_selection(doc, Some(predicted), spec)?;
    } else {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        refuse_pixel_feather(&id, args)?;
        let points = points_arg(args, true)?;
        let pts: Vec<(i32, i32)> = points.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
        let shape = PxMask::polygon(doc.sprite.width, doc.sprite.height, &pts);
        doc.sprite.combine_selection(shape, px_mode);
    }
    selection_out(ctx, args)
}

pub(crate) const WAND: Tool = Tool {
    name: "select.wand",
    description: "Select the area around the seed (x, y) like the editor's Magic Wand: pixels within `tolerance` of the seed's color (default 0 in Pixel documents, 32 in Image documents), connected to it unless `contiguous: false`, sampled from the target layer (`sample: current_layer`, default) or the composed image (`all_layers`). On a uniform layer it selects the whole canvas. `mode` as for select.rect.",
    schema: || {
        tool_schema(
            json!({
                "x": { "type": "number", "description": "Seed x (an integer in Pixel documents)" },
                "y": { "type": "number", "description": "Seed y (an integer in Pixel documents)" },
                "tolerance": { "type": "integer", "minimum": 0, "maximum": 255, "description": "Color distance 0–255; default 0 (Pixel) / 32 (Image)" },
                "contiguous": { "type": "boolean", "description": "Only the area connected to the seed; default true" },
                "sample": { "type": "string", "enum": ["current_layer", "all_layers"], "description": "Sample the target layer or the composed image; default current_layer" },
                "mode": mode_prop(),
                "layer": layer_prop(),
                "frame": frame_prop(),
            }),
            &["x", "y"],
        )
    },
    run: select_wand,
};

/// The seed, refused outside the canvas (AC-15).
fn seed_arg(args: &Args, w: u32, h: u32) -> Result<(f64, f64), AgentError> {
    let num = |key: &str| {
        args.get(key)
            .and_then(Value::as_f64)
            .ok_or_else(|| AgentError::tool(format!("argument `{key}` must be a number")))
    };
    let (x, y) = (num("x")?, num("y")?);
    if x < 0.0 || y < 0.0 || x >= f64::from(w) || y >= f64::from(h) {
        return Err(AgentError::tool(format!(
            "the seed ({x}, {y}) lies outside the {w}×{h} canvas"
        )));
    }
    Ok((x, y))
}

fn select_wand(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (px_mode, im_mode) = mode_arg(args)?;
    let contiguous = args.bool("contiguous").unwrap_or(true);
    let sample = args.str("sample").unwrap_or("current_layer");
    let tolerance = args
        .u32("tolerance")?
        .map(|t| {
            u8::try_from(t).map_err(|_| AgentError::tool("argument `tolerance` must be 0–255"))
        })
        .transpose()?;
    if ctx.ws.is_image(args)? {
        let (_, doc) = ctx.ws.image_mut(args)?;
        let (x, y) = seed_arg(args, doc.width(), doc.height())?;
        let layer = match args.u32("layer")? {
            Some(id) => doc.layer_arg(id, "layer")?,
            None => doc.document().active_layer_index(),
        };
        let tolerance = tolerance.unwrap_or(IMAGE_TOLERANCE);
        let (x, y) = (x as f32, y as f32);
        let predicted = magic_wand(
            doc.document(),
            layer,
            Point::new(x, y),
            tolerance,
            contiguous,
            parse_sample(sample),
        )
        .map(|shape| apply_mode(doc.document().selection.as_ref(), shape, im_mode));
        let spec = CommandSpec::SelectWand {
            layer,
            x,
            y,
            tolerance,
            contiguous,
            sample: sample.to_owned(),
            mode: label_mode(im_mode).to_owned(),
        };
        let predicted = predicted.or_else(|| doc.document().selection.clone());
        run_image_selection(doc, predicted, spec)?;
    } else {
        let (_, doc) = ctx.ws.doc_mut(args)?;
        let (x, y) = seed_arg(args, doc.sprite.width, doc.sprite.height)?;
        for (key, v) in [("x", x), ("y", y)] {
            if v.fract() != 0.0 {
                return Err(AgentError::tool(format!(
                    "argument `{key}`: Pixel documents take an integer seed (got {v})"
                )));
            }
        }
        let frame = doc.frame_arg(args)?.unwrap_or(doc.active_frame);
        let buffer = if sample == "all_layers" {
            composite_canvas(&doc.sprite, &doc.cels, frame)
                .map_err(|e| AgentError::tool(format!("cannot compose the frame: {e}")))?
        } else {
            let layer = match args.u32("layer")? {
                Some(_) => crate::pixel::layer_id_arg(doc, args, "layer")?,
                None => doc
                    .sprite
                    .paint_target_layer(doc.active_layer)
                    .ok_or_else(|| AgentError::tool("document has no image layer to sample"))?,
            };
            layer_canvas(&doc.sprite, &doc.cels, layer, frame)
        };
        let shape = PxMask::wand(
            &buffer,
            (x as i32, y as i32),
            tolerance.unwrap_or(0),
            contiguous,
        );
        doc.sprite.combine_selection(shape, px_mode);
    }
    selection_out(ctx, args)
}

/// A selection command without arguments of its own.
fn simple(
    ctx: &mut Ctx<'_>,
    args: &Args,
    pixel: impl FnOnce(&mut Doc),
    image: impl FnOnce(&ImageDoc) -> (Option<ImMask>, CommandSpec),
) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        let (_, doc) = ctx.ws.image_mut(args)?;
        let (predicted, spec) = image(doc);
        run_image_selection(doc, predicted, spec)?;
    } else {
        let (_, doc) = ctx.ws.doc_mut(args)?;
        pixel(doc);
    }
    selection_out(ctx, args)
}

pub(crate) const ALL: Tool = Tool {
    name: "select.all",
    description: "Select the whole canvas (the editor's Select All).",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        simple(
            ctx,
            args,
            |d| {
                let (w, h) = (d.sprite.width, d.sprite.height);
                d.sprite.set_selection(PxRect::new(0, 0, w, h));
            },
            |d| {
                let full = ImMask::new_full(d.width(), d.height());
                (Some(full), CommandSpec::SelectAll)
            },
        )
    },
};

pub(crate) const CLEAR: Tool = Tool {
    name: "select.clear",
    description: "Drop the selection (the editor's Deselect); draw.fill, adjust.apply, effect.apply and select.delete act on the whole layer again.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        simple(
            ctx,
            args,
            |d| d.sprite.clear_selection(),
            |_| (None, CommandSpec::Deselect),
        )
    },
};

pub(crate) const INVERT: Tool = Tool {
    name: "select.invert",
    description: "Invert the selection (the editor's Invert Selection). Without a selection, Pixel documents select the whole canvas and Image documents select nothing, as each editor does.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        simple(
            ctx,
            args,
            |d| {
                let (w, h) = (d.sprite.width, d.sprite.height);
                let mut mask = d
                    .sprite
                    .selection_as_mask()
                    .unwrap_or_else(|| PxMask::empty(w, h));
                mask.invert();
                d.sprite.set_selection_mask(mask);
            },
            |d| {
                let mut mask = d
                    .document()
                    .selection
                    .clone()
                    .unwrap_or_else(|| ImMask::new_full(d.width(), d.height()));
                mask.invert();
                (Some(mask), CommandSpec::InvertSelection)
            },
        )
    },
};

/// `select.expand` / `select.contract`: a no-op without a selection.
fn morph(ctx: &mut Ctx<'_>, args: &Args, grow: bool) -> Result<ToolOutput, AgentError> {
    let radius = args.req_u32("radius")?;
    simple(
        ctx,
        args,
        |d| {
            if let Some(mask) = d.sprite.selection_as_mask() {
                let out = if grow {
                    mask.expand(radius)
                } else {
                    mask.contract(radius)
                };
                d.sprite.set_selection_mask(out);
            }
        },
        |d| {
            let predicted = d.document().selection.clone().map(|mut m| {
                if grow {
                    m.expand(radius);
                } else {
                    m.contract(radius);
                }
                m
            });
            let spec = if grow {
                CommandSpec::ExpandSelection { radius }
            } else {
                CommandSpec::ContractSelection { radius }
            };
            (predicted, spec)
        },
    )
}

pub(crate) const EXPAND: Tool = Tool {
    name: "select.expand",
    description: "Grow the selection by `radius` px (1–250), the editor's Expand Selection. Without a selection nothing changes.",
    schema: || radius_schema("Pixels to grow by (1–250)"),
    run: |ctx, args| morph(ctx, args, true),
};

pub(crate) const CONTRACT: Tool = Tool {
    name: "select.contract",
    description: "Shrink the selection by `radius` px (1–250), the editor's Contract Selection; it may leave nothing selected. Without a selection nothing changes.",
    schema: || radius_schema("Pixels to shrink by (1–250)"),
    run: |ctx, args| morph(ctx, args, false),
};

pub(crate) const FEATHER: Tool = Tool {
    name: "select.feather",
    description: "Image documents: soften the selection's edges by `radius` px (1–250), the editor's Feather Selection. Without a selection nothing changes.",
    schema: || radius_schema("Feather radius in px (1–250)"),
    run: |ctx, args| {
        let radius = args.req_u32("radius")?;
        if !ctx.ws.is_image(args)? {
            let (id, _) = ctx.ws.doc(args)?;
            return Err(AgentError::tool(format!(
                "select.feather: document `{id}` is a Pixel document, which has no `feather`; select.feather works on Image documents only"
            )));
        }
        let (_, doc) = ctx.ws.image_mut(args)?;
        let predicted = doc.document().selection.clone().map(|mut m| {
            m.feather(radius);
            m
        });
        run_image_selection(doc, predicted, CommandSpec::FeatherSelection { radius })?;
        selection_out(ctx, args)
    },
};

pub(crate) const GET: Tool = Tool {
    name: "select.get",
    description: "Read the selection: its bounds {x, y, w, h} (or null), the selected-pixel count over the whole canvas, and `rows` of two-digit hex coverage per pixel (00 unselected, ff selected; Image documents also every value between) for `region` (at most 64×64 inside the canvas; default: the selection's bounds). Changes nothing.",
    schema: || {
        tool_schema(
            json!({ "region": crate::args::region_schema("Canvas rectangle to read (at most 64×64); default: the selection's bounds") }),
            &[],
        )
    },
    run: select_get,
};

/// Selection coverage (0–255) of a canvas pixel.
type Coverage<'a> = Box<dyn Fn(u32, u32) -> u8 + 'a>;

fn select_get(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let out = selection_out(ctx, args)?.json;
    let image = ctx.ws.is_image(args)?;
    let (cw, ch, coverage): (u32, u32, Coverage<'_>) = if image {
        let (_, doc) = ctx.ws.image(args)?;
        let sel = doc.document().selection.as_ref();
        (
            doc.width(),
            doc.height(),
            Box::new(move |x, y| sel.map_or(0, |m| m.coverage(x, y))),
        )
    } else {
        let (_, doc) = ctx.ws.doc(args)?;
        let sprite = &doc.sprite;
        (
            sprite.width,
            sprite.height,
            Box::new(move |x, y| {
                if sprite.selection_contains(x as i32, y as i32) {
                    255
                } else {
                    0
                }
            }),
        )
    };
    let region = match args.get("region") {
        Some(r) => Some(parse_region(r)?),
        None => out["selection"].as_object().map(|b| {
            let n = |k: &str| b.get(k).and_then(Value::as_i64).unwrap_or(0);
            PxRect::new(n("x") as i32, n("y") as i32, n("w") as u32, n("h") as u32)
        }),
    };
    let Some(r) = region else {
        return Ok(ToolOutput::json(json!({
            "doc": out["doc"],
            "selection": Value::Null,
            "count": 0,
            "region": Value::Null,
            "rows": [],
        })));
    };
    if r.width > READ_MAX || r.height > READ_MAX {
        return Err(AgentError::tool(format!(
            "region {}×{} is larger than the {READ_MAX}×{READ_MAX} limit of select.get; read it in parts with `region`",
            r.width, r.height
        )));
    }
    if !crate::args::inside_canvas(r, cw, ch) {
        return Err(AgentError::tool(format!(
            "region ({}, {}, {}×{}) is not inside the {cw}×{ch} canvas",
            r.x, r.y, r.width, r.height
        )));
    }
    let rows: Vec<String> = (0..r.height)
        .map(|dy| {
            (0..r.width)
                .map(|dx| format!("{:02x}", coverage(r.x as u32 + dx, r.y as u32 + dy)))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    Ok(ToolOutput::json(json!({
        "doc": out["doc"],
        "selection": out["selection"],
        "count": out["count"],
        "region": { "x": r.x, "y": r.y, "width": r.width, "height": r.height },
        "rows": rows,
    })))
}

pub(crate) const DELETE: Tool = Tool {
    name: "select.delete",
    description: "The editor's Delete: make the selected pixels of the target layer transparent (Image documents scale a feathered pixel's alpha by its coverage); the selection stays. Without a selection, Image documents clear the whole layer and Pixel documents change nothing. Returns {doc, layer, changed}; one undo step when a pixel changed.",
    schema: || tool_schema(json!({ "layer": layer_prop(), "frame": frame_prop() }), &[]),
    run: select_delete,
};

/// Whether the Image Delete of the layer at `index` would change a pixel
/// (the same arithmetic as `pincel_image_core::delete_selection`).
fn image_delete_changes(doc: &ImageDoc, index: usize) -> bool {
    let d = doc.document();
    let src = &d.layers()[index].pixels;
    let (w, h) = (src.width(), src.height());
    let pixel = |x: u32, y: u32| src.get_pixel(x, y).unwrap_or(Color::TRANSPARENT);
    match d.selection.as_ref() {
        None => (0..h).any(|y| (0..w).any(|x| pixel(x, y) != Color::TRANSPARENT)),
        Some(sel) => (0..h).any(|y| {
            (0..w).any(|x| {
                let cov = sel.coverage(x, y);
                let c = pixel(x, y);
                if cov == 0 || c.a == 0 {
                    return false;
                }
                let keep = 1.0 - f32::from(cov) / 255.0;
                (f32::from(c.a) * keep).round() as u8 != c.a
            })
        }),
    }
}

fn select_delete(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        let (id, doc) = ctx.ws.image_mut(args)?;
        let index = doc.target(args)?;
        let changed = image_delete_changes(doc, index)
            && doc.run(CommandSpec::DeleteSelection { layer: index })?;
        return Ok(ToolOutput::json(json!({
            "doc": id,
            "layer": doc.layer_id(index),
            "changed": changed,
        })));
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let (layer, frame) = doc.paint_target(args)?;
    let changed = match doc.sprite.selection.filter(|r| !r.is_empty()) {
        None => false,
        Some(sel) => {
            doc.commit_if_changed(|| vec![ClearRegion::new(layer, frame, sel).masked().into()])?
        }
    };
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": layer.0,
        "changed": changed,
    })))
}
