//! `canvas.resize`, `image.*` and `layer.flip` / `layer.rotate`: the
//! canvas and layer transforms of both editors, each one undo step when it
//! changes the document. See docs/specs/mcp-headless-image.md AC-13 and
//! docs/specs/mcp-parity-selection-transform.md AC-24 … AC-33.

use pincel_core::{
    Anchor, AnyCommand, HistoryKind, Interpolation, LayerKind, Orientation, Rect, ReframeCanvas,
    ScaleImage, TransformCanvas, TransformCel,
};
use pincel_image_core::auto_crop_rect;
use pincel_image_core::command::spec::CommandSpec;
use serde_json::{Value, json};

use crate::args::Args;
use crate::image::check_size;
use crate::pixel::{frame_prop, layer_prop, tool_schema};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// Every transform tool, in `tools/list` order.
pub(crate) static TOOLS: &[Tool] = &[
    RESIZE,
    SCALE,
    CROP,
    AUTO_CROP,
    FLIP,
    ROTATE,
    LAYER_FLIP,
    LAYER_ROTATE,
];

/// The anchors `canvas.resize` takes, with the Pixel core's name for each.
const ANCHORS: [(&str, &str); 9] = [
    ("top_left", "top_left"),
    ("top_center", "top"),
    ("top_right", "top_right"),
    ("center_left", "left"),
    ("center", "center"),
    ("center_right", "right"),
    ("bottom_left", "bottom_left"),
    ("bottom_center", "bottom"),
    ("bottom_right", "bottom_right"),
];
/// Largest Pixel document side, as in the editor (`NEW_DOC_LIMITS`).
const PIXEL_MAX: i64 = 4096;
/// Largest Image document side, as in the editor (`MAX_CANVAS_DIM`).
const IMAGE_MAX: i64 = 16_384;

fn size_out(id: &str, width: u32, height: u32, changed: bool) -> ToolOutput {
    ToolOutput::json(json!({ "doc": id, "width": width, "height": height, "changed": changed }))
}

fn size_schema(extra: Value) -> Value {
    let mut props = json!({
        "width": { "type": "integer", "description": "New width in px: 1–4096 in Pixel documents, 1–16384 in Image documents (area at most 64 Mpx)" },
        "height": { "type": "integer", "description": "New height in px: 1–4096 in Pixel documents, 1–16384 in Image documents (area at most 64 Mpx)" },
    });
    if let (Some(p), Value::Object(e)) = (props.as_object_mut(), extra) {
        p.extend(e);
    }
    tool_schema(props, &["width", "height"])
}

/// The requested size within the mode's limits (AC-29; Image:
/// mcp-headless-image AC-21).
fn new_size(args: &Args, image: bool) -> Result<(u32, u32), AgentError> {
    let mut size = [0u32; 2];
    for (slot, key) in size.iter_mut().zip(["width", "height"]) {
        let v = args
            .i64(key)
            .ok_or_else(|| AgentError::tool(format!("missing required argument `{key}`")))?;
        // Image sizes above the limit get check_size's message, which
        // names the size and the area limit.
        let max = if image {
            i64::from(u32::MAX)
        } else {
            PIXEL_MAX
        };
        if !(1..=max).contains(&v) {
            let max = if image { IMAGE_MAX } else { PIXEL_MAX };
            return Err(AgentError::tool(format!("{key} must be 1–{max}")));
        }
        *slot = v as u32;
    }
    if image {
        check_size(size[0], size[1])?;
    }
    Ok((size[0], size[1]))
}

/// Refuse a Pixel transform that the core refuses for tilemap layers, with
/// a reason an agent can act on (AC-30).
fn refuse_tilemaps(doc: &Doc) -> Result<(), AgentError> {
    if let Some(l) = doc
        .sprite
        .layers
        .iter()
        .find(|l| matches!(l.kind, LayerKind::Tilemap { .. }))
    {
        return Err(AgentError::tool(format!(
            "the document has a tilemap layer ({} {:?}); tilemap layers cannot be transformed",
            l.id.0, l.name
        )));
    }
    Ok(())
}

