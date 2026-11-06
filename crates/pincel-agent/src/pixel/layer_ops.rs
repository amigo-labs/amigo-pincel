//! `layer.duplicate`, `layer.move`, `layer.merge_*` and `layer.flatten`:
//! the layer operations of both editors (docs/specs/mcp-parity-structure.md).
//! Pixel documents run `pincel-core` commands here; Image documents go to
//! `crate::image::layer`.

use pincel_core::{
    DuplicateLayer, FlattenImage, LayerId, LayerKind, LayerPlacement, MergeDown, MoveLayerTo,
};
use serde_json::{Value, json};

use super::structure::{layer_arg, layers_json};
use super::tool_schema;
use crate::args::{Args, Place, placement, placement_props};
use crate::image::layer as image_layer;
use crate::workspace::Doc;
use crate::{AgentError, Ctx, Tool, ToolOutput};

fn layer_only_schema(description: &str) -> Value {
    tool_schema(
        json!({ "layer": { "type": "integer", "minimum": 0, "description": description } }),
        &["layer"],
    )
}

pub(super) fn place_core(p: Place) -> LayerPlacement {
    match p {
        Place::Above => LayerPlacement::Above,
        Place::Below => LayerPlacement::Below,
        Place::Into => LayerPlacement::Into,
    }
}

/// `true` when `id` sits (at any depth) inside the group `ancestor`.
fn descends_from(doc: &Doc, id: LayerId, ancestor: LayerId) -> bool {
    let mut cur = doc.sprite.layer(id).and_then(|l| l.parent);
    for _ in 0..=doc.sprite.layers.len() {
        match cur {
            Some(p) if p == ancestor => return true,
            Some(p) => cur = doc.sprite.layer(p).and_then(|l| l.parent),
            None => return false,
        }
    }
    false
}

/// Check a move of `layer` (`None` for a new layer) relative to `target`
/// (mcp-parity-structure AC-07).
pub(super) fn check_placement(
    doc: &Doc,
    layer: Option<LayerId>,
    target: LayerId,
    place: Place,
) -> Result<(), AgentError> {
    if let Some(l) = layer {
        if l == target {
            return Err(AgentError::tool(format!(
                "argument `target`: layer {} is the moved layer itself",
                target.0
            )));
        }
        if descends_from(doc, target, l) {
            return Err(AgentError::tool(format!(
                "argument `target`: layer {} is inside the moved group; a group cannot move into its own subtree",
                target.0
            )));
        }
    }
    let is_group = doc
        .sprite
        .layer(target)
        .is_some_and(|t| matches!(t.kind, LayerKind::Group));
    if place == Place::Into && !is_group {
        return Err(AgentError::tool(format!(
            "argument `place`: \"into\" needs a group as `target`, and layer {} is not a group",
            target.0
        )));
    }
    Ok(())
}

pub(super) const LAYER_DUPLICATE: Tool = Tool {
    name: "layer.duplicate",
    description: "Duplicate a layer directly above itself, named \"<name> copy\", with the same pixels in every frame, visibility, opacity, blend mode and lock state, and make the copy active. Returns its id. Image documents copy a group with its whole subtree; Pixel groups cannot be duplicated.",
    schema: || layer_only_schema("Layer id from layer.list"),
    run: layer_duplicate,
};

fn layer_duplicate(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        return image_layer::duplicate(ctx, args);
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let lid = layer_arg(doc, args, "layer")?;
    if doc
        .sprite
        .layer(lid)
        .is_some_and(|l| matches!(l.kind, LayerKind::Group))
    {
        return Err(AgentError::tool(format!(
            "layer {} is a group; Pixel groups cannot be duplicated (duplicate the layers inside it)",
            lid.0
        )));
    }
    doc.commit(vec![DuplicateLayer::new(lid).into()])?;
    // The command takes the next free id, the highest one now.
    let copy = doc.sprite.layers.iter().map(|l| l.id).max_by_key(|l| l.0);
    doc.active_layer = copy;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": copy.map(|l| l.0),
        "layers": layers_json(&doc.sprite),
    })))
}

pub(super) const LAYER_MOVE: Tool = Tool {
    name: "layer.move",
    description: "Move a layer (a group with its whole subtree) directly above or below the layer `target` as its sibling, or into the group `target` as its top child (`place`: above, below, into). Works on locked layers.",
    schema: || {
        let mut props = placement_props();
        props["layer"] =
            json!({ "type": "integer", "minimum": 0, "description": "Id of the layer to move" });
        tool_schema(props, &["layer"])
    },
    run: layer_move,
};

