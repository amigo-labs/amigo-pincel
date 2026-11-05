//! `palette.*`: the Pixel palette, its entries and palette files. See
//! docs/specs/mcp-headless-pixel.md AC-18 and
//! docs/specs/mcp-parity-structure.md AC-17 … AC-23, AC-30.

use pincel_core::{
    AddPaletteEntry, ColorMode, MAX_PALETTE_ENTRIES, MovePaletteEntry, Palette, PaletteEntry,
    PalettePreset, RemovePaletteEntry, ReplacePalette, SetPaletteColor, palette_to_png,
    read_palette, write_gpl, write_hex, write_jasc_pal,
};
use serde_json::{Value, json};

use super::{path_prop, tool_schema};
use crate::args::{Args, color_schema, hex, parse_color, parse_hex};
use crate::workspace::{Doc, color_mode_name};
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// The palette as `palette.get` reports it: every entry's color and name.
fn palette_json(palette: &Palette) -> Value {
    let entries: Vec<Value> = palette
        .colors
        .iter()
        .map(|e| json!({ "color": hex(e.rgba), "name": e.name.clone().unwrap_or_default() }))
        .collect();
    json!({ "size": entries.len(), "entries": entries })
}

fn palette_out(id: &str, doc: &Doc) -> ToolOutput {
    let mut out = palette_json(&doc.sprite.palette);
    out["doc"] = json!(id);
    ToolOutput::json(out)
}

/// The argument `key` as an index into the palette; the error names the
/// argument and its value (AC-27).
fn entry_arg(doc: &Doc, args: &Args, key: &str) -> Result<usize, AgentError> {
    let index = args.req_u32(key)? as usize;
    let len = doc.sprite.palette.len();
    if index < len {
        Ok(index)
    } else {
        Err(AgentError::tool(format!(
            "argument `{key}`: palette index {index} does not exist (the palette has {len} entries, indices 0–{})",
            len.saturating_sub(1)
        )))
    }
}

/// Refuse a change that would re-point pixels of an indexed or grayscale
/// document (AC-19, AC-21).
fn require_rgba(doc: &Doc, what: &str) -> Result<(), AgentError> {
    if doc.sprite.color_mode == ColorMode::Rgba {
        return Ok(());
    }
    Err(AgentError::tool(format!(
        "{what} works on rgba documents only; this document's color mode is {}, whose pixels reference palette entries by position",
        color_mode_name(doc.sprite.color_mode)
    )))
}

pub(super) const PALETTE_GET: Tool = Tool {
    name: "palette.get",
    description: "Return the palette: each entry's color (#RRGGBBAA) and name (empty when unnamed); `{\"index\": n}` colors refer to these indices.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        Ok(palette_out(&id, doc))
    },
};

pub(super) const PALETTE_SET: Tool = Tool {
    name: "palette.set",
    description: "Replace the whole palette with `entries` (1–256 colors as \"#RRGGBB\" or \"#RRGGBBAA\"). Pixels keep their colors.",
    schema: || {
        tool_schema(
            json!({
                "entries": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 256,
                    "items": { "type": "string", "description": "\"#RRGGBB\" or \"#RRGGBBAA\"" },
                },
            }),
            &["entries"],
        )
    },
    run: palette_set,
};

fn palette_set(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let entries = args
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| AgentError::tool("missing required argument `entries`"))?;
    let mut colors = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        let c = e.as_str().and_then(parse_hex).ok_or_else(|| {
            AgentError::tool(format!(
                "argument `entries[{i}]`: malformed color {e}; accepted forms: \"#RRGGBB\", \"#RRGGBBAA\""
            ))
        })?;
        colors.push(PaletteEntry::new(c));
    }
    doc.commit(vec![
        ReplacePalette::new(Palette::from_entries(colors)).into(),
    ])?;
    Ok(palette_out(&id, doc))
}

