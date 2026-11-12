//! `doc.*` and `export.*` on Image documents: create, open, preview,
//! pixel readback, undo / redo, `.ora` save and flat exports.

use pincel_image_core::codec::{
    decode_hinted, from_ora_bytes, is_jxl, to_bmp_bytes, to_jpeg_bytes, to_ora_bytes, to_png_bytes,
    to_tiff_bytes, to_webp_bytes,
};
use pincel_image_core::{Document, ImageBuffer, Rect, compose, compose_region};
use serde_json::{Value, json};

use pincel_image_core::codec::psd::{PsdFallback, PsdLimits, PsdNotice, from_psd_bytes};

use super::{ImageDoc, MAX_AREA, MAX_SIDE, check_size, core_err};
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
fn flat_format(path: &str, bytes: &[u8]) -> &'static str {
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
    } else if bytes.starts_with(b"8BPS") {
        "PSD"
    } else if bytes.starts_with(b"DDS ") {
        "DDS"
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        "ICO"
    } else if is_jxl(bytes) {
        "JPEG XL"
    } else if path.to_ascii_lowercase().ends_with(".tga") {
        "TGA"
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

/// The detected format of a file's content, for error messages.
pub(crate) fn detected_format(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"PK\x03\x04") {
        "OpenRaster"
    } else {
        match flat_format("", bytes) {
            "image" => "unknown",
            f => f,
        }
    }
}

/// One English line per item the editor's PSD open notice lists
/// (mcp-parity-export AC-21).
fn psd_notice(n: &PsdNotice) -> String {
    match n {
        PsdNotice::Fallback { reason } => {
            let why = match reason {
                PsdFallback::NoLayers => "the file has no layers",
                PsdFallback::Compressed => "the layers are ZIP-compressed",
                PsdFallback::Unreadable => "the layer section cannot be read",
                PsdFallback::Unrepresentable => "no layer can be represented",
            };
            format!("opened as one flattened layer: {why}")
        }
        PsdNotice::Adjustment { layer } => format!("adjustment or fill layer {layer:?} left out"),
        PsdNotice::Text { layer } => format!("text layer {layer:?} opened as pixels"),
        PsdNotice::SmartObject { layer } => format!("smart object {layer:?} opened as pixels"),
        PsdNotice::Effects { layer } => format!("layer effects of {layer:?} left out"),
        PsdNotice::Mask { layer } => format!("mask of {layer:?} left out"),
        PsdNotice::Clipping { layer } => format!("clipping mask of {layer:?} left out"),
        PsdNotice::Blend { layer, key } => {
            format!("blend mode {key:?} of {layer:?} opened as Normal")
        }
        PsdNotice::PassThrough { group } => {
            format!("pass-through group {group:?} opened as an isolated Normal group")
        }
    }
}

/// `true` when the content is a PSD, or (content unknown) the name ends
/// in `.psd`, as the editor recognizes it.
fn is_psd(path: &str, bytes: &[u8]) -> bool {
    bytes.starts_with(b"8BPS")
        || (flat_format(path, bytes) == "image"
            && !bytes.starts_with(b"PK\x03\x04")
            && path.to_ascii_lowercase().ends_with(".psd"))
}

/// `doc.open` of an `.ora` project, a PSD or a flat image (PNG, JPEG,
/// WebP, BMP, GIF, TIFF, TGA, DDS, ICO, JPEG XL), decoded as the editor's
/// Open does.
pub(crate) fn open(ctx: &mut Ctx<'_>, path: &str) -> Result<ToolOutput, AgentError> {
    let fail = |e: String| AgentError::tool(format!("cannot open `{path}`: {e}"));
    let bytes = ctx.host.read(path).map_err(fail)?;
    if is_psd(path, &bytes) {
        let limits = PsdLimits {
            max_dim: MAX_SIDE,
            max_area: MAX_AREA,
        };
        let import = from_psd_bytes(&bytes, limits).map_err(|e| fail(e.to_string()))?;
        let notices: Vec<String> = import.notices.iter().map(psd_notice).collect();
        let doc = ImageDoc::new(import.document, None, Some((path.to_owned(), "PSD")));
        let mut info = doc.info("");
        let id = ctx.ws.insert_image(doc);
        info["notices"] = json!(notices);
        return Ok(ToolOutput::json(with_id(info, &id)));
    }
    let doc = if is_ora(path, &bytes) {
        let document = from_ora_bytes(&bytes).map_err(|e| fail(e.to_string()))?;
        check_size(document.canvas.width(), document.canvas.height())
            .map_err(|e| fail(e.to_string()))?;
        ImageDoc::new(document, Some(path.to_owned()), None)
    } else {
        // TGA has no signature: the path is its hint
        // (docs/specs/image-formats-more.md AC-15).
        let pixels = decode_hinted(&bytes, Some(path)).map_err(|e| fail(e.to_string()))?;
        check_size(pixels.width(), pixels.height()).map_err(|e| fail(e.to_string()))?;
        let document = Document::from_pixels(pixels).map_err(|e| fail(e.to_string()))?;
        ImageDoc::new(
            document,
            None,
            Some((path.to_owned(), flat_format(path, &bytes))),
        )
    };
    let info = doc.info("");
    let id = ctx.ws.insert_image(doc);
    Ok(ToolOutput::json(with_id(info, &id)))
}

