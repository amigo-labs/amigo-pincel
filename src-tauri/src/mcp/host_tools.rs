//! Tools the desktop app adds beside the document tools: `gen.image` and
//! `gen.models` (docs/specs/comfyui-generate.md AC-16 – AC-20, Decision Log
//! 2026-11-06 "ComfyUI"). They are host tools, not part of `pincel-agent`,
//! which stays free of I/O; like every call they run in the webview, which
//! drives the generation through the ComfyUI backend.

use pincel_agent::ToolDef;
use serde_json::json;

/// `gen.*` tools, listed whether or not a server is usable (mcp-live-desktop
/// AC-06, comfyui-generate AC-19).
pub fn host_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "gen.image",
            description: "Generate an image with the local image model (ComfyUI) and add it as a new layer directly above the active layer, as one undo step. Without a selection it fills the canvas; with a selection it generates only inside the selection (inpainting). `strength` below 1 refines the active layer's pixels instead of starting from noise. Returns the new layer's name, the seed and the model. Can take minutes; gen.models lists the valid `model` and `lora` names.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": { "type": "string", "description": "What to generate" },
                    "negative": { "type": "string", "description": "What to avoid (ignored by Flux models)" },
                    "model": { "type": "string", "description": "Checkpoint from gen.models; default: the Generate panel's model, else the first checkpoint" },
                    "lora": { "type": "string", "description": "LoRA from gen.models; default none" },
                    "lora_strength": { "type": "number", "minimum": 0, "maximum": 2, "description": "Default 1" },
                    "strength": { "type": "number", "minimum": 0.01, "maximum": 1, "description": "1 generates from the prompt alone (default); below 1 refines the active layer" },
                    "seed": { "type": "integer", "minimum": 0, "description": "Default random" },
                    "doc": { "type": "string", "description": "Document id from doc.list; default: the active document" },
                },
                "required": ["prompt"],
                "additionalProperties": false,
            }),
        },
        ToolDef {
            name: "gen.models",
            description: "List the checkpoints (models) and LoRAs of the connected ComfyUI server, by the names gen.image accepts.",
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
        },
    ]
}
