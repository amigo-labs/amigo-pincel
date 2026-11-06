//! Moving content and the headless clipboard: `layer.translate`,
//! `select.move_content`, `select.move`, `layer.import` (Paste) and
//! `export.selection` (Copy / Copy Merged / Cut). The clipboard is a PNG
//! file in the workspace. See
//! docs/specs/mcp-parity-selection-transform-content.md.

use pincel_core::{
    AnyCommand, Bus, ClearRegion, ColorMode, CopySource, Layer, LayerId, MoveSelectionContent,
    PasteLayer, PastedImage, PutCel, Rect, copy_pixels, encode_png, import_png,
};
use pincel_image_core::codec::{decode, to_png_bytes};
use pincel_image_core::command::PasteLayer as ImagePaste;
use pincel_image_core::command::spec::CommandSpec;
use pincel_image_core::copy_selection;
use serde_json::{Value, json};

use crate::args::Args;
use crate::image::{ImageDoc, check_size};
use crate::pixel::{frame_prop, layer_id_arg, layer_prop, path_prop, tool_schema};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// Every tool of this module, in `tools/list` order.
pub(crate) static TOOLS: &[Tool] = &[
    LAYER_TRANSLATE,
    SELECT_MOVE_CONTENT,
    SELECT_MOVE,
    LAYER_IMPORT,
    EXPORT_SELECTION,
];

/// Largest Pixel document side, as in the editor.
const PIXEL_MAX: u32 = 4096;

fn offset_schema(with_target: bool) -> Value {
    let mut props = json!({
        "dx": { "type": "integer", "description": "Horizontal offset in canvas px (right is positive)" },
        "dy": { "type": "integer", "description": "Vertical offset in canvas px (down is positive)" },
    });
    if with_target {
        props["layer"] = layer_prop();
        props["frame"] = frame_prop();
    }
    tool_schema(props, &["dx", "dy"])
}

fn offset(args: &Args) -> Result<(i32, i32), AgentError> {
    Ok((args.req_i32("dx")?, args.req_i32("dy")?))
}

/// The Image layer an edit targets (`layer` or the active layer), without
/// the paint checks: the editor command decides what it refuses. A
/// `frame` other than 0 does not exist in an Image document.
fn image_layer(doc: &ImageDoc, args: &Args) -> Result<usize, AgentError> {
    crate::image::doc::check_frame(args)?;
    match args.u32("layer")? {
        Some(id) => doc.layer_arg(id, "layer"),
        None => Ok(doc.document().active_layer_index()),
    }
}

fn changed_out(id: &str, layer: Option<u32>, changed: bool) -> ToolOutput {
    let mut out = json!({ "doc": id, "changed": changed });
    if let Some(l) = layer {
        out["layer"] = json!(l);
    }
    ToolOutput::json(out)
}

pub(crate) const LAYER_TRANSLATE: Tool = Tool {
    name: "layer.translate",
    description: "Image documents: move a layer's content by (dx, dy) like the editor's Move tool without a selection; pixels pushed off the canvas are gone and the uncovered area is transparent. A group moves its whole subtree.",
    schema: || offset_schema(true),
    run: |ctx, args| {
        let (dx, dy) = offset(args)?;
        let (id, doc) = ctx.ws.image_mut(args)?;
        let layer = image_layer(doc, args)?;
        // No zero-offset shortcut: the lock check runs first (AC-08), and
        // the dry run reports a zero offset as no change (AC-07).
        let changed = doc.run_if_changed(|| CommandSpec::TranslateLayer { layer, dx, dy })?;
        Ok(changed_out(&id, Some(doc.layer_id(layer)), changed))
    },
};

pub(crate) const SELECT_MOVE_CONTENT: Tool = Tool {
    name: "select.move_content",
    description: "Move the selected pixels of the target layer by (dx, dy) with the selection, like the editor's Move Selected Pixels; transparency stays behind and pixels moved off the canvas are dropped. Pixel documents need a selection; Image documents move the whole layer without one. One undo step that also restores the selection.",
    schema: || offset_schema(true),
    run: select_move_content,
};

