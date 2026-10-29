//! `doc.*` and `export.*` on Image documents: create, open, preview,
//! pixel readback, undo / redo, `.ora` save and flat exports.

use pincel_image_core::codec::{
    decode, from_ora_bytes, to_jpeg_bytes, to_ora_bytes, to_png_bytes, to_webp_bytes,
};
use pincel_image_core::{Document, ImageBuffer, Rect, compose, compose_region};
use serde_json::{Value, json};

use super::{ImageDoc, MAX_SIDE, check_size, core_err};
use crate::args::{Args, hex, inside_canvas, parse_region};
use crate::pixel::{
    EXPORT_MAX_EDGE, READ_MAX, path_prop, rect_json, region_arg, tool_schema, with_id,
};
use crate::preview::{GRID_MIN_SCALE, PREVIEW_MAX_EDGE, auto_scale, check_edges, scaled_png};
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// PNG compression level of the editor's PNG export and project save.
const PNG_LEVEL: u8 = 6;
/// The editor's default JPEG quality.
const JPEG_QUALITY: u64 = 90;

/// `true` for a ZIP container (OpenRaster), as the editor sniffs it.
fn is_ora(path: &str, bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04") || path.to_ascii_lowercase().ends_with(".ora")
}

/// The format of a flat image file, from its signature.
fn flat_format(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "PNG"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "JPEG"
    } else if bytes.starts_with(b"GIF8") {
        "GIF"
    } else if bytes.starts_with(b"BM") {
        "BMP"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "WebP"
    } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        "TIFF"
    } else {
        "image"
    }
}

/// `doc.new` with `mode: "image"`: one transparent layer, as the editor's
/// New Image document.
pub(crate) fn new_doc(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    if args.get("palette").is_some() {
        return Err(AgentError::tool(
            "`palette` applies to Pixel documents; Image documents have no palette",
        ));
    }
    let width = args.req_u32("width")?;
    let height = args.req_u32("height")?;
    for (key, v) in [("width", width), ("height", height)] {
        if v == 0 {
            return Err(AgentError::tool(format!("{key} must be 1–{MAX_SIDE}")));
        }
    }
    check_size(width, height).map_err(|e| AgentError::tool(format!("cannot create: {e}")))?;
    let document = Document::new(width, height).map_err(core_err)?;
    let doc = ImageDoc::new(document, None, None);
    let info = doc.info("");
    let id = ctx.ws.insert_image(doc);
    Ok(ToolOutput::json(with_id(info, &id)))
}

/// `doc.open` of an `.ora` project or a flat image (PNG, JPEG, WebP, BMP,
/// GIF, TIFF), decoded as the editor's Open does.
pub(crate) fn open(ctx: &mut Ctx<'_>, path: &str) -> Result<ToolOutput, AgentError> {
    let fail = |e: String| AgentError::tool(format!("cannot open `{path}`: {e}"));
    let bytes = ctx.host.read(path).map_err(fail)?;
    let doc = if is_ora(path, &bytes) {
        let document = from_ora_bytes(&bytes).map_err(|e| fail(e.to_string()))?;
        check_size(document.canvas.width(), document.canvas.height())
            .map_err(|e| fail(e.to_string()))?;
        ImageDoc::new(document, Some(path.to_owned()), None)
    } else {
        let pixels = decode(&bytes).map_err(|e| fail(e.to_string()))?;
        check_size(pixels.width(), pixels.height()).map_err(|e| fail(e.to_string()))?;
        let document = Document::from_pixels(pixels).map_err(|e| fail(e.to_string()))?;
        ImageDoc::new(document, None, Some((path.to_owned(), flat_format(&bytes))))
    };
    let info = doc.info("");
    let id = ctx.ws.insert_image(doc);
    Ok(ToolOutput::json(with_id(info, &id)))
}

/// Image documents have one frame; `frame` may only name it.
fn check_frame(args: &Args) -> Result<(), AgentError> {
    match args.u32("frame")? {
        None | Some(0) => Ok(()),
        Some(f) => Err(AgentError::tool(format!(
            "frame {f} does not exist; Image documents have a single frame (0)"
        ))),
    }
}

/// Composed RGBA of `r` (inside the canvas).
fn composed(doc: &ImageDoc, r: Rect) -> Vec<u8> {
    compose_region(doc.document().layers(), r).into_raw()
}

