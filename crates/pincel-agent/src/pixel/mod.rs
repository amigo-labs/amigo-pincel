//! The shared document tools and the Pixel-mode tools over `pincel-core`.
//! See docs/specs/mcp-headless-pixel.md (Technical notes, "Tools").

mod doc;
mod draw;
pub(crate) mod effects;
mod layer_ops;
mod palette;
mod structure;

pub(crate) use doc::{EXPORT_MAX_EDGE, READ_MAX, path_prop, rect_json, region_arg, with_id};
pub(crate) use draw::apply_effect;
pub(crate) use structure::layer_arg as layer_id_arg;

use serde_json::{Map, Value, json};

use crate::Tool;
use crate::schema;

/// Every tool, in `tools/list` order.
pub(crate) static TOOLS: &[Tool] = &[
    doc::NEW,
    doc::OPEN,
    doc::LIST,
    doc::INFO,
    doc::CLOSE,
    doc::PREVIEW,
    doc::READ_PIXELS,
    doc::UNDO,
    doc::REDO,
    doc::SAVE,
    doc::EXPORT_PNG,
    doc::EXPORT_SHEET,
    structure::LAYER_ADD,
    structure::LAYER_REMOVE,
    structure::LAYER_LIST,
    structure::LAYER_SET,
    layer_ops::LAYER_DUPLICATE,
    layer_ops::LAYER_MOVE,
    layer_ops::LAYER_MERGE_DOWN,
    layer_ops::LAYER_MERGE_VISIBLE,
    layer_ops::LAYER_MERGE_GROUP,
    layer_ops::LAYER_FLATTEN,
    structure::FRAME_ADD,
    structure::FRAME_REMOVE,
    structure::FRAME_SET_DURATION,
    structure::FRAME_LIST,
    structure::TAG_ADD,
    structure::TAG_REMOVE,
    structure::TAG_LIST,
    structure::TAG_SET,
    palette::PALETTE_GET,
    palette::PALETTE_SET,
    palette::PALETTE_LOAD_PRESET,
    palette::PALETTE_SET_ENTRY,
    palette::PALETTE_ADD_ENTRY,
    palette::PALETTE_REMOVE_ENTRY,
    palette::PALETTE_MOVE_ENTRY,
    palette::PALETTE_IMPORT,
    palette::PALETTE_EXPORT,
    draw::PIXELS,
    draw::LINE,
    draw::RECT,
    draw::ELLIPSE,
    draw::FILL,
    draw::CLEAR,
    draw::EFFECT_APPLY,
];

/// Tool schema with the optional `doc` argument every tool takes.
pub(crate) fn tool_schema(properties: Value, required: &[&str]) -> Value {
    let mut props = match properties {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    props.insert(
        "doc".to_owned(),
        json!({
            "type": "string",
            "description": "Document id from doc.new / doc.open / doc.list; default: the active document",
        }),
    );
    schema::object(Value::Object(props), required)
}

/// Tool schema for pixel tools: `doc`, `layer` and `frame` (AC-43).
pub(crate) fn target_schema(properties: Value, required: &[&str]) -> Value {
    let mut props = match properties {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    props.insert("layer".to_owned(), layer_prop());
    props.insert("frame".to_owned(), frame_prop());
    tool_schema(Value::Object(props), required)
}

pub(crate) fn layer_prop() -> Value {
    json!({
        "type": "integer",
        "minimum": 0,
        "description": "Layer id from layer.list; default: the active layer",
    })
}

pub(crate) fn frame_prop() -> Value {
    json!({
        "type": "integer",
        "minimum": 0,
        "description": "Frame index from frame.list; default: the active frame",
    })
}

pub(crate) fn int(description: &str) -> Value {
    json!({ "type": "integer", "description": description })
}
