//! `draw.clone`: the Image editor's Clone Stamp (docs/specs/clone-stamp.md
//! AC-19, AC-20). Builds the `clone_stroke` command the editor sends for one
//! full-pressure stroke through `points`, delivered as one segment.

use pincel_image_core::DEFAULT_BRUSH_ANGLE;
use pincel_image_core::command::spec::CommandSpec;
use serde_json::json;

use super::draw::{
    BRUSH_SIZE, brush_props, choice, edit_checked, edit_schema, num_or, percent, points_arg,
    str_or, with_props,
};
use crate::args::Args;
use crate::{AgentError, Tool};

/// The `source` argument: two integers naming a pixel on a `w`×`h` canvas.
fn source_arg(args: &Args, w: u32, h: u32) -> Result<(i64, i64), AgentError> {
    let v = args.get("source").ok_or_else(|| {
        AgentError::tool(
            "missing required argument `source` ([x, y], the pixel copied to the first point)",
        )
    })?;
    let xy = v
        .as_array()
        .filter(|a| a.len() == 2)
        .and_then(|a| Some((a[0].as_i64()?, a[1].as_i64()?)));
    let Some((x, y)) = xy else {
        return Err(AgentError::tool(format!(
            "argument `source`: {v} is not an [x, y] pair of integers"
        )));
    };
    if x < 0 || y < 0 || x >= i64::from(w) || y >= i64::from(h) {
        return Err(AgentError::tool(format!(
            "argument `source`: [{x}, {y}] lies outside the {w}×{h} canvas"
        )));
    }
    Ok((x, y))
}

pub(crate) const CLONE: Tool = Tool {
    name: "draw.clone",
    description: "Image documents: the editor's Clone Stamp along `points`: the integer pixel `source` ([x, y]) is copied to the pixel under the first point, and every other pixel the brush covers receives the pixel at the same offset, composited by the brush coverage (as the Brush lays down color). `sample` current_layer (default) copies from the target layer, all_layers from the composite of the visible layers, both as they were before the call. Takes the brush arguments of draw.stroke (size default 8, hardness, opacity, shape, angle). Respects the selection; pixels whose source lies outside the canvas stay as they are. One undo step; no step when no pixel changes.",
    schema: || {
        edit_schema(
            with_props(
                brush_props(),
                json!({
                    "source": {
                        "type": "array",
                        "minItems": 2,
                        "maxItems": 2,
                        "items": { "type": "integer" },
                        "description": "[x, y]: the canvas pixel copied to the pixel under the first point",
                    },
                    "sample": choice(&["current_layer", "all_layers"], "Copy from the target layer (default) or from the composite of the visible layers"),
                }),
            ),
            &["points", "source"],
        )
    },
    run: |ctx, args| {
        let (w, h) = {
            let (_, doc) = ctx.ws.image_mut(args)?;
            (doc.width(), doc.height())
        };
        edit_checked(ctx, args, |layer, stroke_id| {
            let points = points_arg(args)?;
            let (sx, sy) = source_arg(args, w, h)?;
            // Source minus the pixel under the first point; a point far
            // off the canvas paints nothing, whatever the clamped offset.
            let delta = |s: i64, p: f32| -> i32 {
                (s - p.floor() as i64).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
            };
            let offset = [delta(sx, points[0][0]), delta(sy, points[0][1])];
            Ok(CommandSpec::CloneStroke {
                layer,
                size: args.i64("size").map_or(BRUSH_SIZE, |s| s as u64) as u32,
                opacity: percent(args, "opacity"),
                shape: str_or(args, "shape", "hard_round").to_owned(),
                hardness: percent(args, "hardness"),
                offset,
                sample: str_or(args, "sample", "current_layer").to_owned(),
                points,
                pressure: Vec::new(),
                pressure_size: false,
                pressure_opacity: false,
                azimuth: Vec::new(),
                angle: num_or(args, "angle", f64::from(DEFAULT_BRUSH_ANGLE)),
                tilt_angle: false,
                anti_alias: false,
                stroke_id,
            })
        })
    },
};