/// Run the Pixel command `make` builds when it changes the document and
/// report the new size.
fn pixel_canvas(
    id: &str,
    doc: &mut Doc,
    make: impl Fn() -> AnyCommand,
) -> Result<ToolOutput, AgentError> {
    let changed = doc.commit_if_changed(|| vec![make()])?;
    Ok(size_out(id, doc.sprite.width, doc.sprite.height, changed))
}

/// Run the Image command `build` makes when it changes the document and
/// report the new size.
fn image_canvas(
    ctx: &mut Ctx<'_>,
    args: &Args,
    build: impl Fn() -> CommandSpec,
) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let changed = doc.run_if_changed(build)?;
    Ok(size_out(&id, doc.width(), doc.height(), changed))
}

pub(crate) const RESIZE: Tool = Tool {
    name: "canvas.resize",
    description: "Change the canvas size without scaling the content, like the editor's Canvas Size; `anchor` places the old content (default: center in Pixel documents, top_left in Image documents). New area is transparent. Pixel documents lose the selection (undo restores it).",
    schema: || {
        let names: Vec<&str> = ANCHORS.iter().map(|(n, _)| *n).collect();
        size_schema(json!({
            "anchor": { "type": "string", "enum": names, "description": "Where the old content stays; default center (Pixel) / top_left (Image)" },
        }))
    },
    run: |ctx, args| {
        let image = ctx.ws.is_image(args)?;
        let (width, height) = new_size(args, image)?;
        if image {
            let anchor = args.str("anchor").unwrap_or("top_left").to_owned();
            return image_canvas(ctx, args, || CommandSpec::ResizeCanvas {
                width,
                height,
                anchor: anchor.clone(),
            });
        }
        let name = args.str("anchor").unwrap_or("center");
        let anchor = ANCHORS
            .iter()
            .find(|(n, _)| *n == name)
            .and_then(|(_, core)| Anchor::from_name(core))
            .ok_or_else(|| {
                AgentError::tool(format!("argument `anchor`: unknown anchor {name:?}"))
            })?;
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let (ow, oh) = (doc.sprite.width, doc.sprite.height);
        pixel_canvas(&id, doc, || {
            ReframeCanvas::resize(ow, oh, width, height, anchor).into()
        })
    },
};

pub(crate) const SCALE: Tool = Tool {
    name: "image.scale",
    description: "Resample the whole image to width × height like the editor's Scale Image / Image Size (`interpolation` nearest, bilinear or bicubic, and in Image documents supersampling — an area average when shrinking, bicubic when an edge grows; default nearest in Pixel documents, bilinear in Image documents).",
    schema: || {
        size_schema(json!({
            "interpolation": { "type": "string", "enum": ["nearest", "bilinear", "bicubic", "supersampling"], "description": "Resampling; default nearest (Pixel) / bilinear (Image); supersampling in Image documents only" },
        }))
    },
    run: |ctx, args| {
        let image = ctx.ws.is_image(args)?;
        let (width, height) = new_size(args, image)?;
        if image {
            let interpolation = args.str("interpolation").unwrap_or("bilinear").to_owned();
            return image_canvas(ctx, args, || CommandSpec::ScaleImage {
                width,
                height,
                interpolation: interpolation.clone(),
            });
        }
        let name = args.str("interpolation").unwrap_or("nearest");
        let filter = Interpolation::from_name(name).ok_or_else(|| {
            AgentError::tool(format!(
                "argument `interpolation`: unknown interpolation {name:?}"
            ))
        })?;
        let (id, doc) = ctx.ws.doc_mut(args)?;
        refuse_tilemaps(doc)?;
        pixel_canvas(&id, doc, || ScaleImage::new(width, height, filter).into())
    },
};

fn no_selection(id: &str) -> AgentError {
    AgentError::tool(format!(
        "image.crop_to_selection needs a selection, and document `{id}` has none; select an area with select.rect, select.ellipse or select.wand first"
    ))
}