/// Image documents have one frame; `frame` may only name it.
pub(crate) fn check_frame(args: &Args) -> Result<(), AgentError> {
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

/// The extensions `doc.save` writes for Image documents.
const SAVE_EXTENSIONS: &str = ".ora, .png, .jpg / .jpeg, .webp, .bmp, .tif / .tiff";

/// The format `doc.save` writes for `path`'s extension.
fn save_format(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    let ext = lower.rsplit_once('.').map(|(_, e)| e)?;
    match ext {
        "ora" => Some("ORA"),
        "png" => Some("PNG"),
        "jpg" | "jpeg" => Some("JPEG"),
        "webp" => Some("WebP"),
        "bmp" => Some("BMP"),
        "tif" | "tiff" => Some("TIFF"),
        _ => None,
    }
}

/// What a flat save would lose (the editor's `flatteningLoses`), one entry
/// per cause (mcp-parity-export AC-15).
fn flatten_losses(doc: &ImageDoc) -> Vec<String> {
    let layers = doc.document().layers();
    let mut out = Vec::new();
    if layers.len() > 1 {
        out.push(format!("{} layers", layers.len()));
    }
    for l in layers {
        if !l.visible {
            out.push(format!("hidden layer {:?}", l.name));
        }
        if l.opacity < 1.0 {
            out.push(format!(
                "layer {:?} at {} % opacity",
                l.name,
                (l.opacity * 100.0).round()
            ));
        }
        if l.blend_mode != pincel_image_core::BlendMode::Normal {
            out.push(format!(
                "layer {:?} with blend mode {:?}",
                l.name, l.blend_mode
            ));
        }
    }
    out
}

/// `doc.save`: the document in the format of the path's extension, or
/// back to the file it came from (mcp-parity-export AC-14 … AC-19).
pub(crate) fn save(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image(args)?;
    let layered = doc.document().layer_count() > 1;
    let (path, format) = match (args.str("path"), &doc.path, &doc.source) {
        (Some(p), _, _) => {
            let format = save_format(p).ok_or_else(|| {
                // A PSD source or several layers: `.ora` first, as the
                // editor's Save As preselects it (AC-19).
                let psd = matches!(&doc.source, Some((_, "PSD")));
                AgentError::tool(if psd || layered {
                    format!(
                        "doc.save cannot write `{p}`: pass a `.ora` path (keeps the layers) or a path ending in {SAVE_EXTENSIONS}"
                    )
                } else {
                    format!(
                        "doc.save cannot write `{p}`: Image documents are saved as {SAVE_EXTENSIONS}"
                    )
                })
            })?;
            (p.to_owned(), format)
        }
        (None, Some(p), _) => (p.clone(), "ORA"),
        (None, None, Some((src, format))) => match *format {
            "PNG" | "JPEG" | "WebP" | "BMP" | "TIFF" => (src.clone(), *format),
            other => {
                let hint = if other == "PSD" || layered {
                    "pass a `.ora` path (keeps the layers) or "
                } else {
                    "pass "
                };
                return Err(AgentError::tool(format!(
                    "document `{id}` was opened from the {other} file `{src}`, which doc.save cannot write; {hint}a path ending in {SAVE_EXTENSIONS}"
                )));
            }
        },
        (None, None, None) => {
            return Err(AgentError::tool(format!(
                "document `{id}` was never opened from or saved to a file; pass a `path` (e.g. \"image.ora\")"
            )));
        }
    };
    let quality = match args.i64("quality") {
        Some(q) => Some(
            u8::try_from(q)
                .ok()
                .filter(|q| (1..=100).contains(q))
                .ok_or_else(|| AgentError::tool("argument `quality` must be an integer 1–100"))?,
        ),
        None => None,
    };
    let bytes = if format == "ORA" {
        to_ora_bytes(doc.document(), PNG_LEVEL).map_err(|e| e.to_string())
    } else {
        let losses = flatten_losses(doc);
        if !losses.is_empty() && !args.bool("flatten").unwrap_or(false) {
            return Err(AgentError::tool(format!(
                "saving to the flat {format} file `{path}` would flatten the document ({}); pass `flatten: true` to write the composite (the document keeps its layers), or save to a `.ora` path",
                losses.join(", ")
            )));
        }
        let image = flat(doc);
        match format {
            "PNG" => to_png_bytes(&image, PNG_LEVEL),
            "JPEG" => to_jpeg_bytes(
                &image,
                quality.or(doc.jpeg_quality).unwrap_or(JPEG_QUALITY as u8),
            ),
            "WebP" => to_webp_bytes(&image),
            "BMP" => to_bmp_bytes(&image),
            _ => to_tiff_bytes(&image),
        }
        .map_err(|e| e.to_string())
    }
    .map_err(|e| AgentError::tool(format!("cannot encode `{path}`: {e}")))?;
    ctx.host
        .write(&path, &bytes)
        .map_err(|e| AgentError::tool(format!("cannot save `{path}`: {e}")))?;
    let (_, doc) = ctx.ws.image_mut(args)?;
    if format == "ORA" {
        doc.path = Some(path.clone());
    } else {
        doc.path = None;
        doc.source = Some((path.clone(), format));
        if format == "JPEG" {
            doc.jpeg_quality = Some(quality.or(doc.jpeg_quality).unwrap_or(JPEG_QUALITY as u8));
        }
    }
    doc.dirty = false;
    Ok(ToolOutput::json(
        json!({ "doc": id, "path": path, "format": format }),
    ))
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

pub(crate) const EXPORT_BMP: Tool = Tool {
    name: "export.bmp",
    description: "Image documents: export the composed image as a BMP file (workspace-relative `path`), as the editor's BMP save.",
    schema: || export_schema(false),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.image(args)?;
        let path = args.req_str("path")?.to_owned();
        let size = (doc.width(), doc.height());
        let bytes = to_bmp_bytes(&flat(doc)).map_err(|e| AgentError::tool(e.to_string()))?;
        write_export(ctx, &id, &path, &bytes, size)
    },
};

pub(crate) const EXPORT_TIFF: Tool = Tool {
    name: "export.tiff",
    description: "Image documents: export the composed image as a TIFF file (workspace-relative `path`), as the editor's TIFF save.",
    schema: || export_schema(false),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.image(args)?;
        let path = args.req_str("path")?.to_owned();
        let size = (doc.width(), doc.height());
        let bytes = to_tiff_bytes(&flat(doc)).map_err(|e| AgentError::tool(e.to_string()))?;
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
        assert_eq!(flat_format("a.png", b"\x89PNG\r\n"), "PNG");
        assert_eq!(flat_format("a.jpg", &[0xFF, 0xD8, 0xFF]), "JPEG");
        assert_eq!(flat_format("a.webp", b"RIFF\0\0\0\0WEBPVP8L"), "WebP");
        assert_eq!(flat_format("a.bin", b"nothing"), "image");
        assert_eq!(flat_format("A.TGA", b"\0\0\x02"), "TGA");
        assert_eq!(flat_format("x", b"DDS |"), "DDS");
        assert_eq!(flat_format("x.png", &[0xFF, 0x0A, 0]), "JPEG XL");
    }

    #[test]
    fn psd_notice_names_the_layer_or_blend_key_of_each_item() {
        // mcp-parity-export AC-21: one English line per notice, naming
        // what it concerns.
        let layer = || "Ink 2".to_owned();
        for (notice, want) in [
            (
                PsdNotice::Adjustment { layer: layer() },
                "adjustment or fill layer \"Ink 2\" left out",
            ),
            (
                PsdNotice::Text { layer: layer() },
                "text layer \"Ink 2\" opened as pixels",
            ),
            (
                PsdNotice::SmartObject { layer: layer() },
                "smart object \"Ink 2\" opened as pixels",
            ),
            (
                PsdNotice::Effects { layer: layer() },
                "layer effects of \"Ink 2\" left out",
            ),
            (
                PsdNotice::Mask { layer: layer() },
                "mask of \"Ink 2\" left out",
            ),
            (
                PsdNotice::Clipping { layer: layer() },
                "clipping mask of \"Ink 2\" left out",
            ),
            (
                PsdNotice::Blend {
                    layer: layer(),
                    key: "hLit".to_owned(),
                },
                "blend mode \"hLit\" of \"Ink 2\" opened as Normal",
            ),
            (
                PsdNotice::PassThrough { group: layer() },
                "pass-through group \"Ink 2\" opened as an isolated Normal group",
            ),
            (
                PsdNotice::Fallback {
                    reason: PsdFallback::Compressed,
                },
                "opened as one flattened layer: the layers are ZIP-compressed",
            ),
        ] {
            assert_eq!(psd_notice(&notice), want);
        }
    }
}