pub(super) const PALETTE_LOAD_PRESET: Tool = Tool {
    name: "palette.load_preset",
    description: "Replace the palette with a built-in preset: db32 (DawnBringer 32), db16, pico8, gameboy or nes. The presets are also readable as pincel://palettes/{id} resources.",
    schema: || {
        let ids: Vec<&str> = PalettePreset::ALL.iter().map(|p| p.name()).collect();
        tool_schema(
            json!({ "preset": { "type": "string", "enum": ids, "description": "Preset id" } }),
            &["preset"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let name = args.req_str("preset")?;
        let preset = PalettePreset::from_name(name)
            .ok_or_else(|| AgentError::tool(format!("unknown palette preset `{name}`")))?;
        doc.commit(vec![ReplacePalette::new(Palette::preset(preset)).into()])?;
        Ok(palette_out(&id, doc))
    },
};

fn index_prop(description: &str) -> Value {
    json!({ "type": "integer", "minimum": 0, "description": description })
}

pub(super) const PALETTE_SET_ENTRY: Tool = Tool {
    name: "palette.set_entry",
    description: "Change the color of one palette entry. Works in every color mode; in an indexed document the pixels using the entry show the new color.",
    schema: || {
        tool_schema(
            json!({
                "index": index_prop("Palette index from palette.get"),
                "color": color_schema("New color"),
            }),
            &["index", "color"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let index = entry_arg(doc, args, "index")?;
        let color = args
            .get("color")
            .ok_or_else(|| AgentError::tool("missing required argument `color`"))?;
        let color = parse_color(color, "color", &doc.sprite.palette)?;
        doc.commit_if_changed(|| vec![SetPaletteColor::new(index, color).into()])?;
        Ok(palette_out(&id, doc))
    },
};

pub(super) const PALETTE_ADD_ENTRY: Tool = Tool {
    name: "palette.add_entry",
    description: "Append a color to the palette (at most 256 entries) and return its index.",
    schema: || {
        tool_schema(
            json!({ "color": color_schema("Color to append") }),
            &["color"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        if doc.sprite.palette.len() >= MAX_PALETTE_ENTRIES {
            return Err(AgentError::tool(format!(
                "the palette is full ({MAX_PALETTE_ENTRIES} entries); remove an entry first"
            )));
        }
        let color = args
            .get("color")
            .ok_or_else(|| AgentError::tool("missing required argument `color`"))?;
        let color = parse_color(color, "color", &doc.sprite.palette)?;
        doc.commit(vec![AddPaletteEntry::new(PaletteEntry::new(color)).into()])?;
        let mut out = palette_out(&id, doc);
        out.json["index"] = json!(doc.sprite.palette.len() - 1);
        Ok(out)
    },
};

pub(super) const PALETTE_REMOVE_ENTRY: Tool = Tool {
    name: "palette.remove_entry",
    description: "Remove one palette entry; later entries move down by one. rgba documents only (indexed and grayscale pixels reference entries by position).",
    schema: || {
        tool_schema(
            json!({ "index": index_prop("Palette index from palette.get") }),
            &["index"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        require_rgba(doc, "palette.remove_entry")?;
        let index = entry_arg(doc, args, "index")?;
        doc.commit(vec![RemovePaletteEntry::new(index).into()])?;
        Ok(palette_out(&id, doc))
    },
};

pub(super) const PALETTE_MOVE_ENTRY: Tool = Tool {
    name: "palette.move_entry",
    description: "Move the palette entry `from` to index `to`; the entries in between shift by one. rgba documents only.",
    schema: || {
        tool_schema(
            json!({
                "from": index_prop("Index of the entry to move"),
                "to": index_prop("Index the entry ends up at"),
            }),
            &["from", "to"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        require_rgba(doc, "palette.move_entry")?;
        let from = entry_arg(doc, args, "from")?;
        let to = entry_arg(doc, args, "to")?;
        doc.commit_if_changed(|| vec![MovePaletteEntry::new(from, to).into()])?;
        Ok(palette_out(&id, doc))
    },
};

pub(super) const PALETTE_IMPORT: Tool = Tool {
    name: "palette.import",
    description: "Replace the palette with a palette file from the workspace: GIMP .gpl (entry names are kept), Lospec .hex, JASC .pal, or a PNG palette image (every distinct color in reading order). The format is detected from the content, not the extension. At most 256 colors; rgba documents only. Returns the number of colors.",
    schema: || {
        tool_schema(
            json!({ "path": path_prop("Workspace-relative palette file") }),
            &["path"],
        )
    },
    run: palette_import,
};

fn palette_import(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let path = args.req_str("path")?;
    {
        let (_, doc) = ctx.ws.doc(args)?;
        require_rgba(doc, "palette.import")?;
    }
    let bytes = ctx
        .host
        .read(path)
        .map_err(|e| AgentError::tool(format!("cannot read `{path}`: {e}")))?;
    let palette = read_palette(&bytes).map_err(|e| {
        AgentError::tool(format!(
            "cannot import `{path}`: {e} (palette.import reads GIMP .gpl, Lospec .hex, JASC .pal and PNG palette images of 1–256 colors)"
        ))
    })?;
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let count = palette.len();
    doc.commit(vec![ReplacePalette::new(palette).into()])?;
    let mut out = palette_out(&id, doc);
    out.json["colors"] = json!(count);
    Ok(out)
}

/// Default edge of one PNG swatch, as in the editor.
const DEFAULT_CELL: u32 = 8;

pub(super) const PALETTE_EXPORT: Tool = Tool {
    name: "palette.export",
    description: "Write the palette to a workspace file in `format`, whatever the path's extension: gpl (GIMP, named `name`, default \"palette\"), hex (Lospec), pal (JASC) or png (a one-row strip of `cell`×`cell` px swatches, default 8). gpl, hex and pal store no alpha: it is dropped.",
    schema: || {
        tool_schema(
            json!({
                "path": path_prop("Workspace-relative output file"),
                "format": { "type": "string", "enum": ["gpl", "hex", "pal", "png"], "description": "File format" },
                "name": { "type": "string", "description": "Palette name stored in a gpl file; default \"palette\"" },
                "cell": { "type": "integer", "minimum": 1, "maximum": 64, "description": "Swatch edge in px for png; default 8" },
            }),
            &["path", "format"],
        )
    },
    run: palette_export,
};

fn palette_export(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    let path = args.req_str("path")?;
    let format = args.req_str("format")?;
    let palette = &doc.sprite.palette;
    let bytes = match format {
        "gpl" => write_gpl(palette, args.str("name").unwrap_or("palette")).into_bytes(),
        "hex" => write_hex(palette).into_bytes(),
        "pal" => write_jasc_pal(palette).into_bytes(),
        "png" => {
            if palette.is_empty() {
                return Err(AgentError::tool(
                    "cannot export an empty palette as png: there is no swatch to draw",
                ));
            }
            let cell = args.u32("cell")?.unwrap_or(DEFAULT_CELL);
            if !(1..=64).contains(&cell) {
                return Err(AgentError::tool(format!(
                    "argument `cell` must be 1–64 (got {cell})"
                )));
            }
            palette_to_png(palette, cell)
                .map_err(|e| AgentError::tool(format!("cannot export palette: {e}")))?
        }
        other => {
            return Err(AgentError::tool(format!(
                "argument `format`: unknown palette format {other:?} (gpl, hex, pal, png)"
            )));
        }
    };
    let colors = palette.len();
    ctx.host
        .check_write(path)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    ctx.host
        .write(path, &bytes)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "path": path,
        "format": format,
        "colors": colors,
        "bytes": bytes.len(),
    })))
}
