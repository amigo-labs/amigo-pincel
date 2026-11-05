//! `layer.*`, `frame.*` and `tag.*`: the document structure.

use pincel_core::{
    AddFrame, AddLayer, AddTag, AddTilemapLayer, AnyCommand, BlendMode, Cel, Frame, FrameIndex,
    Layer, LayerId, LayerKind, LayerPlacement, MoveLayerTo, PutCel, RemoveFrame, RemoveLayer,
    RemoveTag, Rgba, SetFrameDuration, SetLayerBlendMode, SetLayerEditable, SetLayerName,
    SetLayerOpacity, SetLayerVisible, SetTagDirection, SetTagName, SetTagRange, Sprite, Tag,
    TagDirection, TilesetId,
};
use serde_json::{Value, json};

use super::layer_ops::{check_placement, place_core};
use super::{int, tool_schema};
use crate::args::{Args, parse_hex, placement, placement_props};
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

fn tileset_of(kind: &LayerKind) -> Option<u32> {
    match kind {
        LayerKind::Tilemap { tileset_id } => Some(tileset_id.0),
        _ => None,
    }
}

fn kind_name(kind: &LayerKind) -> &'static str {
    match kind {
        LayerKind::Image => "image",
        LayerKind::Group => "group",
        LayerKind::Tilemap { .. } => "tilemap",
    }
}

pub(super) fn layers_json(sprite: &Sprite) -> Value {
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
                "tileset": tileset_of(&l.kind),
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

/// The timeline's colors for new tags, in order (TimelinePanel.svelte
/// `TAG_COLORS`); tag n gets entry n mod 6.
const TAG_COLORS: [u32; 6] = [0x5b8def, 0xe0a83c, 0x58b368, 0xd0607a, 0x9a6ad8, 0x3cb7c4];

/// A tag color as `#RRGGBB` (tag colors carry no alpha in the editor).
fn tag_hex(c: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
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
            json!({
                "index": i,
                "name": t.name,
                "from": t.from.0,
                "to": t.to.0,
                "direction": dir,
                "color": tag_hex(t.color),
            })
        })
        .collect();
    json!(tags)
}

/// The argument `key` as an existing layer id; the error names the
/// argument and its value (mcp-parity-structure AC-27).
pub(crate) fn layer_arg(doc: &Doc, args: &Args, key: &str) -> Result<LayerId, AgentError> {
    let id = LayerId::new(args.req_u32(key)?);
    doc.sprite.layer(id).map(|_| id).ok_or_else(|| {
        AgentError::tool(format!(
            "argument `{key}`: no layer with id {}; layer.list shows the ids",
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
            "argument `parent`: layer {} is not a group layer",
            id.0
        ))),
        None => Err(AgentError::tool(format!(
            "argument `parent`: no layer with id {}",
            id.0
        ))),
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
        if ctx.ws.is_image(args)? {
            return crate::image::layer::list(ctx, args);
        }
        let (id, doc) = ctx.ws.doc(args)?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "layers": layers_json(&doc.sprite) }),
        ))
    },
};

pub(super) const LAYER_ADD: Tool = Tool {
    name: "layer.add",
    description: "Add a layer and make it the active layer; returns its id. Without `target` it goes where the editor puts a new layer: Pixel documents on top of the stack (or as the top child of the group `parent`), Image documents directly above the active layer in its group (a group on top of the stack). `target` + `place` put it above / below the layer `target` or into the group `target`.",
    schema: || {
        let mut props = layer_props();
        if let (Some(p), Value::Object(extra)) = (props.as_object_mut(), placement_props()) {
            p.extend(extra);
        }
        props["kind"] = json!({ "type": "string", "enum": ["image", "group", "tilemap"], "description": "Layer kind; default image. tilemap (Pixel documents) needs `tileset`" });
        props["tileset"] = json!({ "type": "integer", "minimum": 0, "description": "kind tilemap: id of the tileset (tileset.list) the layer uses" });
        tool_schema(props, &[])
    },
    run: layer_add,
};

