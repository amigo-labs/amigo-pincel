//! `draw.*`, `selection.*` and `effect.apply`: pixel edits on one cel.
//!
//! Every edit runs the editor's own `pincel-core` command on a scratch copy
//! of the target cel and commits only the changed rectangle as one
//! [`PatchCel`] (dirty-rect deltas, pincel.md §6.3), so a call is one exact
//! undo step, memory stays bounded and a failure changes nothing.

use pincel_core::{
    AddEmptyCel, AnyCommand, Cel, CelData, CelMap, ClearRegion, Command, DrawEllipse, DrawLine,
    DrawRectangle, FillRegion, PatchCel, Rect, Rgba, SetPixel, Sprite,
};
use pincel_effects::{EffectImage, EffectSpec};
use serde_json::{Map, Value, json};

use super::effects::{catalog_summary, effect_ids, wire_params};
use super::{int, target_schema};
use crate::args::{Args, color_schema, int_i32, parse_color, parse_region, region_schema};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

/// Run `edit` on a scratch copy of the target cel and commit the result as
/// one undo step (nothing when the cel did not change).
fn paint(
    ctx: &mut Ctx<'_>,
    args: &Args,
    edit: impl FnOnce(&Doc, &mut Cel) -> Result<(), AgentError>,
) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let (layer, frame) = doc.paint_target(args)?;
    let exists = doc.cels.get(layer, frame).is_some();
    let original = doc.cel_or_empty(layer, frame);
    let mut cel = original.clone();
    edit(doc, &mut cel)?;
    let patch = diff_patch(&original, &cel)?;
    let changed = patch.is_some();
    if let Some(patch) = patch {
        let mut cmds: Vec<AnyCommand> = Vec::with_capacity(2);
        if !exists {
            cmds.push(AddEmptyCel::new(layer, frame, patch.cel_size, original.position).into());
        }
        cmds.push(PatchCel::new(layer, frame, patch.origin, patch.size, patch.pixels).into());
        doc.commit(cmds)?;
    }
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": layer.0,
        "frame": frame.0,
        "changed": changed,
    })))
}

/// The changed rectangle between two same-sized image cels.
struct Patch {
    /// Cel-local top-left of the rectangle.
    origin: (u32, u32),
    /// Width and height of the rectangle.
    size: (u32, u32),
    /// The edited pixels inside it (RGBA8, row-major).
    pixels: Vec<u8>,
    /// Size of the whole cel buffer.
    cel_size: (u32, u32),
}