fn select_move_content(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (dx, dy) = offset(args)?;
    if ctx.ws.is_image(args)? {
        let (id, doc) = ctx.ws.image_mut(args)?;
        let layer = image_layer(doc, args)?;
        doc.check_paintable(layer)?;
        let changed = (dx, dy) != (0, 0)
            && doc.run_if_changed(|| CommandSpec::TranslateSelection { layer, dx, dy })?;
        return Ok(changed_out(&id, Some(doc.layer_id(layer)), changed));
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let (layer, frame) = doc.paint_target(args)?;
    if !doc.sprite.has_selection() {
        return Err(AgentError::tool(format!(
            "select.move_content needs a selection in Pixel documents, and document `{id}` has none; select an area with select.* first"
        )));
    }
    if (dx, dy) == (0, 0) {
        return Ok(changed_out(&id, Some(layer.0), false));
    }
    // The editor paints into an empty cel when the layer has none here.
    let empty = doc
        .cels
        .get(layer, frame)
        .is_none()
        .then(|| doc.cel_or_empty(layer, frame));
    let changed = doc.commit_if_changed(|| {
        let mut cmds: Vec<pincel_core::AnyCommand> = empty
            .iter()
            .map(|c| PutCel::new(c.clone()).into())
            .collect();
        cmds.push(MoveSelectionContent::new(layer, frame, dx, dy).into());
        cmds
    })?;
    Ok(changed_out(&id, Some(layer.0), changed))
}

pub(crate) const SELECT_MOVE: Tool = Tool {
    name: "select.move",
    description: "Image documents: move only the selection outline by (dx, dy), like the editor's Move Selection; no pixel changes. Without a selection nothing changes.",
    schema: || offset_schema(false),
    run: |ctx, args| {
        let (dx, dy) = offset(args)?;
        let (id, doc) = ctx.ws.image_mut(args)?;
        let changed = (dx, dy) != (0, 0)
            && doc.run_if_changed(|| CommandSpec::TranslateSelectionMask { dx, dy })?;
        let mut out = changed_out(&id, None, changed);
        out.json["selection"] = doc.info(&id)["selection"].clone();
        Ok(out)
    },
};

pub(crate) const LAYER_IMPORT: Tool = Tool {
    name: "layer.import",
    description: "Paste an image file from the workspace as a new layer directly above the active layer, named \"Layer N\", its top-left at (x, y) (default 0, 0), like the editor's Paste; the layer becomes active and its id is returned. Pixel documents take PNG (on the target `frame`); Image documents PNG, JPEG, WebP, BMP, GIF or TIFF. `expand: true` first grows the canvas right and down to cover the image; otherwise the parts outside the canvas are cut off. With export.selection this is the headless Copy / Paste.",
    schema: || {
        tool_schema(
            json!({
                "path": path_prop("Workspace-relative image file"),
                "x": { "type": "integer", "description": "Canvas x of the image's left edge; default 0" },
                "y": { "type": "integer", "description": "Canvas y of the image's top edge; default 0" },
                "expand": { "type": "boolean", "description": "Grow the canvas (anchored top-left) to cover the image; default false" },
                "frame": frame_prop(),
            }),
            &["path"],
        )
    },
    run: layer_import,
};

/// The canvas size a Paste with `expand` needs for a `w`×`h` image at
/// `(x, y)`: it grows only to the right and down.
fn expanded(cw: u32, ch: u32, x: i32, y: i32, w: u32, h: u32) -> (i64, i64) {
    (
        (i64::from(x) + i64::from(w)).max(i64::from(cw)),
        (i64::from(y) + i64::from(h)).max(i64::from(ch)),
    )
}

fn layer_import(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let path = args.req_str("path")?;
    let x = args
        .get("x")
        .map(|_| args.req_i32("x"))
        .transpose()?
        .unwrap_or(0);
    let y = args
        .get("y")
        .map(|_| args.req_i32("y"))
        .transpose()?
        .unwrap_or(0);
    let expand = args.bool("expand").unwrap_or(false);
    let image = ctx.ws.is_image(args)?;
    if !image {
        let (_, doc) = ctx.ws.doc(args)?;
        if doc.sprite.color_mode != ColorMode::Rgba {
            return Err(AgentError::tool(
                "cannot paste into an indexed Pixel sprite (writing indexed pixels is not supported yet)",
            ));
        }
        doc.frame_arg(args)?;
    }
    let bytes = ctx
        .host
        .read(path)
        .map_err(|e| AgentError::tool(format!("cannot read `{path}`: {e}")))?;
    if image {
        crate::image::doc::check_frame(args)?;
        let buffer = decode(&bytes).map_err(|e| {
            AgentError::tool(format!(
                "cannot paste `{path}`: {e} (Image documents paste PNG, JPEG, WebP, BMP, GIF or TIFF)"
            ))
        })?;
        // An image doc.open refuses is refused here as well.
        check_size(buffer.width(), buffer.height())
            .map_err(|e| AgentError::tool(format!("cannot paste `{path}`: {e}")))?;
        let (id, doc) = ctx.ws.image_mut(args)?;
        if expand {
            let (w, h) = expanded(
                doc.width(),
                doc.height(),
                x,
                y,
                buffer.width(),
                buffer.height(),
            );
            check_size(
                u32::try_from(w).unwrap_or(u32::MAX),
                u32::try_from(h).unwrap_or(u32::MAX),
            )?;
        }
        doc.apply(vec![Box::new(ImagePaste::at(buffer, x, y, expand))])?;
        let layer = doc.document().active_layer_index();
        return Ok(ToolOutput::json(json!({
            "doc": id,
            "layer": doc.layer_id(layer),
            "width": doc.width(),
            "height": doc.height(),
        })));
    }
    let img = import_png(&bytes).map_err(|e| {
        AgentError::tool(format!(
            "cannot paste `{path}`: {e}; Pixel documents paste PNG files only"
        ))
    })?;
    let (id, doc) = ctx.ws.doc_mut(args)?;
    if expand {
        let (w, h) = expanded(
            doc.sprite.width,
            doc.sprite.height,
            x,
            y,
            img.width,
            img.height,
        );
        if w > i64::from(PIXEL_MAX) || h > i64::from(PIXEL_MAX) {
            return Err(AgentError::tool(format!(
                "pasting with expand needs a {w}×{h} canvas, above the Pixel limit of {PIXEL_MAX} px per side"
            )));
        }
    }
    let frame = doc.frame_arg(args)?.unwrap_or(doc.active_frame);
    let new_id = match doc.sprite.layers.iter().map(|l| l.id.0).max() {
        None => 0,
        Some(u32::MAX) => return Err(AgentError::tool("layer id space exhausted")),
        Some(m) => m + 1,
    };
    let name = format!("Layer {}", doc.sprite.layers.len() + 1);
    let above = copy_layer(doc);
    let image = PastedImage {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
    };
    let layer = Layer::image(LayerId::new(new_id), name);
    doc.commit(vec![
        PasteLayer::new(layer, frame, image)
            .at(x, y)
            .above(above)
            .expanding(expand)
            .into(),
    ])?;
    doc.active_layer = Some(LayerId::new(new_id));
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": new_id,
        "width": doc.sprite.width,
        "height": doc.sprite.height,
    })))
}

