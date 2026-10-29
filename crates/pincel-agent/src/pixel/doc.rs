//! `doc.*` and `export.*`: document lifecycle, preview, pixel readback,
//! undo / redo, save and export.

use pincel_core::{
    AtlasOptions, CelData, FrameIndex, LayerKind, Palette, PalettePreset, Rect, Rgba,
    aseprite_sheet_json, export_atlas_png, import_png, read_aseprite, write_aseprite,
};
use serde_json::{Value, json};

use super::{frame_prop, layer_prop, tool_schema};
use crate::args::{Args, hex, inside_canvas, parse_region, region_schema};
use crate::preview::{
    GRID_MIN_SCALE, PREVIEW_MAX_EDGE, auto_scale, compose_region, encode_png, render_png, upscale,
};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// Largest edge of an exported PNG.
const EXPORT_MAX_EDGE: u32 = 16_384;
/// Largest `doc.read_pixels` region edge (AC-14).
const READ_MAX: u32 = 64;
/// Editor bound for a new Pixel document's sides (`NEW_DOC_LIMITS`).
const NEW_DOC_MAX: u32 = 4096;

fn preset_ids() -> Vec<&'static str> {
    PalettePreset::ALL.iter().map(|p| p.name()).collect()
}

fn path_prop(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn scale_prop() -> Value {
    json!({ "type": "integer", "minimum": 1, "maximum": 256, "description": "Integer upscale factor; every document pixel becomes a scale×scale block" })
}

fn is_aseprite(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".aseprite") || lower.ends_with(".ase")
}

pub(super) const NEW: Tool = Tool {
    name: "doc.new",
    description: "Create a new in-memory document and make it the active document. Pixel documents start like the editor's New document: one image layer, one 100 ms frame, the DB32 palette (or `palette`). Nothing is written to disk until doc.save.",
    schema: || {
        tool_schema(
            json!({
                "mode": { "type": "string", "enum": ["pixel"], "description": "Document mode" },
                "width": { "type": "integer", "minimum": 1, "maximum": NEW_DOC_MAX, "description": "Canvas width in pixels" },
                "height": { "type": "integer", "minimum": 1, "maximum": NEW_DOC_MAX, "description": "Canvas height in pixels" },
                "palette": { "type": "string", "enum": preset_ids(), "description": "Palette preset id; default db32" },
            }),
            &["mode", "width", "height"],
        )
    },
    run: doc_new,
};

fn doc_new(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let width = args.req_u32("width")?;
    let height = args.req_u32("height")?;
    let palette = match args.str("palette") {
        Some(name) => Palette::preset(
            PalettePreset::from_name(name)
                .ok_or_else(|| AgentError::tool(format!("unknown palette preset `{name}`")))?,
        ),
        None => Palette::db32(),
    };
    let doc = Doc::new_pixel(width, height, palette)?;
    let info = doc.info("");
    let id = ctx.ws.insert(doc);
    Ok(ToolOutput::json(with_id(info, &id)))
}

fn with_id(mut info: Value, id: &str) -> Value {
    info["doc"] = json!(id);
    info
}

pub(super) const OPEN: Tool = Tool {
    name: "doc.open",
    description: "Open a workspace-relative .aseprite file as a new Pixel document and make it active.",
    schema: || {
        tool_schema(
            json!({ "path": path_prop("Workspace-relative path of an .aseprite file") }),
            &["path"],
        )
    },
    run: doc_open,
};

fn doc_open(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let path = args.req_str("path")?;
    if !is_aseprite(path) {
        return Err(AgentError::tool(format!(
            "cannot open `{path}`: only .aseprite / .ase files open as Pixel documents"
        )));
    }
    let bytes = ctx
        .host
        .read(path)
        .map_err(|e| AgentError::tool(format!("cannot open `{path}`: {e}")))?;
    let out = read_aseprite(&bytes)
        .map_err(|e| AgentError::tool(format!("cannot open `{path}`: {e}")))?;
    let doc = Doc::from_parts(out.sprite, out.cels, Some(path.to_owned()));
    let info = doc.info("");
    let id = ctx.ws.insert(doc);
    Ok(ToolOutput::json(with_id(info, &id)))
}