/// The bounding box of every pixel that differs between `before` and
/// `after`, with `after`'s pixels inside it; `None` when nothing changed.
fn diff_patch(before: &Cel, after: &Cel) -> Result<Option<Patch>, AgentError> {
    let (CelData::Image(a), CelData::Image(b)) = (&before.data, &after.data) else {
        return Err(AgentError::tool("pixel tools need an image cel"));
    };
    if (a.width, a.height) != (b.width, b.height) || a.data.len() != b.data.len() {
        return Err(AgentError::tool("the edit changed the cel size"));
    }
    let w = a.width as usize;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for (i, (pa, pb)) in a
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.data.as_chunks::<4>().0)
        .enumerate()
    {
        if pa != pb {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 == usize::MAX {
        return Ok(None);
    }
    let (pw, ph) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut pixels = Vec::with_capacity(pw * ph * 4);
    for y in y0..=y1 {
        let start = (y * w + x0) * 4;
        pixels.extend_from_slice(&b.data[start..start + pw * 4]);
    }
    Ok(Some(Patch {
        origin: (x0 as u32, y0 as u32),
        size: (pw as u32, ph as u32),
        pixels,
        cel_size: (a.width, a.height),
    }))
}

/// Apply an editor command to `cel` alone.
fn run_core<C: Command>(sprite: &Sprite, cel: &mut Cel, mut cmd: C) -> Result<(), AgentError> {
    let mut scratch = sprite.clone();
    let mut cels = CelMap::new();
    let key = (cel.layer, cel.frame);
    cels.insert(cel.clone());
    cmd.apply(&mut scratch, &mut cels)
        .map_err(|e| AgentError::tool(e.to_string()))?;
    *cel = cels
        .remove(key.0, key.1)
        .ok_or_else(|| AgentError::tool("the target cel vanished during the edit"))?;
    Ok(())
}

/// Keep `original` outside the active selection (AC-25).
fn mask_to_selection(sprite: &Sprite, original: &Cel, edited: &mut Cel) {
    if !sprite.has_selection() {
        return;
    }
    let (CelData::Image(src), CelData::Image(dst)) = (&original.data, &mut edited.data) else {
        return;
    };
    let (px, py) = original.position;
    for y in 0..src.height {
        for x in 0..src.width {
            if !sprite.selection_contains(px + x as i32, py + y as i32) {
                let i = (y as usize * src.width as usize + x as usize) * 4;
                dst.data[i..i + 4].copy_from_slice(&src.data[i..i + 4]);
            }
        }
    }
}

fn color_arg(doc: &Doc, args: &Args) -> Result<Rgba, AgentError> {
    let v = args
        .get("color")
        .ok_or_else(|| AgentError::tool("missing required argument `color`"))?;
    parse_color(v, "color", &doc.sprite.palette)
}

fn check_in_canvas(doc: &Doc, x: i32, y: i32, what: &str) -> Result<(), AgentError> {
    let (w, h) = (doc.sprite.width, doc.sprite.height);
    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
        return Err(AgentError::tool(format!(
            "{what} ({x}, {y}) outside {w}×{h} canvas"
        )));
    }
    Ok(())
}

fn endpoints() -> Value {
    json!({
        "x0": int("Start x"),
        "y0": int("Start y"),
        "x1": int("End x"),
        "y1": int("End y"),
        "color": color_schema("Color"),
    })
}

fn shape_schema(filled: bool) -> Value {
    let mut props = endpoints();
    if filled {
        props["filled"] = json!({ "type": "boolean", "description": "Fill the interior; default false (outline)" });
    }
    target_schema(props, &["x0", "y0", "x1", "y1", "color"])
}

type Corners = ((i32, i32), (i32, i32));

fn corners(args: &Args) -> Result<Corners, AgentError> {
    Ok((
        (args.req_i32("x0")?, args.req_i32("y0")?),
        (args.req_i32("x1")?, args.req_i32("y1")?),
    ))
}

pub(super) const PIXELS: Tool = Tool {
    name: "draw.pixels",
    description: "Set individual pixels: a batch of {x, y, color}. The batch is atomic: a pixel outside the canvas or a bad color rejects the whole call. Colors: \"#RRGGBB\", \"#RRGGBBAA\" or {\"index\": n} (palette index).",
    schema: || {
        target_schema(
            json!({
                "pixels": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "x": { "type": "integer" },
                            "y": { "type": "integer" },
                            "color": color_schema("Pixel color"),
                        },
                        "required": ["x", "y", "color"],
                        "additionalProperties": false,
                    },
                },
            }),
            &["pixels"],
        )
    },
    run: |ctx, args| {
        paint(ctx, args, |doc, cel| {
            let items = args
                .get("pixels")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let mut batch: Option<SetPixel> = None;
            for (i, p) in items.iter().enumerate() {
                let x = int_i32(p.get("x"), &format!("pixels[{i}].x"))?;
                let y = int_i32(p.get("y"), &format!("pixels[{i}].y"))?;
                check_in_canvas(doc, x, y, "pixel")?;
                let c = p.get("color").unwrap_or(&Value::Null);
                let color = parse_color(c, &format!("pixels[{i}].color"), &doc.sprite.palette)?;
                let next = SetPixel::new(cel.layer, cel.frame, x, y, color);
                match &mut batch {
                    None => batch = Some(next),
                    Some(b) => {
                        b.merge(&next);
                    }
                }
            }
            match batch {
                Some(cmd) => run_core(&doc.sprite, cel, cmd),
                None => Ok(()),
            }
        })
    },
};