fn layer_add(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        return crate::image::layer::add(ctx, args);
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let new_id = match doc.sprite.layers.iter().map(|l| l.id.0).max() {
        None => 0,
        Some(u32::MAX) => return Err(AgentError::tool("layer id space exhausted")),
        Some(m) => m + 1,
    };
    let lid = LayerId::new(new_id);
    let tilemap = args.str("kind") == Some("tilemap");
    let tileset = match (tilemap, args.u32("tileset")?) {
        (true, None) => {
            return Err(AgentError::tool(
                "argument `tileset` is needed with `kind: \"tilemap\"` (tileset.list shows the ids)",
            ));
        }
        (false, Some(_)) => {
            return Err(AgentError::tool(
                "argument `tileset` needs `kind: \"tilemap\"`",
            ));
        }
        (true, Some(t)) => Some(doc.sprite.tileset(TilesetId::new(t)).ok_or_else(|| {
            AgentError::tool(format!(
                "argument `tileset`: no tileset with id {t}; tileset.list shows the ids"
            ))
        })?),
        (false, None) => None,
    };
    let name = args.str("name").map_or_else(
        || match tileset {
            Some(ts) => format!("{} Layer", ts.name),
            None => format!("Layer {}", doc.sprite.layers.len() + 1),
        },
        str::to_owned,
    );
    let mut layer = match (args.str("kind"), tileset) {
        (Some("group"), _) => Layer::group(lid, name),
        (_, Some(ts)) => Layer::tilemap(lid, name, ts.id),
        _ => Layer::image(lid, name),
    };
    // A tilemap layer gets an empty grid on every frame (AC-19).
    let seeded: Option<Vec<Cel>> = tileset.map(|ts| {
        let (tw, th) = ts.tile_size;
        let (gw, gh) = (
            doc.sprite.width.div_ceil(tw.max(1)),
            doc.sprite.height.div_ceil(th.max(1)),
        );
        (0..doc.sprite.frames.len())
            .map(|f| Cel::tilemap(lid, FrameIndex::new(f as u32), gw, gh))
            .collect()
    });
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
    let placement = placement(args)?;
    if placement.is_some() && args.get("parent").is_some() {
        return Err(AgentError::tool(
            "arguments `parent` and `target` exclude each other; use `target` with `place: \"into\"`",
        ));
    }
    let parent = group_arg(doc, args)?;
    let target = match placement {
        Some((_, place)) => {
            let target = layer_arg(doc, args, "target")?;
            check_placement(doc, None, target, place)?;
            Some((target, place_core(place)))
        }
        None => parent.map(|p| (p, LayerPlacement::Into)),
    };
    let mut cmds: Vec<AnyCommand> = vec![match seeded {
        Some(cels) => AddTilemapLayer::new(layer, cels).into(),
        None => AddLayer::on_top(layer).into(),
    }];
    if let Some((t, place)) = target {
        cmds.push(MoveLayerTo::new(lid, t, place).into());
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
    if ctx.ws.is_image(args)? {
        return crate::image::layer::set(ctx, args);
    }
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
        if ctx.ws.is_image(args)? {
            return crate::image::layer::remove(ctx, args);
        }
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
                "color": { "type": "string", "description": "Timeline color \"#RRGGBB\"; default: the timeline's next tag color" },
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
        tag.direction = direction_arg(d)?;
    }
    tag.color = match args.str("color") {
        Some(c) => parse_hex(c).filter(|_| c.len() == 7).ok_or_else(|| {
            AgentError::tool(format!(
                "argument `color`: malformed tag color {c:?}; tags take \"#RRGGBB\""
            ))
        })?,
        None => Rgba::from_u32(TAG_COLORS[doc.sprite.tags.len() % TAG_COLORS.len()] << 8 | 0xff),
    };
    doc.commit(vec![AddTag::new(tag).into()])?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "tag": doc.sprite.tags.len() - 1,
        "tags": tags_json(&doc.sprite),
    })))
}