pub(crate) const CROP: Tool = Tool {
    name: "image.crop_to_selection",
    description: "Crop the canvas to the selection's bounding box (clipped to the canvas), like the editor's Crop to Selection. Needs a selection.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        if ctx.ws.is_image(args)? {
            let (id, doc) = ctx.ws.image(args)?;
            let has_selection = doc
                .document()
                .selection
                .as_ref()
                .is_some_and(|s| s.bounding_box().is_some());
            if !has_selection {
                return Err(no_selection(&id));
            }
            return image_canvas(ctx, args, || CommandSpec::CropToSelection);
        }
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let canvas = Rect::new(0, 0, doc.sprite.width, doc.sprite.height);
        let rect = doc
            .sprite
            .selection
            .filter(|r| !r.is_empty())
            .map(|r| r.intersect(canvas))
            .filter(|r| !r.is_empty())
            .ok_or_else(|| no_selection(&id))?;
        let changed = doc.commit_if_changed_as(HistoryKind::CropToSelection, || {
            vec![ReframeCanvas::new(rect).into()]
        })?;
        Ok(size_out(&id, doc.sprite.width, doc.sprite.height, changed))
    },
};

pub(crate) const AUTO_CROP: Tool = Tool {
    name: "image.auto_crop",
    description: "Image documents only: crop the canvas to the content inside a uniform border, like the editor's Image ▸ Auto Crop. The border color is the top-left pixel of the visible composite; pixels match when all channels are equal or both are fully transparent. Returns `cropped` and, when cropped, the new size and the kept `rect` in the old canvas's pixels. Nothing to crop is not an error.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (_, doc) = ctx.ws.image_mut(args)?;
        let Some(rect) = auto_crop_rect(doc.document()) else {
            return Ok(ToolOutput::json(json!({ "cropped": false })));
        };
        doc.run_if_changed(|| CommandSpec::AutoCrop)?;
        Ok(ToolOutput::json(json!({
            "cropped": true,
            "width": doc.width(),
            "height": doc.height(),
            "rect": { "x": rect.x, "y": rect.y, "w": rect.w, "h": rect.h },
        })))
    },
};

fn direction_horizontal(args: &Args) -> Result<bool, AgentError> {
    match args.req_str("direction")? {
        "horizontal" => Ok(true),
        "vertical" => Ok(false),
        other => Err(AgentError::tool(format!(
            "argument `direction`: unknown direction {other:?} (horizontal, vertical)"
        ))),
    }
}

fn direction_schema(extra: Value) -> Value {
    let mut props = json!({ "direction": { "type": "string", "enum": ["horizontal", "vertical"], "description": "Mirror axis" } });
    if let (Some(p), Value::Object(e)) = (props.as_object_mut(), extra) {
        p.extend(e);
    }
    tool_schema(props, &["direction"])
}

/// A clockwise angle of 90, 180 or 270 degrees.
fn angle_arg(args: &Args) -> Result<u32, AgentError> {
    match args.i64("angle") {
        Some(a @ (90 | 180 | 270)) => Ok(a as u32),
        _ => Err(AgentError::tool(
            "argument `angle` must be one of: 90, 180, 270",
        )),
    }
}

fn angle_schema(extra: Value) -> Value {
    let mut props = json!({ "angle": { "type": "integer", "enum": [90, 180, 270], "description": "Clockwise rotation in degrees" } });
    if let (Some(p), Value::Object(e)) = (props.as_object_mut(), extra) {
        p.extend(e);
    }
    tool_schema(props, &["angle"])
}

fn rotation(angle: u32) -> Orientation {
    match angle {
        90 => Orientation::Rotate90Cw,
        180 => Orientation::Rotate180,
        _ => Orientation::Rotate90Ccw,
    }
}

pub(crate) const FLIP: Tool = Tool {
    name: "image.flip",
    description: "Mirror the whole image (every layer and frame) horizontally or vertically, like the editor's Flip Canvas. Pixel documents lose the selection (undo restores it).",
    schema: || direction_schema(json!({})),
    run: |ctx, args| {
        let horizontal = direction_horizontal(args)?;
        if ctx.ws.is_image(args)? {
            return image_canvas(ctx, args, || CommandSpec::FlipCanvas { horizontal });
        }
        let (id, doc) = ctx.ws.doc_mut(args)?;
        refuse_tilemaps(doc)?;
        let op = if horizontal {
            Orientation::FlipHorizontal
        } else {
            Orientation::FlipVertical
        };
        pixel_canvas(&id, doc, || TransformCanvas::new(op).into())
    },
};