pub(super) const LINE: Tool = Tool {
    name: "draw.line",
    description: "Draw a 1-px line from (x0, y0) to (x1, y1), like the editor's Line tool. Parts outside the canvas are clipped.",
    schema: || shape_schema(false),
    run: |ctx, args| {
        paint(ctx, args, |doc, cel| {
            let ((x0, y0), (x1, y1)) = corners(args)?;
            let color = color_arg(doc, args)?;
            run_core(
                &doc.sprite,
                cel,
                DrawLine::new(cel.layer, cel.frame, x0, y0, x1, y1, color),
            )
        })
    },
};

pub(super) const RECT: Tool = Tool {
    name: "draw.rect",
    description: "Draw a rectangle with corners (x0, y0) and (x1, y1) (inclusive), outline or `filled`, like the editor's Rectangle tool. Clipped to the canvas.",
    schema: || shape_schema(true),
    run: |ctx, args| {
        paint(ctx, args, |doc, cel| {
            let (a, b) = corners(args)?;
            let color = color_arg(doc, args)?;
            let filled = args.bool("filled").unwrap_or(false);
            run_core(
                &doc.sprite,
                cel,
                DrawRectangle::new(cel.layer, cel.frame, a, b, filled, color),
            )
        })
    },
};

pub(super) const ELLIPSE: Tool = Tool {
    name: "draw.ellipse",
    description: "Draw the ellipse inscribed in the box with corners (x0, y0) and (x1, y1), outline or `filled`, like the editor's Ellipse tool. Clipped to the canvas.",
    schema: || shape_schema(true),
    run: |ctx, args| {
        paint(ctx, args, |doc, cel| {
            let (a, b) = corners(args)?;
            let color = color_arg(doc, args)?;
            let filled = args.bool("filled").unwrap_or(false);
            run_core(
                &doc.sprite,
                cel,
                DrawEllipse::new(cel.layer, cel.frame, a, b, filled, color),
            )
        })
    },
};

pub(super) const FILL: Tool = Tool {
    name: "draw.fill",
    description: "Bucket fill from the seed (x, y) like the editor's Bucket tool: the 4-connected area of exactly the seed's color. `contiguous: false` recolors every pixel of the seed's color. Respects the selection.",
    schema: || {
        target_schema(
            json!({
                "x": int("Seed x"),
                "y": int("Seed y"),
                "color": color_schema("Fill color"),
                "contiguous": { "type": "boolean", "description": "Only the connected area; default true" },
            }),
            &["x", "y", "color"],
        )
    },
    run: |ctx, args| {
        paint(ctx, args, |doc, cel| {
            let (x, y) = (args.req_i32("x")?, args.req_i32("y")?);
            check_in_canvas(doc, x, y, "seed")?;
            let color = color_arg(doc, args)?;
            let original = cel.clone();
            if args.bool("contiguous").unwrap_or(true) {
                run_core(
                    &doc.sprite,
                    cel,
                    FillRegion::new(cel.layer, cel.frame, x, y, color),
                )?;
            } else {
                replace_color(cel, x, y, color);
            }
            mask_to_selection(&doc.sprite, &original, cel);
            Ok(())
        })
    },
};