fn direction_arg(d: &str) -> Result<TagDirection, AgentError> {
    DIRECTIONS
        .iter()
        .find(|(n, _)| *n == d)
        .map(|(_, dir)| *dir)
        .ok_or_else(|| AgentError::tool(format!("argument `direction`: unknown direction `{d}`")))
}

/// The `tag` argument as an existing tag index (AC-27).
fn tag_arg(doc: &Doc, args: &Args) -> Result<usize, AgentError> {
    let index = args.req_u32("tag")? as usize;
    let count = doc.sprite.tags.len();
    if index < count {
        Ok(index)
    } else {
        Err(AgentError::tool(format!(
            "argument `tag`: no tag with index {index} (document has {count} tags); tag.list shows them"
        )))
    }
}

pub(super) const TAG_SET: Tool = Tool {
    name: "tag.set",
    description: "Change an animation tag's name, frame range (`from` and `to` together) or playback direction. Fields not given stay; one undo step.",
    schema: || {
        let dirs: Vec<&str> = DIRECTIONS.iter().map(|(n, _)| *n).collect();
        tool_schema(
            json!({
                "tag": { "type": "integer", "minimum": 0, "description": "Tag index from tag.list" },
                "name": { "type": "string", "description": "New tag name" },
                "from": { "type": "integer", "minimum": 0, "description": "New first frame index (with `to`)" },
                "to": { "type": "integer", "minimum": 0, "description": "New last frame index, inclusive (with `from`)" },
                "direction": { "type": "string", "enum": dirs, "description": "New playback direction" },
            }),
            &["tag"],
        )
    },
    run: tag_set,
};

fn tag_set(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let index = tag_arg(doc, args)?;
    let name = args.str("name");
    let direction = args.str("direction").map(direction_arg).transpose()?;
    let range = match (args.u32("from")?, args.u32("to")?) {
        (None, None) => None,
        (Some(_), None) => {
            return Err(AgentError::tool(
                "argument `from` needs `to` (the last frame of the range)",
            ));
        }
        (None, Some(_)) => {
            return Err(AgentError::tool(
                "argument `to` needs `from` (the first frame of the range)",
            ));
        }
        (Some(f), Some(t)) => {
            let from = doc
                .check_frame(f)
                .map_err(|e| AgentError::tool(format!("argument `from`: {e}")))?;
            let to = doc
                .check_frame(t)
                .map_err(|e| AgentError::tool(format!("argument `to`: {e}")))?;
            if f > t {
                return Err(AgentError::tool(format!(
                    "argument `from` ({f}) is greater than `to` ({t})"
                )));
            }
            Some((from, to))
        }
    };
    if name.is_none() && range.is_none() && direction.is_none() {
        return Err(AgentError::tool(
            "tag.set needs at least one of `name`, `from` + `to` or `direction`",
        ));
    }
    if name.is_some_and(str::is_empty) {
        return Err(AgentError::tool(
            "argument `name`: a tag name cannot be empty",
        ));
    }
    let changed = doc.commit_if_changed(|| {
        let mut cmds: Vec<AnyCommand> = Vec::new();
        if let Some(n) = name {
            cmds.push(SetTagName::new(index, n).into());
        }
        if let Some((from, to)) = range {
            cmds.push(SetTagRange::new(index, from, to).into());
        }
        if let Some(d) = direction {
            cmds.push(SetTagDirection::new(index, d).into());
        }
        cmds
    })?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "changed": changed,
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
        let index = tag_arg(doc, args)?;
        doc.commit(vec![RemoveTag::new(index).into()])?;
        Ok(ToolOutput::json(
            json!({ "doc": id, "tags": tags_json(&doc.sprite) }),
        ))
    },
};
