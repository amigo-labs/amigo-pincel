//! `layer.*` on Image documents: the layer tree (groups keep their
//! subtree as one run after the group entry), built from the editor's
//! layer commands. See docs/specs/mcp-parity-structure.md.

use pincel_image_core::command::{
    AddGroup, AddLayer, Command, DuplicateLayer, FlattenImage, MergeDown, MergeGroup, MergeVisible,
    MoveLayerTo, RemoveLayer, RenameLayer, SetLayerBlendMode, SetLayerLocked, SetLayerOpacity,
    SetLayerVisible,
};
use pincel_image_core::{BlendMode, LayerPlacement};
use serde_json::{Value, json};

use super::ImageDoc;
use crate::args::{Args, Place, placement};
use crate::{AgentError, Ctx, ToolOutput};

/// Image blend modes by their wire name (the Pixel names they share).
const BLENDS: [(&str, BlendMode); 12] = [
    ("normal", BlendMode::Normal),
    ("multiply", BlendMode::Multiply),
    ("screen", BlendMode::Screen),
    ("overlay", BlendMode::Overlay),
    ("darken", BlendMode::Darken),
    ("lighten", BlendMode::Lighten),
    ("color_dodge", BlendMode::ColorDodge),
    ("color_burn", BlendMode::ColorBurn),
    ("hard_light", BlendMode::HardLight),
    ("soft_light", BlendMode::SoftLight),
    ("difference", BlendMode::Difference),
    ("exclusion", BlendMode::Exclusion),
];

fn blend_name(mode: BlendMode) -> &'static str {
    BLENDS
        .iter()
        .find(|(_, m)| *m == mode)
        .map_or("normal", |(n, _)| *n)
}

fn blend_arg(name: &str) -> Result<BlendMode, AgentError> {
    BLENDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, m)| *m)
        .ok_or_else(|| {
            let names: Vec<&str> = BLENDS.iter().map(|(n, _)| *n).collect();
            AgentError::tool(format!(
                "blend mode `{name}` is not available in Image documents (available: {})",
                names.join(", ")
            ))
        })
}

