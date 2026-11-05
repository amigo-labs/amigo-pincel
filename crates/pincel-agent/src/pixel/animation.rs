//! `export.apng` and `export.gif`: the Pixel editor's File ▸ Export ▸
//! Animation… (docs/specs/mcp-parity-export.md AC-11 … AC-13, AC-27).

use pincel_core::{AnimationFormat, AnimationOptions, animation_info, export_animation};
use serde_json::{Value, json};

use super::{path_prop, tool_schema};
use crate::args::Args;
use crate::{AgentError, Ctx, Tool, ToolOutput};

fn schema() -> Value {
    tool_schema(
        json!({
            "path": path_prop("Workspace-relative output path"),
            "tag": { "type": "string", "description": "Export only this tag's frames, in its direction; default every frame once" },
            "scale": { "type": "integer", "minimum": 1, "maximum": 10, "description": "Integer upscale 1–10; default 1" },
            "loops": { "type": "integer", "minimum": 0, "maximum": 65535, "description": "Plays, 0 = forever; default the tag's repeat count, else 0" },
        }),
        &["path"],
    )
}

fn export(
    ctx: &mut Ctx<'_>,
    args: &Args,
    format: AnimationFormat,
) -> Result<ToolOutput, AgentError> {
    let (id, doc) = ctx.ws.doc(args)?;
    let path = args.req_str("path")?.to_owned();
    let tag = args.str("tag").map(str::to_owned);
    let scale = args.u32("scale")?.unwrap_or(1);
    let info = animation_info(&doc.sprite, &doc.cels, tag.as_deref())
        .map_err(|e| AgentError::tool(format!("cannot export the animation: {e}")))?;
    let loops = match args.u32("loops")? {
        Some(l) => {
            u16::try_from(l).map_err(|_| AgentError::tool("argument `loops` must be 0–65535"))?
        }
        None => info.default_loops,
    };
    let mut opts = AnimationOptions::new(format)
        .with_scale(scale)
        .with_loops(loops);
    opts.tag = tag;
    let bytes = export_animation(&doc.sprite, &doc.cels, &opts)
        .map_err(|e| AgentError::tool(format!("cannot export the animation: {e}")))?;
    let (width, height) = (doc.sprite.width * scale, doc.sprite.height * scale);
    let mut out = json!({
        "doc": id,
        "path": path,
        "frames": info.frame_count,
        "width": width,
        "height": height,
        "loops": loops,
    });
    if format == AnimationFormat::Gif {
        let mut warnings = Vec::new();
        if info.gif_approximated {
            warnings.push(json!({
                "code": "colors_approximated",
                "message": "A frame has more colors than a GIF color table holds (256); its colors are approximated.",
            }));
        }
        if info.semi_transparent {
            warnings.push(json!({
                "code": "transparency_on_off",
                "message": "A frame has semi-transparent pixels; GIF stores transparency only as on or off.",
            }));
        }
        out["warnings"] = json!(warnings);
    }
    ctx.host
        .check_write(&path)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    ctx.host
        .write(&path, &bytes)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    Ok(ToolOutput::json(out))
}

pub(super) const EXPORT_APNG: Tool = Tool {
    name: "export.apng",
    description: "Pixel documents: export the animation as an animated PNG, like the editor's File ▸ Export ▸ Animation…: every frame once (or a `tag`'s frames in its direction), upscaled by `scale` 1–10, playing `loops` times (0 = forever; default the tag's repeat count, else 0).",
    schema,
    run: |ctx, args| export(ctx, args, AnimationFormat::Apng),
};

pub(super) const EXPORT_GIF: Tool = Tool {
    name: "export.gif",
    description: "Pixel documents: export the animation as a GIF, like the editor's File ▸ Export ▸ Animation… (arguments as export.apng). `warnings` names what GIF cannot keep: colors_approximated (a frame has more than 256 colors) and transparency_on_off (semi-transparent pixels).",
    schema,
    run: |ctx, args| export(ctx, args, AnimationFormat::Gif),
};