/// The layer the Pixel editor's Copy reads (and Paste puts the new layer
/// above): the selected layer whatever its kind, else the paint target.
fn copy_layer(doc: &Doc) -> Option<LayerId> {
    match doc.active_layer {
        Some(id) if doc.sprite.layer(id).is_some() => Some(id),
        _ => doc.sprite.paint_target_layer(doc.active_layer),
    }
}

pub(crate) const EXPORT_SELECTION: Tool = Tool {
    name: "export.selection",
    description: "The headless Copy: write the target layer's pixels inside the selection (the whole canvas without one) to a workspace PNG, whatever the path's extension, as the editor's Copy puts them on the clipboard; pixels outside the selection's shape are transparent (Image: alpha scaled by coverage). `merged: true` copies the composite of the visible layers (Copy Merged); `cut: true` then clears the copied pixels in one undo step (Cut). Reports the area {x, y, w, h}; layer.import at that x, y pastes it back.",
    schema: || {
        tool_schema(
            json!({
                "path": path_prop("Workspace-relative output PNG"),
                "merged": { "type": "boolean", "description": "Copy the composite of the visible layers; default false" },
                "cut": { "type": "boolean", "description": "Clear the copied pixels afterwards (one undo step); default false" },
                "layer": layer_prop(),
                "frame": frame_prop(),
            }),
            &["path"],
        )
    },
    run: export_selection,
};

/// What a Copy produced: the canvas area and its PNG.
struct Copied {
    area: Rect,
    png: Vec<u8>,
}

fn export_selection(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let path = args.req_str("path")?.to_owned();
    let merged = args.bool("merged").unwrap_or(false);
    let cut = args.bool("cut").unwrap_or(false);
    if cut && merged {
        return Err(AgentError::tool(
            "arguments `cut` and `merged` exclude each other: no editor cuts from the composite",
        ));
    }
    if merged && args.get("layer").is_some() {
        return Err(AgentError::tool(
            "arguments `merged` and `layer` exclude each other: the composite has no layer",
        ));
    }
    let (copied, cut_target) = if ctx.ws.is_image(args)? {
        image_copy(ctx, args, merged, cut)?
    } else {
        pixel_copy(ctx, args, merged, cut)?
    };
    // A cut the core refuses fails before anything is written (AC-21).
    if let Some(CutTarget::Pixel(layer, frame)) = cut_target {
        let (_, doc) = ctx.ws.doc(args)?;
        let (mut sprite, mut cels) = (doc.sprite.clone(), doc.cels.clone());
        let cmd = pixel_cut(layer, frame, copied.area, doc.sprite.has_selection());
        Bus::new()
            .execute(cmd, &mut sprite, &mut cels)
            .map_err(|e| AgentError::tool(e.to_string()))?;
    }
    // The file first: a refused write leaves the document unchanged (AC-19).
    ctx.host
        .check_write(&path)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    ctx.host
        .write(&path, &copied.png)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    let changed = match cut_target {
        None => false,
        Some(CutTarget::Pixel(layer, frame)) => {
            let (_, doc) = ctx.ws.doc_mut(args)?;
            let masked = doc.sprite.has_selection();
            doc.commit_if_changed(|| vec![pixel_cut(layer, frame, copied.area, masked)])?
        }
        Some(CutTarget::Image(layer)) => {
            let (_, doc) = ctx.ws.image_mut(args)?;
            doc.run_if_changed(|| CommandSpec::DeleteSelection { layer })?
        }
    };
    let id = ctx.ws.resolve(args)?;
    let a = copied.area;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "path": path,
        "area": { "x": a.x, "y": a.y, "w": a.width, "h": a.height },
        "cut": cut,
        "changed": changed,
    })))
}