/// Opacity on the tools' 0–255 scale.
fn opacity_255(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn opacity_arg(args: &Args) -> Result<Option<f32>, AgentError> {
    args.i64("opacity")
        .map(|o| {
            u8::try_from(o)
                .map(|o| f32::from(o) / 255.0)
                .map_err(|_| AgentError::tool("opacity must be 0–255"))
        })
        .transpose()
}

pub(crate) fn layers_json(doc: &ImageDoc) -> Value {
    let d = doc.document();
    let layers: Vec<Value> = d
        .layers()
        .iter()
        .enumerate()
        .map(|(i, l)| {
            json!({
                "id": doc.layer_id(i),
                "index": i,
                "name": l.name,
                "kind": if l.is_group() { "group" } else { "image" },
                "parent": d.parent_index(i).map(|p| doc.layer_id(p)),
                "visible": l.visible,
                "opacity": opacity_255(l.opacity),
                "blend": blend_name(l.blend_mode),
                "locked": l.locked,
            })
        })
        .collect();
    json!(layers)
}

fn place_core(p: Place) -> LayerPlacement {
    match p {
        Place::Above => LayerPlacement::Above,
        Place::Below => LayerPlacement::Below,
        Place::Into => LayerPlacement::Into,
    }
}

/// The property commands of `layer.add` / `layer.set` for the layer at
/// `index`.
fn property_cmds(args: &Args, index: usize) -> Result<Vec<Box<dyn Command>>, AgentError> {
    let mut cmds: Vec<Box<dyn Command>> = Vec::new();
    if let Some(n) = args.str("name") {
        cmds.push(Box::new(RenameLayer::new(index, n.to_owned())));
    }
    if let Some(v) = args.bool("visible") {
        cmds.push(Box::new(SetLayerVisible::new(index, v)));
    }
    if let Some(o) = opacity_arg(args)? {
        cmds.push(Box::new(SetLayerOpacity::new(index, o)));
    }
    if let Some(b) = args.str("blend") {
        cmds.push(Box::new(SetLayerBlendMode::new(index, blend_arg(b)?)));
    }
    if let Some(l) = args.bool("locked") {
        cmds.push(Box::new(SetLayerLocked::new(index, l)));
    }
    Ok(cmds)
}

/// The `parent` argument as the index of an existing group.
fn parent_arg(doc: &ImageDoc, args: &Args) -> Result<Option<usize>, AgentError> {
    let Some(id) = args.u32("parent")? else {
        return Ok(None);
    };
    let index = doc.layer_arg(id, "parent")?;
    if !doc.document().layers()[index].is_group() {
        return Err(AgentError::tool(format!(
            "argument `parent`: layer {id} is not a group layer"
        )));
    }
    Ok(Some(index))
}

/// Check a move of the subtree at `index` relative to `target`
/// (mcp-parity-structure AC-07). `index` is `None` for a new layer.
fn check_placement(
    doc: &ImageDoc,
    index: Option<usize>,
    target: usize,
    place: Place,
) -> Result<(), AgentError> {
    let d = doc.document();
    let tid = doc.layer_id(target);
    if let Some(i) = index {
        if i == target {
            return Err(AgentError::tool(format!(
                "argument `target`: layer {tid} is the moved layer itself"
            )));
        }
        if (i..d.subtree_end(i)).contains(&target) {
            return Err(AgentError::tool(format!(
                "argument `target`: layer {tid} is inside the moved group; a group cannot move into its own subtree"
            )));
        }
    }
    if place == Place::Into && !d.layers()[target].is_group() {
        return Err(AgentError::tool(format!(
            "argument `place`: \"into\" needs a group as `target`, and layer {tid} is not a group"
        )));
    }
    Ok(())
}

pub(crate) fn list(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image(args)?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}

/// `layer.add`: a transparent layer directly above the active layer (a
/// group: on top of the root stack, as the editor's Add Group), or placed
/// by `parent` / `target` + `place`; made active.
pub(crate) fn add(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let placement = placement(args)?;
    if placement.is_some() && args.get("parent").is_some() {
        return Err(AgentError::tool(
            "arguments `parent` and `target` exclude each other; use `target` with `place: \"into\"`",
        ));
    }
    let parent = parent_arg(doc, args)?;
    let target = match placement {
        Some((t, p)) => {
            let t = doc.layer_arg(t, "target")?;
            check_placement(doc, None, t, p)?;
            Some((t, place_core(p)))
        }
        None => parent.map(|g| (g, LayerPlacement::Into)),
    };
    let d = doc.document();
    let group = args.str("kind") == Some("group");
    let active = d.active_layer_index();
    let new_index = if group {
        d.layer_count()
    } else {
        d.subtree_end(active)
    };
    let mut cmds: Vec<Box<dyn Command>> = if group {
        vec![Box::new(AddGroup::on_top())]
    } else {
        vec![Box::new(AddLayer::above(active))]
    };
    cmds.extend(property_cmds(args, new_index)?);
    if let Some((t, place)) = target {
        // The insertion shifts every layer at or above it up by one.
        let t = if t >= new_index { t + 1 } else { t };
        cmds.push(Box::new(MoveLayerTo::new(new_index, t, place)));
    }
    doc.apply(cmds)?;
    let new = doc.document().active_layer_index();
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": doc.layer_id(new),
        "layers": layers_json(doc),
    })))
}

pub(crate) fn set(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let index = doc.layer_arg(args.req_u32("layer")?, "layer")?;
    let mut cmds = property_cmds(args, index)?;
    if let Some(g) = parent_arg(doc, args)? {
        check_placement(doc, Some(index), g, Place::Into)?;
        cmds.push(Box::new(MoveLayerTo::new(index, g, LayerPlacement::Into)));
    }
    doc.apply(cmds)?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}