pub(crate) fn preview(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image(args)?;
    check_frame(args)?;
    let region = region_arg(args, doc.width(), doc.height())?;
    let scale = args
        .u32("scale")?
        .unwrap_or_else(|| auto_scale(region.width, region.height));
    let want_grid = args.bool("grid").unwrap_or(false);
    let grid = want_grid && scale >= GRID_MIN_SCALE;
    check_edges(region.width, region.height, scale, PREVIEW_MAX_EDGE)?;
    let r = Rect::new(region.x, region.y, region.width, region.height);
    let (png, width, height) = scaled_png(&composed(doc, r), r.w, r.h, scale, grid)?;
    let mut json = json!({
        "doc": id,
        "frame": 0,
        "scale": scale,
        "width": width,
        "height": height,
        "region": rect_json(region),
        "grid": grid,
    });
    if want_grid && !grid {
        json["note"] = json!("grid omitted below scale 4");
    }
    Ok(ToolOutput {
        json,
        image_png: Some(png),
    })
}

pub(crate) fn read_pixels(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image(args)?;
    if args.str("format") == Some("index") {
        return Err(AgentError::tool(format!(
            "format `index` reads palette indices, which only Pixel documents have; document `{id}` is an Image document (use format `hex`)"
        )));
    }
    check_frame(args)?;
    let v = args
        .get("region")
        .ok_or_else(|| AgentError::tool("missing required argument `region`"))?;
    let region = parse_region(v)?;
    if region.width > READ_MAX || region.height > READ_MAX {
        return Err(AgentError::tool(format!(
            "region {}×{} is larger than the {READ_MAX}×{READ_MAX} limit of doc.read_pixels; read it in parts",
            region.width, region.height
        )));
    }
    let (w, h) = (doc.width(), doc.height());
    if !inside_canvas(region, w, h) {
        return Err(AgentError::tool(format!(
            "region ({}, {}, {}×{}) lies outside the {w}×{h} canvas",
            region.x, region.y, region.width, region.height
        )));
    }
    let r = Rect::new(region.x, region.y, region.width, region.height);
    let pixels = match args.u32("layer")? {
        None => composed(doc, r),
        Some(layer) => {
            let index = doc.layer_index(layer)?;
            doc.document().layers()[index]
                .pixels
                .copy_region(r)
                .map_err(core_err)?
                .into_raw()
        }
    };
    let rows: Vec<String> = pixels
        .chunks_exact(r.w as usize * 4)
        .map(|row| {
            row.as_chunks::<4>()
                .0
                .iter()
                .map(|p| hex(pincel_core::Rgba::new(p[0], p[1], p[2], p[3])))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    Ok(ToolOutput::json(json!({
        "doc": id,
        "frame": 0,
        "region": rect_json(region),
        "format": "hex",
        "rows": rows,
    })))
}

pub(crate) fn undo(ctx: &mut Ctx<'_>, args: &Args, redo: bool) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let done = if redo { doc.bus.redo() } else { doc.bus.undo() }.map_err(core_err)?;
    if done {
        doc.dirty = true;
        doc.sync_ids();
    }
    let mut out = json!({
        "doc": id,
        "undo_depth": doc.bus.history.undo_depth(),
        "redo_depth": doc.bus.history.redo_depth(),
    });
    out[if redo { "redone" } else { "undone" }] = json!(done);
    Ok(ToolOutput::json(out))
}

/// `doc.save`: an OpenRaster project with every layer (AC-16, AC-20).
pub(crate) fn save(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image(args)?;
    let path = match (args.str("path"), &doc.path, &doc.source) {
        (Some(p), _, _) => p.to_owned(),
        (None, Some(p), _) => p.clone(),
        (None, None, Some((src, format))) => {
            return Err(AgentError::tool(format!(
                "document `{id}` was opened from the {format} file `{src}`; doc.save writes OpenRaster projects only, so pass a `.ora` path to doc.save (keeps the layers), or use export.png to write a flat image"
            )));
        }
        (None, None, None) => {
            return Err(AgentError::tool(format!(
                "document `{id}` was never opened from or saved to a file; pass a `path` (e.g. \"image.ora\")"
            )));
        }
    };
    if !path.to_ascii_lowercase().ends_with(".ora") {
        return Err(AgentError::tool(format!(
            "doc.save writes Image documents as OpenRaster (.ora) projects; `{path}` has another extension (use export.png, export.jpeg or export.webp for flat images)"
        )));
    }
    let bytes = to_ora_bytes(doc.document(), PNG_LEVEL)
        .map_err(|e| AgentError::tool(format!("cannot encode `{path}`: {e}")))?;
    ctx.host
        .write(&path, &bytes)
        .map_err(|e| AgentError::tool(format!("cannot save `{path}`: {e}")))?;
    let (_, doc) = ctx.ws.image_mut(args)?;
    doc.path = Some(path.clone());
    doc.dirty = false;
    Ok(ToolOutput::json(json!({ "doc": id, "path": path })))
}

/// Write `bytes` of an export and report it.
fn write_export(
    ctx: &mut Ctx<'_>,
    id: &str,
    path: &str,
    bytes: &[u8],
    size: (u32, u32),
) -> Result<ToolOutput, AgentError> {
    ctx.host
        .write(path, bytes)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "path": path,
        "width": size.0,
        "height": size.1,
    })))
}

