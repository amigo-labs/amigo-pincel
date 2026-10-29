//! `layer.*` on Image documents: a flat layer stack (no groups until
//! `image-layer-groups`), built from the editor's layer commands.

use pincel_image_core::BlendMode;
use pincel_image_core::command::{
    AddLayer, Command, RemoveLayer, RenameLayer, SetLayerBlendMode, SetLayerLocked,
    SetLayerOpacity, SetLayerVisible,
};
use serde_json::{Value, json};

use super::ImageDoc;
use crate::args::Args;
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

/// Image documents have no layer groups (AC-05).
fn refuse_groups(id: &str, args: &Args) -> Result<(), AgentError> {
    if args.get("parent").is_some() || args.str("kind") == Some("group") {
        return Err(AgentError::tool(format!(
            "document `{id}` is an Image document, which has no layer groups; `parent` and kind `group` work on Pixel documents only"
        )));
    }
    Ok(())
}

pub(crate) fn layers_json(doc: &ImageDoc) -> Value {
    let layers: Vec<Value> = doc
        .document()
        .layers()
        .iter()
        .enumerate()
        .map(|(i, l)| {
            json!({
                "id": doc.layer_id(i),
                "index": i,
                "name": l.name,
                "kind": "image",
                "parent": null,
                "visible": l.visible,
                "opacity": opacity_255(l.opacity),
                "blend": blend_name(l.blend_mode),
                "locked": l.locked,
            })
        })
        .collect();
    json!(layers)
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

pub(crate) fn list(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image(args)?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}

/// `layer.add`: a transparent layer on top of the stack, made active.
pub(crate) fn add(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    refuse_groups(&id, args)?;
    let top = doc.document().layer_count() - 1;
    let mut cmds: Vec<Box<dyn Command>> = vec![Box::new(AddLayer::above(top))];
    cmds.extend(property_cmds(args, top + 1)?);
    doc.apply(cmds)?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "layer": doc.layer_id(top + 1),
        "layers": layers_json(doc),
    })))
}

pub(crate) fn set(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    refuse_groups(&id, args)?;
    let index = doc.layer_index(args.req_u32("layer")?)?;
    doc.apply(property_cmds(args, index)?)?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}

pub(crate) fn remove(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.image_mut(args)?;
    let index = doc.layer_index(args.req_u32("layer")?)?;
    doc.check_unlocked(index)?;
    doc.apply(vec![Box::new(RemoveLayer::at(index))])?;
    Ok(ToolOutput::json(
        json!({ "doc": id, "layers": layers_json(doc) }),
    ))
}