fn layer_move(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        return image_layer::move_to(ctx, args);
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let lid = layer_arg(doc, args, "layer")?;
    let (_, place) = placement(args)?
        .ok_or_else(|| AgentError::tool("missing required arguments `target` and `place`"))?;
    let target = layer_arg(doc, args, "target")?;
    check_placement(doc, Some(lid), target, place)?;
    let changed =
        doc.commit_if_changed(|| vec![MoveLayerTo::new(lid, target, place_core(place)).into()])?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "changed": changed,
        "layers": layers_json(&doc.sprite),
    })))
}

/// Refuse when the layer `id` is locked itself or through a group
/// (mcp-parity-structure AC-29).
fn check_unlocked(doc: &Doc, ids: impl IntoIterator<Item = LayerId>) -> Result<(), AgentError> {
    ids.into_iter().try_for_each(|l| doc.check_unlocked(l))
}

pub(super) const LAYER_MERGE_DOWN: Tool = Tool {
    name: "layer.merge_down",
    description: "Merge a layer into the image layer directly below it in the same group, as the editor's Merge Down, and remove it. Refused for the bottom layer, a group, a layer without an image layer directly below it, and locked layers.",
    schema: || layer_only_schema("Id of the upper layer"),
    run: layer_merge_down,
};

fn layer_merge_down(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        return image_layer::merge_down(ctx, args);
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let lid = layer_arg(doc, args, "layer")?;
    let layers = &doc.sprite.layers;
    let index = layers
        .iter()
        .position(|l| l.id == lid)
        .ok_or_else(|| AgentError::tool(format!("argument `layer`: no layer with id {}", lid.0)))?;
    match layers[index].kind {
        LayerKind::Image => {}
        LayerKind::Group => {
            return Err(AgentError::tool(format!(
                "layer {} is a group; Merge Down needs an image layer",
                lid.0
            )));
        }
        LayerKind::Tilemap { .. } => {
            return Err(AgentError::tool(format!(
                "layer {} is a tilemap layer; Merge Down needs an image layer",
                lid.0
            )));
        }
    }
    if index == 0 {
        return Err(AgentError::tool(format!(
            "layer {} is the bottom layer; there is no layer below it to merge into",
            lid.0
        )));
    }
    let below = &layers[index - 1];
    if !matches!(below.kind, LayerKind::Image) || below.parent != layers[index].parent {
        return Err(AgentError::tool(format!(
            "layer {} has no image layer directly below it in the same group (a group or tilemap layer is not an image layer)",
            lid.0
        )));
    }
    let below = below.id;
    check_unlocked(doc, [lid, below])?;
    doc.commit(vec![MergeDown::new(lid).into()])?;
    if doc.active_layer == Some(lid) {
        doc.active_layer = Some(below);
    }
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(&doc.sprite) }),
    ))
}

pub(super) const LAYER_MERGE_VISIBLE: Tool = Tool {
    name: "layer.merge_visible",
    description: "Image documents: merge every visible layer (a visible group with its subtree) into one layer named Merged, as the editor's Merge Visible; hidden layers stay.",
    schema: || tool_schema(json!({}), &[]),
    run: |ctx, args| {
        ctx.ws.image(args)?;
        image_layer::merge_visible(ctx, args)
    },
};

pub(super) const LAYER_MERGE_GROUP: Tool = Tool {
    name: "layer.merge_group",
    description: "Image documents: replace a group and its subtree with one image layer holding the group's composite, as the editor's Merge Group.",
    schema: || layer_only_schema("Id of the group"),
    run: |ctx, args| {
        ctx.ws.image(args)?;
        image_layer::merge_group(ctx, args)
    },
};

pub(super) const LAYER_FLATTEN: Tool = Tool {
    name: "layer.flatten",
    description: "Flatten the document into one layer, as the editor's Flatten: Pixel documents keep the visible composite of every frame (hidden layers are dropped); Image documents flatten onto white into a layer named Background. Refused while a layer is locked.",
    schema: || tool_schema(json!({}), &[]),
    run: layer_flatten,
};

fn layer_flatten(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    if ctx.ws.is_image(args)? {
        return image_layer::flatten(ctx, args);
    }
    let (id, doc) = ctx.ws.doc_mut(args)?;
    let ids: Vec<LayerId> = doc.sprite.layers.iter().map(|l| l.id).collect();
    check_unlocked(doc, ids)?;
    let changed = doc.commit_if_changed(|| vec![FlattenImage::new().into()])?;
    if changed {
        doc.active_layer = doc.sprite.layers.first().map(|l| l.id);
    }
    Ok(ToolOutput::json(json!({
        "doc": id,
        "changed": changed,
        "layers": layers_json(&doc.sprite),
    })))
}
