//! `canvas.resize` and `image.*`: whole-document transforms through the
//! editor's Image menu commands, each one undo step.

use pincel_image_core::command::spec::CommandSpec;
use serde_json::{Value, json};

use super::check_size;
use crate::args::Args;
use crate::pixel::tool_schema;
use crate::{AgentError, Ctx, Tool, ToolOutput};

const ANCHORS: [&str; 9] = [
    "top_left",
    "top_center",
    "top_right",
    "center_left",
    "center",
    "center_right",
    "bottom_left",
    "bottom_center",
    "bottom_right",
];

/// Run a document-wide command and report the new size.
fn transform(
    ctx: &mut Ctx<'_>,
    args: &Args,
    build: impl FnOnce() -> Result<CommandSpec, AgentError>,
) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let spec = build()?;
    let changed = doc.run(spec)?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "width": doc.width(),
        "height": doc.height(),
        "changed": changed,
    })))
}

fn size_props(extra: Value) -> Value {
    let mut props = json!({
        "width": { "type": "integer", "minimum": 1, "description": "New width in px (at most 16384; area at most 64 Mpx)" },
        "height": { "type": "integer", "minimum": 1, "description": "New height in px (at most 16384; area at most 64 Mpx)" },
    });
    if let (Some(p), Value::Object(e)) = (props.as_object_mut(), extra) {
        p.extend(e);
    }
    tool_schema(props, &["width", "height"])
}

/// The requested size, refused above the editor's limits (AC-21).
fn new_size(args: &Args) -> Result<(u32, u32), AgentError> {
    let (w, h) = (args.req_u32("width")?, args.req_u32("height")?);
    check_size(w, h)?;
    Ok((w, h))
}

pub(crate) const RESIZE: Tool = Tool {
    name: "canvas.resize",
    description: "Image documents: change the canvas size without scaling the content, like the editor's Canvas Size; `anchor` places the old content (default top_left). New area is transparent. One undo step.",
    schema: || {
        size_props(json!({
            "anchor": { "type": "string", "enum": ANCHORS, "description": "Where the old content stays; default top_left" },
        }))
    },
    run: |ctx, args| {
        let (width, height) = new_size(args)?;
        transform(ctx, args, || {
            Ok(CommandSpec::ResizeCanvas {
                width,
                height,
                anchor: args.str("anchor").unwrap_or("top_left").to_owned(),
            })
        })
    },
};

pub(crate) const SCALE: Tool = Tool {
    name: "image.scale",
    description: "Image documents: resample every layer to width × height like the editor's Image Size (interpolation nearest / bilinear (default) / bicubic). One undo step.",
    schema: || {
        size_props(json!({
            "interpolation": { "type": "string", "enum": ["nearest", "bilinear", "bicubic"], "description": "Resampling; default bilinear" },
        }))
    },
    run: |ctx, args| {
        let (width, height) = new_size(args)?;
        transform(ctx, args, || {
            Ok(CommandSpec::ScaleImage {
                width,
                height,
                interpolation: args.str("interpolation").unwrap_or("bilinear").to_owned(),
            })
        })
    },
};

pub(crate) const CROP: Tool = Tool {
    name: "image.crop_to_selection",
    description: "Image documents: crop the canvas to the selection's bounding box, like the editor's Crop to Selection. Needs a selection (select.rect, select.ellipse or select.wand first). One undo step.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.image(args)?;
        let has_selection = doc
            .document()
            .selection
            .as_ref()
            .is_some_and(|s| s.bounding_box().is_some());
        if !has_selection {
            return Err(AgentError::tool(format!(
                "image.crop_to_selection needs a selection, and document `{id}` has none; select an area with select.rect, select.ellipse or select.wand first"
            )));
        }
        transform(ctx, args, || Ok(CommandSpec::CropToSelection))
    },
};

pub(crate) const FLIP: Tool = Tool {
    name: "image.flip",
    description: "Image documents: mirror the whole image (all layers) horizontally or vertically, like the editor's Flip Canvas. One undo step.",
    schema: || {
        tool_schema(
            json!({ "direction": { "type": "string", "enum": ["horizontal", "vertical"], "description": "Mirror axis" } }),
            &["direction"],
        )
    },
    run: |ctx, args| {
        let horizontal = args.req_str("direction")? == "horizontal";
        transform(ctx, args, || Ok(CommandSpec::FlipCanvas { horizontal }))
    },
};

pub(crate) const ROTATE: Tool = Tool {
    name: "image.rotate",
    description: "Image documents: rotate the whole image (all layers) clockwise by 90, 180 or 270 degrees, like the editor's Rotate Canvas; 90 and 270 swap width and height. One undo step.",
    schema: || {
        tool_schema(
            json!({ "angle": { "type": "integer", "enum": [90, 180, 270], "description": "Clockwise rotation in degrees" } }),
            &["angle"],
        )
    },
    run: |ctx, args| {
        let rotation = match args.i64("angle") {
            Some(90) => "cw90",
            Some(180) => "rotate_180",
            Some(270) => "ccw90",
            _ => return Err(AgentError::tool("angle must be one of: 90, 180, 270")),
        };
        transform(ctx, args, || {
            Ok(CommandSpec::RotateCanvas {
                rotation: rotation.to_owned(),
            })
        })
    },
};