/// The Pixel editor's Cut of `rect`: `ClearRegion`, masked by the
/// selection's shape when there is one.
fn pixel_cut(
    layer: LayerId,
    frame: pincel_core::FrameIndex,
    rect: Rect,
    masked: bool,
) -> AnyCommand {
    let cmd = ClearRegion::new(layer, frame, rect);
    if masked { cmd.masked() } else { cmd }.into()
}

/// The layer (and frame) a `cut` clears once the file is written.
enum CutTarget {
    Pixel(LayerId, pincel_core::FrameIndex),
    Image(usize),
}

fn pixel_copy(
    ctx: &mut Ctx<'_>,
    args: &Args,
    merged: bool,
    cut: bool,
) -> Result<(Copied, Option<CutTarget>), AgentError> {
    let (_, doc) = ctx.ws.doc(args)?;
    let frame = doc.frame_arg(args)?.unwrap_or(doc.active_frame);
    let source = if merged {
        CopySource::Merged
    } else {
        let layer = match args.u32("layer")? {
            Some(_) => layer_id_arg(doc, args, "layer")?,
            None => {
                copy_layer(doc).ok_or_else(|| AgentError::tool("document has no layer to copy"))?
            }
        };
        CopySource::Layer(layer)
    };
    let target = match (cut, source) {
        (true, CopySource::Layer(layer)) => {
            // Cut needs an unlocked image layer (paint_target's checks).
            let mut map = serde_json::Map::new();
            map.insert("layer".into(), json!(layer.0));
            map.insert("frame".into(), json!(frame.0));
            let (layer, frame) = doc.paint_target(&Args::new(map))?;
            Some(CutTarget::Pixel(layer, frame))
        }
        _ => None,
    };
    let c = copy_pixels(&doc.sprite, &doc.cels, frame, source)
        .map_err(|e| AgentError::tool(format!("cannot copy: {e}")))?;
    let png = encode_png(&c.rgba, c.rect.width, c.rect.height)
        .map_err(|e| AgentError::tool(format!("cannot encode PNG: {e}")))?;
    Ok((Copied { area: c.rect, png }, target))
}

fn image_copy(
    ctx: &mut Ctx<'_>,
    args: &Args,
    merged: bool,
    cut: bool,
) -> Result<(Copied, Option<CutTarget>), AgentError> {
    let (_, doc) = ctx.ws.image(args)?;
    let layer = image_layer(doc, args)?;
    if cut {
        doc.check_paintable(layer)?;
    }
    let d = doc.document();
    if !merged && d.layers()[layer].is_group() {
        // The editor's Copy has nothing to copy from a group either.
        return Err(AgentError::tool(format!(
            "layer {} ({:?}) is a group, which has no pixels of its own to copy in Image documents; pass `merged: true` for the composite or the id of an image layer inside it",
            doc.layer_id(layer),
            d.layers()[layer].name
        )));
    }
    // The editor's Copy reads the active layer; copy another one from a
    // copy of the document with that layer active.
    let other;
    let source = if merged || layer == d.active_layer_index() {
        d
    } else {
        let mut copy = d.clone();
        copy.set_active_layer(layer)
            .map_err(crate::image::core_err)?;
        other = copy;
        &other
    };
    let (region, pixels) = copy_selection(source, merged).ok_or_else(|| {
        AgentError::tool("nothing is selected: the selection has no selected pixel")
    })?;
    let png = to_png_bytes(&pixels, 1)
        .map_err(|e| AgentError::tool(format!("cannot encode PNG: {e}")))?;
    let area = Rect::new(region.x, region.y, region.w, region.h);
    Ok((Copied { area, png }, cut.then_some(CutTarget::Image(layer))))
}