pub(super) const LIST: Tool = Tool {
    name: "doc.list",
    description: "List the open documents with size, layer / frame / tag counts, palette size, active layer and frame, and whether they have unsaved changes.",
    schema: || tool_schema(json!({}), &[]),
    run: doc_list,
};

fn doc_list(ctx: &mut Ctx<'_>, _args: &Args) -> Result<ToolOutput, AgentError> {
    let docs: Vec<Value> = ctx.ws.iter().map(|(id, d)| d.info(id)).collect();
    Ok(ToolOutput::json(
        json!({ "active": ctx.ws.active(), "documents": docs }),
    ))
}

pub(super) const INFO: Tool = Tool {
    name: "doc.info",
    description: "Describe one document: size, layer / frame / tag counts, palette size, active layer and frame, dirty state, file path and undo / redo depth.",
    schema: || tool_schema(json!({}), &[]),
    run: doc_info,
};

fn doc_info(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    Ok(ToolOutput::json(doc.info(&id)))
}

pub(super) const CLOSE: Tool = Tool {
    name: "doc.close",
    description: "Close a document, discarding unsaved changes (check `dirty` in doc.info and doc.save first). The most recently opened remaining document becomes active.",
    schema: || tool_schema(json!({}), &[]),
    run: doc_close,
};

fn doc_close(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let id = ctx.ws.resolve(args)?;
    ctx.ws.remove(&id);
    Ok(ToolOutput::json(
        json!({ "closed": id, "active": ctx.ws.active(), "open": ctx.ws.len() }),
    ))
}

pub(super) const PREVIEW: Tool = Tool {
    name: "doc.preview",
    description: "Render a frame as a PNG image so you can see the result. Pixel-exact nearest-neighbour upscale: without `scale` the longest side becomes at least 256 px. The PNG is capped at 2048 px per edge; use `region` for large canvases. `grid: true` draws a 50 % grey line on every pixel block (scale 4 and up).",
    schema: || {
        tool_schema(
            json!({
                "frame": { "type": "integer", "minimum": 0, "description": "Frame index; default 0" },
                "scale": scale_prop(),
                "grid": { "type": "boolean", "description": "Draw a pixel grid (only at scale 4 and up)" },
                "region": region_schema("Only this rectangle of the canvas (document pixels)"),
            }),
            &[],
        )
    },
    run: doc_preview,
};