pub(crate) fn remove(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let index = doc.layer_arg(args.req_u32("layer")?, "layer")?;
    doc.check_unlocked(index)?;
    doc.apply(vec![Box::new(RemoveLayer::at(index))])?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}

/// `layer.duplicate`: the layer (a group with its subtree) copied directly
/// above itself and made active.
pub(crate) fn duplicate(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let index = doc.layer_arg(args.req_u32("layer")?, "layer")?;
    doc.apply(vec![Box::new(DuplicateLayer::at(index))])?;
    let copy = doc.document().active_layer_index();
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": doc.layer_id(copy),
        "layers": layers_json(doc),
    })))
}

/// `layer.move`: the layer with its subtree above, below or into `target`.
pub(crate) fn move_to(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let index = doc.layer_arg(args.req_u32("layer")?, "layer")?;
    let (target, place) = placement(args)?
        .ok_or_else(|| AgentError::tool("missing required arguments `target` and `place`"))?;
    let target = doc.layer_arg(target, "target")?;
    check_placement(doc, Some(index), target, place)?;
    let changed = doc
        .apply_if_changed(|| vec![Box::new(MoveLayerTo::new(index, target, place_core(place)))])?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "changed": changed, "layers": layers_json(doc) }),
    ))
}

/// Refuse when one of `indices` is locked itself or through a group
/// (mcp-parity-structure AC-29).
fn check_all_unlocked(
    doc: &ImageDoc,
    indices: impl IntoIterator<Item = usize>,
) -> Result<(), AgentError> {
    indices.into_iter().try_for_each(|i| doc.check_unlocked(i))
}

/// `layer.merge_down`: the editor's Merge Down of a raster layer onto the
/// raster layer directly below it in the same group.
pub(crate) fn merge_down(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let lid = args.req_u32("layer")?;
    let index = doc.layer_arg(lid, "layer")?;
    let d = doc.document();
    if d.layers()[index].is_group() {
        return Err(AgentError::tool(format!(
            "layer {lid} is a group; Merge Down needs an image layer (layer.merge_group merges a group)"
        )));
    }
    if index == 0 {
        return Err(AgentError::tool(format!(
            "layer {lid} is the bottom layer; there is no layer below it to merge into"
        )));
    }
    let below = &d.layers()[index - 1];
    if below.is_group() || below.parent != d.layers()[index].parent {
        return Err(AgentError::tool(format!(
            "layer {lid} has no image layer directly below it in the same group"
        )));
    }
    check_all_unlocked(doc, [index, index - 1])?;
    doc.apply(vec![Box::new(MergeDown::at(index))])?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}

/// `layer.merge_visible`: the editor's Merge Visible.
pub(crate) fn merge_visible(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let d = doc.document();
    // Merge Visible replaces every visible root-level subtree.
    let mut merged = Vec::new();
    let mut i = 0;
    while i < d.layer_count() {
        let end = d.subtree_end(i);
        if d.layers()[i].visible {
            merged.extend(i..end);
        }
        i = end;
    }
    check_all_unlocked(doc, merged)?;
    let changed = doc.apply_if_changed(|| vec![Box::new(MergeVisible::new())])?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "changed": changed, "layers": layers_json(doc) }),
    ))
}

/// `layer.merge_group`: the editor's Merge Group.
pub(crate) fn merge_group(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let lid = args.req_u32("layer")?;
    let index = doc.layer_arg(lid, "layer")?;
    if !doc.document().layers()[index].is_group() {
        return Err(AgentError::tool(format!(
            "argument `layer`: layer {lid} is not a group; layer.merge_group merges a group"
        )));
    }
    check_all_unlocked(doc, index..doc.document().subtree_end(index))?;
    doc.apply(vec![Box::new(MergeGroup::at(index))])?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}

/// `layer.flatten`: the editor's Flatten (onto white).
pub(crate) fn flatten(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    check_all_unlocked(doc, 0..doc.document().layer_count())?;
    let changed = doc.apply_if_changed(|| vec![Box::new(FlattenImage::new())])?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "changed": changed, "layers": layers_json(doc) }),
    ))
}