pub(crate) const ROTATE: Tool = Tool {
    name: "image.rotate",
    description: "Rotate the whole image (every layer and frame) clockwise by 90, 180 or 270 degrees, like the editor's Rotate Canvas; 90 and 270 swap width and height. Pixel documents lose the selection (undo restores it).",
    schema: || angle_schema(json!({})),
    run: |ctx, args| {
        let angle = angle_arg(args)?;
        if ctx.ws.is_image(args)? {
            let rotation = match angle {
                90 => "cw90",
                180 => "rotate_180",
                _ => "ccw90",
            };
            return image_canvas(ctx, args, || CommandSpec::RotateCanvas {
                rotation: rotation.to_owned(),
            });
        }
        let (id, doc) = ctx.ws.doc_mut(args)?;
        refuse_tilemaps(doc)?;
        let op = rotation(angle);
        pixel_canvas(&id, doc, || TransformCanvas::new(op).into())
    },
};

fn layer_out(id: &str, layer: u32, changed: bool) -> ToolOutput {
    ToolOutput::json(json!({ "doc": id, "layer": layer, "changed": changed }))
}

/// `layer.flip` / `layer.rotate` on either mode: `pixel` is the Pixel
/// orientation, `image` the Image editor command for a layer index.
fn layer_transform(
    ctx: &mut Ctx<'_>,
    args: &Args,
    pixel: Orientation,
    image: impl Fn(usize) -> CommandSpec,
) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        let (id, doc) = ctx.ws.image_mut(args)?;
        let index = match args.u32("layer")? {
            Some(l) => doc.layer_arg(l, "layer")?,
            None => doc.document().active_layer_index(),
        };
        let changed = doc.run_if_changed(|| image(index))?;
        return Ok(layer_out(&id, doc.layer_id(index), changed));
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let (layer, frame) = doc.paint_target(args)?;
    if doc.cels.get(layer, frame).is_none() {
        // An empty cel turns into itself.
        return Ok(layer_out(&id, layer.0, false));
    }
    let within = doc.sprite.selection.filter(|r| !r.is_empty());
    let changed = doc.commit_if_changed(|| {
        let mut cmd = TransformCel::new(layer, frame, pixel);
        if let Some(sel) = within {
            cmd = cmd.within(sel);
        }
        vec![cmd.into()]
    })?;
    Ok(layer_out(&id, layer.0, changed))
}

fn target_props() -> Value {
    json!({ "layer": layer_prop(), "frame": frame_prop() })
}

pub(crate) const LAYER_FLIP: Tool = Tool {
    name: "layer.flip",
    description: "Mirror one layer horizontally or vertically, like the editor's Flip Layer; the canvas size stays. Pixel documents flip the target frame's cel, only the selection's bounds when there is a selection; Image documents flip the whole layer (a group with its subtree) whatever is selected.",
    schema: || direction_schema(target_props()),
    run: |ctx, args| {
        let horizontal = direction_horizontal(args)?;
        let pixel = if horizontal {
            Orientation::FlipHorizontal
        } else {
            Orientation::FlipVertical
        };
        let op = if horizontal { "flip_h" } else { "flip_v" };
        layer_transform(ctx, args, pixel, |layer| CommandSpec::TransformLayer {
            layer,
            op: op.to_owned(),
        })
    },
};

pub(crate) const LAYER_ROTATE: Tool = Tool {
    name: "layer.rotate",
    description: "Rotate one layer clockwise by 90, 180 or 270 degrees, like the editor's Rotate Layer; the canvas size stays and content that no longer fits is cut off. Pixel documents turn the target frame's cel, only the selection's bounds (about their centre) when there is a selection; Image documents turn the whole layer (a group with its subtree).",
    schema: || angle_schema(target_props()),
    run: |ctx, args| {
        let angle = angle_arg(args)?;
        layer_transform(ctx, args, rotation(angle), |layer| match angle {
            180 => CommandSpec::TransformLayer {
                layer,
                op: "rotate_180".to_owned(),
            },
            a => CommandSpec::RotateLayer90 {
                layer,
                ccw: a == 270,
            },
        })
    },
};
