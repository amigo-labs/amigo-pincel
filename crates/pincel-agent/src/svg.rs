//! `export.svg`: the editor's File ▸ Export ▸ SVG… on both modes, traced
//! by `pincel-trace` with the dialog's options
//! (docs/specs/mcp-parity-export.md AC-05 … AC-10).

use pincel_core::{ComposeRequest, compose as compose_pixel};
use pincel_trace::{TraceError, TraceOptions, TraceSession, to_svg};
use serde_json::{Value, json};

use crate::args::Args;
use crate::pixel::{frame_prop, path_prop, tool_schema};
use crate::{AgentError, Ctx, Tool, ToolOutput};

pub(crate) const EXPORT_SVG: Tool = Tool {
    name: "export.svg",
    description: "Trace the composed image (Pixel documents: `frame`, default the active frame) into an SVG file, like the editor's File ▸ Export ▸ SVG…. `mode`: pixel_exact (one square per pixel, at most 256 colors; the default on Pixel documents), colors (`colors` 2–16, default 6; the default on Image documents) or mono (`threshold` \"auto\" or 0–255, `invert`, `fill` #RRGGBB, default #000000). Options: `despeckle` (default false), `min_area` 0–100000 (default 2), `alpha_max` 0–1.34 (corner threshold, default 1.0), `opt_curve` (default true), `opt_tolerance` ≥ 0 (default 0.2). Hidden layers are not traced.",
    schema: || {
        tool_schema(
            json!({
                "path": path_prop("Workspace-relative output path"),
                "mode": { "type": "string", "enum": ["pixel_exact", "colors", "mono"], "description": "Trace mode; default pixel_exact (Pixel) / colors (Image)" },
                "colors": { "type": "integer", "minimum": 2, "maximum": 16, "description": "colors: number of colors 2–16; default 6" },
                "threshold": {
                    "anyOf": [{ "type": "string", "enum": ["auto"] }, { "type": "integer", "minimum": 0, "maximum": 255 }],
                    "description": "mono: \"auto\" (default) or 0–255",
                },
                "invert": { "type": "boolean", "description": "mono: trace the light side; default false" },
                "fill": { "type": "string", "description": "mono: fill color #RRGGBB; default #000000" },
                "despeckle": { "type": "boolean", "description": "Remove single-pixel noise first; default false" },
                "min_area": { "type": "integer", "minimum": 0, "maximum": 100000, "description": "Drop shapes smaller than this many px; default 2" },
                "alpha_max": { "type": "number", "minimum": 0, "maximum": 1.34, "description": "Corner threshold 0–1.34; default 1.0" },
                "opt_curve": { "type": "boolean", "description": "Join curves; default true" },
                "opt_tolerance": { "type": "number", "minimum": 0, "description": "Curve optimisation tolerance, at least 0; default 0.2" },
                "frame": frame_prop(),
            }),
            &["path"],
        )
    },
    run: export_svg,
};

/// The dialog's wire mode numbers.
fn mode_number(name: &str) -> Option<u8> {
    match name {
        "colors" => Some(0),
        "mono" => Some(1),
        "pixel_exact" => Some(2),
        _ => None,
    }
}

/// The dialog's option vector (`TraceOptions::from_wire`), filled with
/// the dialog's defaults.
fn wire(args: &Args) -> Result<Vec<f64>, AgentError> {
    let num = |k: &str, d: f64| args.get(k).and_then(Value::as_f64).unwrap_or(d);
    let flag = |k: &str, d: bool| f64::from(u8::from(args.bool(k).unwrap_or(d)));
    let threshold = match args.get("threshold") {
        Some(v) if v.is_number() => v.as_f64().unwrap_or(-1.0),
        _ => -1.0,
    };
    let fill = match args.str("fill") {
        None => 0.0,
        Some(f) => {
            let hex = f
                .strip_prefix('#')
                .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()));
            let v = hex
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .ok_or_else(|| {
                    AgentError::tool(format!(
                        "argument `fill` must be a color #RRGGBB (got {f:?}); the traced fill has no alpha"
                    ))
                })?;
            f64::from(v)
        }
    };
    Ok(vec![
        num("colors", 6.0),
        threshold,
        flag("invert", false),
        fill,
        flag("despeckle", false),
        num("min_area", 2.0),
        num("alpha_max", 1.0),
        flag("opt_curve", true),
        num("opt_tolerance", 0.2),
    ])
}

fn export_svg(ctx: &mut Ctx<'_>, args: &Args) -> Result<ToolOutput, AgentError> {
    let path = args.req_str("path")?.to_owned();
    let image = ctx.ws.is_image(args)?;
    let mode = match args.str("mode") {
        Some(m) => mode_number(m).ok_or_else(|| {
            AgentError::tool(format!(
                "argument `mode`: unknown mode {m:?} (pixel_exact, colors, mono)"
            ))
        })?,
        None if image => 0,
        None => 2,
    };
    let params = wire(args)?;
    let (id, rgba, w, h) = if image {
        let (id, doc) = ctx.ws.image(args)?;
        if let Some(f) = args.u32("frame")?.filter(|&f| f != 0) {
            return Err(AgentError::tool(format!(
                "argument `frame`: frame {f} does not exist; Image documents have a single frame (0)"
            )));
        }
        let img = pincel_image_core::compose(doc.document().layers());
        let (w, h) = (img.width(), img.height());
        (id, img.into_raw(), w, h)
    } else {
        let (id, doc) = ctx.ws.doc(args)?;
        let frame = doc.frame_arg(args)?.unwrap_or(doc.active_frame);
        let request = ComposeRequest::full(frame, doc.sprite.width, doc.sprite.height);
        let mut pixels = Vec::new();
        let composed = compose_pixel(&doc.sprite, &doc.cels, &request, &mut pixels)
            .map_err(|e| AgentError::tool(format!("cannot compose frame {}: {e}", frame.0)))?;
        (id, pixels, composed.width, composed.height)
    };
    let fail = |e: TraceError| match e {
        TraceError::TooManyColors { .. } => AgentError::tool(format!(
            "cannot trace pixel-exact: {e}; use `mode: \"colors\"` to reduce the colors"
        )),
        other => AgentError::tool(format!("cannot trace: {other}")),
    };
    let opts = TraceOptions::from_wire(mode, &params).map_err(fail)?;
    let mut session = TraceSession::new(&rgba, w, h).map_err(fail)?;
    let result = session.trace(&opts).map_err(fail)?;
    let svg = to_svg(&result);
    ctx.host
        .check_write(&path)
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    ctx.host
        .write(&path, svg.as_bytes())
        .map_err(|e| AgentError::tool(format!("cannot write `{path}`: {e}")))?;
    Ok(ToolOutput::json(json!({
        "doc": id,
        "path": path,
        "layers": result.layers.len(),
        "outlines": result.stats.paths,
        "bytes": svg.len(),
        "threshold": result.threshold_used,
    })))
}