fn doc_preview(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    let frame = doc.frame_arg(args)?.unwrap_or(FrameIndex::new(0));
    let region = canvas_region(doc, args)?;
    let scale = args
        .u32("scale")?
        .unwrap_or_else(|| auto_scale(region.width, region.height));
    let want_grid = args.bool("grid").unwrap_or(false);
    let grid = want_grid && scale >= GRID_MIN_SCALE;
    let (png, width, height) = render_png(
        &doc.sprite,
        &doc.cels,
        frame,
        region,
        scale,
        grid,
        PREVIEW_MAX_EDGE,
    )?;
    let mut json = json!({
        "doc": id,
        "frame": frame.0,
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

fn rect_json(r: Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "width": r.width, "height": r.height })
}

/// The `region` argument (default: the whole canvas), refused when it
/// leaves the canvas.
fn canvas_region(doc: &Doc, args: &Args) -> Result<Rect, AgentError> {
    let (w, h) = (doc.sprite.width, doc.sprite.height);
    let Some(v) = args.get("region") else {
        return Ok(Rect::new(0, 0, w, h));
    };
    let r = parse_region(v)?;
    if !inside_canvas(r, w, h) {
        return Err(AgentError::tool(format!(
            "region ({}, {}, {}×{}) lies outside the {w}×{h} canvas",
            r.x, r.y, r.width, r.height
        )));
    }
    Ok(r)
}

pub(super) const READ_PIXELS: Tool = Tool {
    name: "doc.read_pixels",
    description: "Read the exact colors of a region (at most 64×64) as text: one row per pixel row. Without `layer` it reads the composed frame, with `layer` that layer's cel. format `hex` gives #RRGGBBAA per pixel; `index` gives the first palette index with exactly that color, or `-`.",
    schema: || {
        tool_schema(
            json!({
                "region": region_schema("Rectangle to read, at most 64×64, inside the canvas"),
                "layer": layer_prop(),
                "frame": frame_prop(),
                "format": { "type": "string", "enum": ["hex", "index"], "description": "Pixel format; default hex" },
            }),
            &["region"],
        )
    },
    run: doc_read_pixels,
};

fn doc_read_pixels(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    let v = args
        .get("region")
        .ok_or_else(|| AgentError::tool("missing required argument `region`"))?;
    let r = parse_region(v)?;
    if r.width > READ_MAX || r.height > READ_MAX {
        return Err(AgentError::tool(format!(
            "region {}×{} is larger than the {READ_MAX}×{READ_MAX} limit of doc.read_pixels; read it in parts",
            r.width, r.height
        )));
    }
    let (w, h) = (doc.sprite.width, doc.sprite.height);
    if !inside_canvas(r, w, h) {
        return Err(AgentError::tool(format!(
            "region ({}, {}, {}×{}) lies outside the {w}×{h} canvas",
            r.x, r.y, r.width, r.height
        )));
    }
    let frame = doc.frame_arg(args)?.unwrap_or(doc.active_frame);
    let pixels = match args.u32("layer")? {
        None => compose_region(&doc.sprite, &doc.cels, frame, r)?,
        Some(layer) => layer_pixels(doc, layer, frame, r)?,
    };
    let index = args.str("format") == Some("index");
    let rows: Vec<String> = pixels
        .chunks_exact(r.width as usize * 4)
        .map(|row| {
            row.as_chunks::<4>()
                .0
                .iter()
                .map(|p| {
                    let c = Rgba::new(p[0], p[1], p[2], p[3]);
                    if index {
                        palette_index(&doc.sprite.palette, c)
                    } else {
                        hex(c)
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    Ok(ToolOutput::json(json!({
        "doc": id,
        "frame": frame.0,
        "region": rect_json(r),
        "format": if index { "index" } else { "hex" },
        "rows": rows,
    })))
}

fn palette_index(palette: &Palette, c: Rgba) -> String {
    palette
        .colors
        .iter()
        .position(|e| e.rgba == c)
        .map_or_else(|| "-".to_owned(), |i| i.to_string())
}

/// The cel pixels of `layer` at `frame` inside `r` (sprite coordinates);
/// pixels outside the cel read as transparent.
fn layer_pixels(doc: &Doc, layer: u32, frame: FrameIndex, r: Rect) -> Result<Vec<u8>, AgentError> {
    let id = pincel_core::LayerId::new(layer);
    let l = doc
        .sprite
        .layer(id)
        .ok_or_else(|| AgentError::tool(format!("no layer with id {layer}")))?;
    if !matches!(l.kind, LayerKind::Image) {
        return Err(AgentError::tool(format!(
            "layer {layer} ({:?}) is not an image layer",
            l.name
        )));
    }
    let mut out = vec![0u8; r.width as usize * r.height as usize * 4];
    let Some(cel) = doc.cels.get(id, frame) else {
        return Ok(out);
    };
    let CelData::Image(buf) = &cel.data else {
        return Ok(out);
    };
    for row in 0..r.height as i64 {
        for col in 0..r.width as i64 {
            let lx = i64::from(r.x) + col - i64::from(cel.position.0);
            let ly = i64::from(r.y) + row - i64::from(cel.position.1);
            if lx < 0 || ly < 0 || lx >= i64::from(buf.width) || ly >= i64::from(buf.height) {
                continue;
            }
            let src = ((ly as usize) * buf.width as usize + lx as usize) * 4;
            let dst = ((row as usize) * r.width as usize + col as usize) * 4;
            out[dst..dst + 4].copy_from_slice(&buf.data[src..src + 4]);
        }
    }
    Ok(out)
}

pub(super) const UNDO: Tool = Tool {
    name: "doc.undo",
    description: "Undo the last mutating tool call (every call is one step). Reports the new undo and redo depths.",
    schema: || tool_schema(json!({}), &[]),
    run: doc_undo,
};

fn doc_undo(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let undone = doc.bus.undo(&mut doc.sprite, &mut doc.cels);
    if undone {
        doc.dirty = true;
        doc.clamp_active();
    }
    Ok(ToolOutput::json(json!({
        "doc": id,
        "undone": undone,
        "undo_depth": doc.bus.undo_depth(),
        "redo_depth": doc.bus.redo_depth(),
    })))
}

pub(super) const REDO: Tool = Tool {
    name: "doc.redo",
    description: "Redo the last undone tool call. Reports the new undo and redo depths.",
    schema: || tool_schema(json!({}), &[]),
    run: doc_redo,
};

fn doc_redo(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let redone = doc
        .bus
        .redo(&mut doc.sprite, &mut doc.cels)
        .map_err(|e| AgentError::tool(format!("redo failed: {e}")))?;
    if redone {
        doc.dirty = true;
        doc.clamp_active();
    }
    Ok(ToolOutput::json(json!({
        "doc": id,
        "redone": redone,
        "undo_depth": doc.bus.undo_depth(),
        "redo_depth": doc.bus.redo_depth(),
    })))
}

pub(super) const SAVE: Tool = Tool {
    name: "doc.save",
    description: "Write the document as an .aseprite file to `path` (workspace-relative), or without `path` to the file it was opened from or last saved to.",
    schema: || {
        tool_schema(
            json!({ "path": path_prop("Workspace-relative .aseprite path; default: the document's file") }),
            &[],
        )
    },
    run: doc_save,
};

fn doc_save(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    let path = match args.str("path") {
        Some(p) => p.to_owned(),
        None => doc.path.clone().ok_or_else(|| {
            AgentError::tool(format!(
                "document `{id}` was never opened from or saved to a file; pass a `path` (e.g. \"sprite.aseprite\")"
            ))
        })?,
    };
    if !is_aseprite(&path) {
        return Err(AgentError::tool(format!(
            "doc.save writes .aseprite files; `{path}` has another extension (use export.png for PNG)"
        )));
    }
    let mut bytes = Vec::new();
    write_aseprite(&doc.sprite, &doc.cels, &mut bytes)
        .map_err(|e| AgentError::tool(format!("cannot encode `{path}`: {e}")))?;
    ctx.host
        .write(&path, &bytes)
        .map_err(|e| AgentError::tool(format!("cannot save `{path}`: {e}")))?;
    let (_, doc) = ctx.ws.doc_mut(args)?;
    doc.path = Some(path.clone());
    doc.dirty = false;
    Ok(ToolOutput::json(json!({ "doc": id, "path": path })))
}

pub(super) const EXPORT_PNG: Tool = Tool {
    name: "export.png",
    description: "Export one composed frame as a PNG file (workspace-relative `path`), upscaled by `scale` (default 1) with nearest-neighbour.",
    schema: || {
        tool_schema(
            json!({
                "path": path_prop("Workspace-relative .png path"),
                "frame": { "type": "integer", "minimum": 0, "description": "Frame index; default 0" },
                "scale": scale_prop(),
            }),
            &["path"],
        )
    },
    run: export_png,
};

fn export_png(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    let path = args.req_str("path")?;
    let frame = doc.frame_arg(args)?.unwrap_or(FrameIndex::new(0));
    let scale = args.u32("scale")?.unwrap_or(1);
    let region = Rect::new(0, 0, doc.sprite.width, doc.sprite.height);
    let (png, width, height) = render_png(
        &doc.sprite,
        &doc.cels,
        frame,
        region,
        scale,
        false,
        EXPORT_MAX_EDGE,
    )?;
    ctx.host
        .write(path, &png)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "path": path,
        "frame": frame.0,
        "width": width,
        "height": height,
    })))
}

pub(super) const EXPORT_SHEET: Tool = Tool {
    name: "export.sheet",
    description: "Export frames as a sprite-sheet PNG laid out left to right, top to bottom in `columns` columns (only the frames of `tag` when given). With `json`, also write Aseprite sheet data (JSON) naming the PNG and one rectangle per frame.",
    schema: || {
        tool_schema(
            json!({
                "columns": { "type": "integer", "minimum": 1, "description": "Number of columns" },
                "path": path_prop("Workspace-relative .png path of the sheet"),
                "tag": { "type": "string", "description": "Export only this tag's frames" },
                "scale": scale_prop(),
                "json": path_prop("Workspace-relative path of the sheet-data JSON to write as well"),
            }),
            &["columns", "path"],
        )
    },
    run: export_sheet,
};

fn export_sheet(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    let path = args.req_str("path")?;
    let columns = args.req_u32("columns")?;
    let scale = args.u32("scale")?.unwrap_or(1);
    let mut opts = AtlasOptions::new(columns);
    if let Some(tag) = args.str("tag") {
        opts = opts.with_tag(tag);
    }
    let atlas = export_atlas_png(&doc.sprite, &doc.cels, &opts)
        .map_err(|e| AgentError::tool(format!("cannot export sheet: {e}")))?;
    let mut manifest = atlas.manifest;
    let (sheet_w, sheet_h) = (
        u64::from(manifest.width) * u64::from(scale),
        u64::from(manifest.height) * u64::from(scale),
    );
    if sheet_w > u64::from(EXPORT_MAX_EDGE) || sheet_h > u64::from(EXPORT_MAX_EDGE) {
        return Err(AgentError::tool(format!(
            "the sheet would be {sheet_w}×{sheet_h} px, above the {EXPORT_MAX_EDGE} px cap per edge; use more or fewer `columns`, a `tag` or a smaller `scale`"
        )));
    }
    let png = if scale == 1 {
        atlas.png
    } else {
        let img = import_png(&atlas.png)
            .map_err(|e| AgentError::tool(format!("cannot scale sheet: {e}")))?;
        let big = upscale(&img.rgba, img.width, img.height, scale);
        manifest.width *= scale;
        manifest.height *= scale;
        manifest.frame_width *= scale;
        manifest.frame_height *= scale;
        for f in &mut manifest.frames {
            f.x *= scale;
            f.y *= scale;
            f.w *= scale;
            f.h *= scale;
        }
        encode_png(&big, manifest.width, manifest.height)?
    };
    let json_path = args.str("json");
    let data =
        json_path.map(|jp| aseprite_sheet_json(&doc.sprite, &manifest, &image_ref(path, jp)));
    // Every output path is checked before the first write, so a refused
    // second path leaves no half-written export behind (AC-34).
    for p in std::iter::once(path).chain(json_path) {
        ctx.host
            .check_write(p)
            .map_err(|e| AgentError::tool(format!("cannot write `{p}`: {e}")))?;
    }
    ctx.host
        .write(path, &png)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    if let (Some(jp), Some(data)) = (json_path, data) {
        ctx.host
            .write(jp, data.as_bytes())
            .map_err(|e| AgentError::tool(format!("cannot write `{jp}`: {e}")))?;
    }
    Ok(ToolOutput::json(json!({
        "doc": id,
        "path": path,
        "json": json_path,
        "width": manifest.width,
        "height": manifest.height,
        "columns": manifest.columns,
        "rows": manifest.rows,
        "frames": manifest.frames.len(),
    })))
}

/// How the sheet data names the PNG: its file name when both files share a
/// directory, else its workspace-relative path.
fn image_ref(png: &str, json: &str) -> String {
    let split = |p: &str| -> (String, String) {
        let p = p.replace('\\', "/");
        match p.rsplit_once('/') {
            Some((dir, file)) => (dir.to_owned(), file.to_owned()),
            None => (String::new(), p),
        }
    };
    let (png_dir, png_file) = split(png);
    let (json_dir, _) = split(json);
    if png_dir == json_dir {
        png_file
    } else {
        png.replace('\\', "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_ref_uses_file_name_in_the_same_directory() {
        assert_eq!(image_ref("out/hero.png", "out/hero.json"), "hero.png");
        assert_eq!(image_ref("hero.png", "hero.json"), "hero.png");
        assert_eq!(image_ref("img/hero.png", "data/hero.json"), "img/hero.png");
    }

    #[test]
    fn palette_index_finds_first_exact_match_or_dash() {
        let p = Palette::db32();
        assert_eq!(palette_index(&p, p.colors[3].rgba), "3");
        assert_eq!(palette_index(&p, Rgba::new(1, 2, 3, 4)), "-");
    }
}
