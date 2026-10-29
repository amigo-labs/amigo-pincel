//! `layer.*`, `frame.*`, `tag.*` and `palette.*`: the document structure.

use pincel_core::{
    AddFrame, AddLayer, AddTag, AnyCommand, BlendMode, Cel, Frame, FrameIndex, Layer, LayerId,
    LayerKind, LayerPlacement, MoveLayerTo, Palette, PaletteEntry, PalettePreset, PutCel,
    RemoveFrame, RemoveLayer, RemoveTag, ReplacePalette, SetFrameDuration, SetLayerBlendMode,
    SetLayerEditable, SetLayerName, SetLayerOpacity, SetLayerVisible, Sprite, Tag, TagDirection,
};
use serde_json::{Value, json};

use super::{int, tool_schema};
use crate::args::{Args, hex, parse_hex};
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

const DIRECTIONS: [(&str, TagDirection); 4] = [
    ("forward", TagDirection::Forward),
    ("reverse", TagDirection::Reverse),
    ("pingpong", TagDirection::Pingpong),
    ("pingpong_reverse", TagDirection::PingpongReverse),
];

fn blend_names() -> Vec<&'static str> {
    BlendMode::ALL.iter().map(|m| m.name()).collect()
}

fn kind_name(kind: &LayerKind) -> &'static str {
    match kind {
        LayerKind::Image => "image",
        LayerKind::Group => "group",
        LayerKind::Tilemap { .. } => "tilemap",
    }
}

fn layers_json(sprite: &Sprite) -> Value {
    let layers: Vec<Value> = sprite
        .layers
        .iter()
        .enumerate()
        .map(|(i, l)| {
            json!({
                "id": l.id.0,
                "index": i,
                "name": l.name,
                "kind": kind_name(&l.kind),
                "parent": l.parent.map(|p| p.0),
                "visible": l.visible,
                "opacity": l.opacity,
                "blend": l.blend_mode.name(),
                "locked": !l.editable,
            })
        })
        .collect();
    json!(layers)
}

fn frames_json(sprite: &Sprite) -> Value {
    let frames: Vec<Value> = sprite
        .frames
        .iter()
        .enumerate()
        .map(|(i, f)| json!({ "index": i, "duration_ms": f.duration_ms }))
        .collect();
    json!(frames)
}

fn tags_json(sprite: &Sprite) -> Value {
    let tags: Vec<Value> = sprite
        .tags
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let dir = DIRECTIONS
                .iter()
                .find(|(_, d)| *d == t.direction)
                .map_or("forward", |(n, _)| *n);
            json!({ "index": i, "name": t.name, "from": t.from.0, "to": t.to.0, "direction": dir })
        })
        .collect();
    json!(tags)
}

fn palette_json(palette: &Palette) -> Value {
    let entries: Vec<String> = palette.colors.iter().map(|e| hex(e.rgba)).collect();
    json!({ "size": entries.len(), "entries": entries })
}

/// The `layer` argument as an existing layer id.
fn layer_arg(doc: &Doc, args: &Args, key: &str) -> Result<LayerId, AgentError> {
    let id = LayerId::new(args.req_u32(key)?);
    doc.sprite.layer(id).map(|_| id).ok_or_else(|| {
        AgentError::tool(format!(
            "no layer with id {}; layer.list shows the ids",
            id.0
        ))
    })
}

/// The `parent` argument as an existing group id.
fn group_arg(doc: &Doc, args: &Args) -> Result<Option<LayerId>, AgentError> {
    let Some(id) = args.u32("parent")? else {
        return Ok(None);
    };
    let id = LayerId::new(id);
    match doc.sprite.layer(id).map(|l| &l.kind) {
        Some(LayerKind::Group) => Ok(Some(id)),
        Some(_) => Err(AgentError::tool(format!(
            "parent {} is not a group layer",
            id.0
        ))),
        None => Err(AgentError::tool(format!("no layer with id {}", id.0))),
    }
}

fn layer_props() -> Value {
    json!({
        "name": { "type": "string", "description": "Layer name" },
        "visible": { "type": "boolean" },
        "opacity": { "type": "integer", "minimum": 0, "maximum": 255 },
        "blend": { "type": "string", "enum": blend_names(), "description": "Blend mode" },
        "locked": { "type": "boolean", "description": "Locked layers refuse pixel edits" },
        "parent": { "type": "integer", "minimum": 0, "description": "Id of a group layer to move the layer into (as its top child)" },
    })
}

pub(super) const LAYER_LIST: Tool = Tool {
    name: "layer.list",
    description: "List the layers bottom to top with id, name, kind (image / group / tilemap), parent group, visibility, opacity, blend mode and lock state.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "layers": layers_json(&doc.sprite) }),
        ))
    },
};

pub(super) const LAYER_ADD: Tool = Tool {
    name: "layer.add",
    description: "Add a layer on top of the stack (or as the top child of the group `parent`) and make it the active layer. Returns its id.",
    schema: || {
        let mut props = layer_props();
        props["kind"] = json!({ "type": "string", "enum": ["image", "group"], "description": "Layer kind; default image" });
        tool_schema(props, &[])
    },
    run: layer_add,
};