/// The composed image, as the editor exports it.
fn flat(doc: &ImageDoc) -> ImageBuffer {
    compose(doc.document().layers())
}

/// `export.png`: the composed image as the editor's PNG export writes it,
/// optionally upscaled by `scale` with nearest-neighbour.
pub(crate) fn export_png(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image(args)?;
    let path = args.req_str("path")?.to_owned();
    check_frame(args)?;
    let scale = args.u32("scale")?.unwrap_or(1);
    let (w, h) = check_edges(doc.width(), doc.height(), scale, EXPORT_MAX_EDGE)?;
    let image = flat(doc);
    let png = if scale == 1 {
        to_png_bytes(&image, PNG_LEVEL).map_err(|e| AgentError::tool(e.to_string()))?
    } else {
        scaled_png(image.data(), image.width(), image.height(), scale, false)?.0
    };
    write_export(ctx, &id, &path, &png, (w, h))
}

fn export_schema(quality: bool) -> Value {
    let mut props = json!({ "path": path_prop("Workspace-relative output path") });
    if quality {
        props["quality"] = json!({ "type": "integer", "minimum": 1, "maximum": 100, "description": "JPEG quality; default 90 (as the editor)" });
    }
    tool_schema(props, &["path"])
}

pub(crate) const EXPORT_JPEG: Tool = Tool {
    name: "export.jpeg",
    description: "Image documents: export the composed image as a JPEG file (workspace-relative `path`) with `quality` 1–100 (default 90), exactly as the editor's JPEG export (transparency is flattened the same way).",
    schema: || export_schema(true),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.image(args)?;
        let path = args.req_str("path")?.to_owned();
        let quality = args.i64("quality").map_or(JPEG_QUALITY, |q| q as u64);
        let quality =
            u8::try_from(quality).map_err(|_| AgentError::tool("quality must be 1–100"))?;
        let size = (doc.width(), doc.height());
        let bytes =
            to_jpeg_bytes(&flat(doc), quality).map_err(|e| AgentError::tool(e.to_string()))?;
        write_export(ctx, &id, &path, &bytes, size)
    },
};

pub(crate) const EXPORT_WEBP: Tool = Tool {
    name: "export.webp",
    description: "Image documents: export the composed image as a lossless WebP file (workspace-relative `path`), as the editor's WebP export.",
    schema: || export_schema(false),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.image(args)?;
        let path = args.req_str("path")?.to_owned();
        let size = (doc.width(), doc.height());
        let bytes = to_webp_bytes(&flat(doc)).map_err(|e| AgentError::tool(e.to_string()))?;
        write_export(ctx, &id, &path, &bytes, size)
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_format_reads_signatures() {
        assert_eq!(flat_format(b"\x89PNG\r\n"), "PNG");
        assert_eq!(flat_format(&[0xFF, 0xD8, 0xFF]), "JPEG");
        assert_eq!(flat_format(b"RIFF\0\0\0\0WEBPVP8L"), "WebP");
        assert_eq!(flat_format(b"nothing"), "image");
    }
}