/// Recolor every pixel that has the seed's color (global fill).
fn replace_color(cel: &mut Cel, x: i32, y: i32, color: Rgba) {
    let (px, py) = cel.position;
    let CelData::Image(buf) = &mut cel.data else {
        return;
    };
    let (lx, ly) = (i64::from(x) - i64::from(px), i64::from(y) - i64::from(py));
    if lx < 0 || ly < 0 || lx >= i64::from(buf.width) || ly >= i64::from(buf.height) {
        return;
    }
    let i = (ly as usize * buf.width as usize + lx as usize) * 4;
    let seed = [
        buf.data[i],
        buf.data[i + 1],
        buf.data[i + 2],
        buf.data[i + 3],
    ];
    let new = [color.r, color.g, color.b, color.a];
    for px in buf.data.as_chunks_mut::<4>().0 {
        if *px == seed {
            *px = new;
        }
    }
}

pub(super) const CLEAR: Tool = Tool {
    name: "draw.clear",
    description: "Make the pixels inside `region` transparent; without `region`, the whole cel.",
    schema: || {
        target_schema(
            json!({ "region": region_schema("Rectangle to clear") }),
            &[],
        )
    },
    run: |ctx, args| {
        paint(ctx, args, |doc, cel| match args.get("region") {
            Some(v) => {
                let r = parse_region(v)?;
                run_core(&doc.sprite, cel, ClearRegion::new(cel.layer, cel.frame, r))
            }
            None => {
                if let CelData::Image(buf) = &mut cel.data {
                    buf.data.fill(0);
                }
                Ok(())
            }
        })
    },
};

pub(super) const SELECTION_SET_RECT: Tool = Tool {
    name: "selection.set_rect",
    description: "Select a rectangle. While a selection exists, draw.fill and effect.apply change only pixels inside it. The selection belongs to the document (all layers and frames) and is not an undo step.",
    schema: || {
        target_schema(
            json!({
                "x": int("Left"),
                "y": int("Top"),
                "width": { "type": "integer", "minimum": 1 },
                "height": { "type": "integer", "minimum": 1 },
            }),
            &["x", "y", "width", "height"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        doc.frame_arg(args)?;
        let r = Rect::new(
            args.req_i32("x")?,
            args.req_i32("y")?,
            args.req_u32("width")?,
            args.req_u32("height")?,
        );
        doc.sprite.set_selection(r);
        Ok(ToolOutput::json(json!({
            "doc": id,
            "selection": { "x": r.x, "y": r.y, "width": r.width, "height": r.height },
        })))
    },
};

pub(super) const SELECTION_CLEAR: Tool = Tool {
    name: "selection.clear",
    description: "Drop the selection; draw.fill and effect.apply act on the whole cel again.",
    schema: || target_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        doc.sprite.clear_selection();
        Ok(ToolOutput::json(json!({ "doc": id, "selection": null })))
    },
};

pub(super) const EFFECT_APPLY: Tool = Tool {
    name: "effect.apply",
    description: "Apply an effect or adjustment from the pincel-effects catalog to the target cel, exactly as the editor's Effects / Adjust menus do. Respects the selection. `params` maps parameter names to values; omitted ones use their defaults.",
    schema: || {
        target_schema(
            json!({
                "id": { "type": "string", "enum": effect_ids(), "description": "Effect id" },
                "params": {
                    "type": "object",
                    "description": format!("Named parameters. Catalog: {}", catalog_summary()),
                },
            }),
            &["id"],
        )
    },
    run: |ctx, args| {
        let effect = args.req_str("id")?;
        let empty = Map::new();
        let params = args
            .get("params")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let wire = wire_params(effect, params)?;
        let spec =
            EffectSpec::from_params(effect, &wire).map_err(|e| AgentError::tool(e.to_string()))?;
        paint(ctx, args, |doc, cel| {
            let original = cel.clone();
            let CelData::Image(buf) = &mut cel.data else {
                return Err(AgentError::tool("effects need an image cel"));
            };
            let src = EffectImage::from_rgba8(buf.width, buf.height, buf.data.clone())
                .map_err(|e| AgentError::tool(e.to_string()))?;
            buf.data = spec.run(&src).into_raw();
            mask_to_selection(&doc.sprite, &original, cel);
            Ok(())
        })
    },
};