fn layer_add(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let new_id = match doc.sprite.layers.iter().map(|l| l.id.0).max() {
        None => 0,
        Some(u32::MAX) => return Err(AgentError::tool("layer id space exhausted")),
        Some(m) => m + 1,
    };
    let lid = LayerId::new(new_id);
    let name = args.str("name").map_or_else(
        || format!("Layer {}", doc.sprite.layers.len() + 1),
        str::to_owned,
    );
    let mut layer = if args.str("kind") == Some("group") {
        Layer::group(lid, name)
    } else {
        Layer::image(lid, name)
    };
    if let Some(v) = args.bool("visible") {
        layer.visible = v;
    }
    if let Some(o) = args.i64("opacity") {
        layer.opacity = u8::try_from(o).map_err(|_| AgentError::tool("opacity must be 0–255"))?;
    }
    if let Some(b) = args.str("blend") {
        layer.blend_mode = BlendMode::from_name(b)
            .ok_or_else(|| AgentError::tool(format!("unknown blend mode `{b}`")))?;
    }
    if let Some(locked) = args.bool("locked") {
        layer.editable = !locked;
    }
    let parent = group_arg(doc, args)?;
    let mut cmds: Vec<AnyCommand> = vec![AddLayer::on_top(layer).into()];
    if let Some(p) = parent {
        cmds.push(MoveLayerTo::new(lid, p, LayerPlacement::Into).into());
    }
    doc.commit(cmds)?;
    doc.active_layer = Some(lid);
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": new_id,
        "layers": layers_json(&doc.sprite),
    })))
}

pub(super) const LAYER_SET: Tool = Tool {
    name: "layer.set",
    description: "Change a layer's name, visibility, opacity (0–255), blend mode, lock state, or move it into the group `parent`. One undo step.",
    schema: || {
        let mut props = layer_props();
        props["layer"] =
            json!({ "type": "integer", "minimum": 0, "description": "Layer id from layer.list" });
        tool_schema(props, &["layer"])
    },
    run: layer_set,
};

fn layer_set(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let lid = layer_arg(doc, args, "layer")?;
    let mut cmds: Vec<AnyCommand> = Vec::new();
    if let Some(n) = args.str("name") {
        cmds.push(SetLayerName::new(lid, n).into());
    }
    if let Some(v) = args.bool("visible") {
        cmds.push(SetLayerVisible::new(lid, v).into());
    }
    if let Some(o) = args.i64("opacity") {
        let o = u8::try_from(o).map_err(|_| AgentError::tool("opacity must be 0–255"))?;
        cmds.push(SetLayerOpacity::new(lid, o).into());
    }
    if let Some(b) = args.str("blend") {
        let mode = BlendMode::from_name(b)
            .ok_or_else(|| AgentError::tool(format!("unknown blend mode `{b}`")))?;
        cmds.push(SetLayerBlendMode::new(lid, mode).into());
    }
    if let Some(locked) = args.bool("locked") {
        cmds.push(SetLayerEditable::new(lid, !locked).into());
    }
    if let Some(p) = group_arg(doc, args)? {
        cmds.push(MoveLayerTo::new(lid, p, LayerPlacement::Into).into());
    }
    doc.commit(cmds)?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(&doc.sprite) }),
    ))
}

pub(super) const LAYER_REMOVE: Tool = Tool {
    name: "layer.remove",
    description: "Remove a layer (a group with all its children) and its cels.",
    schema: || {
        tool_schema(
            json!({ "layer": { "type": "integer", "minimum": 0, "description": "Layer id from layer.list" } }),
            &["layer"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let lid = layer_arg(doc, args, "layer")?;
        doc.check_unlocked(lid)?;
        doc.commit(vec![RemoveLayer::new(lid).into()])?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "layers": layers_json(&doc.sprite) }),
        ))
    },
};

pub(super) const FRAME_LIST: Tool = Tool {
    name: "frame.list",
    description: "List the frames with their index and duration in milliseconds.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "frames": frames_json(&doc.sprite) }),
        ))
    },
};

fn duration_prop() -> Value {
    json!({ "type": "integer", "minimum": 1, "maximum": 65535, "description": "Frame duration in milliseconds" })
}

pub(super) const FRAME_ADD: Tool = Tool {
    name: "frame.add",
    description: "Append a frame and make it the active frame. With `clone_from`, the new frame gets copies of that frame's cels (and its duration unless `duration_ms` is given); otherwise it starts empty with 100 ms.",
    schema: || {
        tool_schema(
            json!({
                "clone_from": { "type": "integer", "minimum": 0, "description": "Frame index whose cels to copy" },
                "duration_ms": duration_prop(),
            }),
            &[],
        )
    },
    run: frame_add,
};

fn frame_add(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let source = args
        .u32("clone_from")?
        .map(|f| doc.check_frame(f))
        .transpose()?;
    let duration = match args.u32("duration_ms")? {
        Some(d) => u16::try_from(d).map_err(|_| AgentError::tool("duration_ms must be 1–65535"))?,
        None => source.map_or(100, |s| doc.sprite.frames[s.0 as usize].duration_ms),
    };
    let new_index = FrameIndex::new(doc.sprite.frames.len() as u32);
    let mut cmds: Vec<AnyCommand> = vec![AddFrame::append(Frame::new(duration)).into()];
    if let Some(src) = source {
        let clones: Vec<Cel> = doc
            .cels
            .iter()
            .filter(|(_, c)| c.frame == src)
            .map(|(_, c)| Cel {
                frame: new_index,
                ..c.clone()
            })
            .collect();
        cmds.extend(clones.into_iter().map(|c| PutCel::new(c).into()));
    }
    doc.commit(cmds)?;
    doc.active_frame = new_index;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "frame": new_index.0,
        "frames": frames_json(&doc.sprite),
    })))
}

pub(super) const FRAME_REMOVE: Tool = Tool {
    name: "frame.remove",
    description: "Remove a frame and its cels; later frames move down by one. The last remaining frame cannot be removed.",
    schema: || {
        tool_schema(
            json!({ "frame": int("Frame index from frame.list") }),
            &["frame"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let frame = doc.check_frame(args.req_u32("frame")?)?;
        doc.commit(vec![RemoveFrame::new(frame).into()])?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "frames": frames_json(&doc.sprite) }),
        ))
    },
};

pub(super) const FRAME_SET_DURATION: Tool = Tool {
    name: "frame.set_duration",
    description: "Set how long a frame is shown, in milliseconds.",
    schema: || {
        tool_schema(
            json!({ "frame": int("Frame index from frame.list"), "duration_ms": duration_prop() }),
            &["frame", "duration_ms"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let frame = doc.check_frame(args.req_u32("frame")?)?;
        let d = u16::try_from(args.req_u32("duration_ms")?)
            .map_err(|_| AgentError::tool("duration_ms must be 1–65535"))?;
        doc.commit(vec![SetFrameDuration::new(frame, d).into()])?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "frames": frames_json(&doc.sprite) }),
        ))
    },
};

pub(super) const TAG_LIST: Tool = Tool {
    name: "tag.list",
    description: "List the animation tags with index, name, frame range and playback direction.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "tags": tags_json(&doc.sprite) }),
        ))
    },
};

pub(super) const TAG_ADD: Tool = Tool {
    name: "tag.add",
    description: "Add an animation tag over frames `from`..=`to` (indices) with a playback direction.",
    schema: || {
        let dirs: Vec<&str> = DIRECTIONS.iter().map(|(n, _)| *n).collect();
        tool_schema(
            json!({
                "name": { "type": "string", "description": "Tag name (e.g. walk)" },
                "from": { "type": "integer", "minimum": 0, "description": "First frame index" },
                "to": { "type": "integer", "minimum": 0, "description": "Last frame index (inclusive)" },
                "direction": { "type": "string", "enum": dirs, "description": "Playback direction; default forward" },
            }),
            &["name", "from", "to"],
        )
    },
    run: tag_add,
};

fn tag_add(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let from = doc.check_frame(args.req_u32("from")?)?;
    let to = doc.check_frame(args.req_u32("to")?)?;
    let mut tag = Tag::new(args.req_str("name")?, from, to);
    if let Some(d) = args.str("direction") {
        tag.direction = DIRECTIONS
            .iter()
            .find(|(n, _)| *n == d)
            .map(|(_, dir)| *dir)
            .ok_or_else(|| AgentError::tool(format!("unknown direction `{d}`")))?;
    }
    doc.commit(vec![AddTag::new(tag).into()])?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "tag": doc.sprite.tags.len() - 1,
        "tags": tags_json(&doc.sprite),
    })))
}

pub(super) const TAG_REMOVE: Tool = Tool {
    name: "tag.remove",
    description: "Remove an animation tag by its index from tag.list.",
    schema: || {
        tool_schema(
            json!({ "tag": { "type": "integer", "minimum": 0, "description": "Tag index from tag.list" } }),
            &["tag"],
        )
    },
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc_mut(args)?;
        let index = args.req_u32("tag")? as usize;
        if index >= doc.sprite.tags.len() {
            return Err(AgentError::tool(format!(
                "no tag with index {index} (document has {} tags)",
                doc.sprite.tags.len()
            )));
        }
        doc.commit(vec![RemoveTag::new(index).into()])?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "tags": tags_json(&doc.sprite) }),
        ))
    },
};

pub(super) const PALETTE_GET: Tool = Tool {
    name: "palette.get",
    description: "Return the palette as #RRGGBBAA entries; `{\"index\": n}` colors refer to these indices.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        let (id, doc) = ctx.ws.doc(args)?;
        let mut out = palette_json(&doc.sprite.palette);
        out["doc"] = json!(id);
        Ok(ToolOutput::json(out))
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
    let mut out = palette_json(&doc.sprite.palette);
    out["doc"] = json!(id);
    Ok(ToolOutput::json(out))
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
        let mut out = palette_json(&doc.sprite.palette);
        out["doc"] = json!(id);
        Ok(ToolOutput::json(out))
    },
};
