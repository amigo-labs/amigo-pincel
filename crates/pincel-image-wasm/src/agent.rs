//! Live agent calls (docs/specs/mcp-live-desktop.md): the desktop app runs
//! an MCP client's tool call on the document open in the editor, through
//! the same `pincel-agent` tools as the headless server.

use std::cell::RefCell;
use std::collections::HashMap;

use pincel_agent::live::{ImageAgentState, ImageLoan, LiveFile, WireFonts, call_image_wire};
use pincel_image_core::command::HistoryKind;
use wasm_bindgen::prelude::*;

use crate::{STROKES, with_bus};

thread_local! {
    /// The agent's state per document handle (stable layer ids, stroke
    /// counter), created by the first live call and dropped on close.
    pub(crate) static AGENT: RefCell<HashMap<u32, ImageAgentState>> = RefCell::new(HashMap::new());
}

/// Run the agent tool `name` with `args_json` (a JSON object, or empty for
/// none) on the document `handle`, which the reply calls `doc_id`. A
/// mutating call is exactly one undo step. `file_path` / `file_bytes` carry
/// the file the user picked for a reading tool (`layer.import`,
/// `palette.import`). `fonts_json` is the editor's font list with the faces
/// read for this call, by their [`register_font`](crate::register_font) ids
/// (`pincel_agent::live::WireFonts`, docs/specs/system-fonts-mcp.md AC-11);
/// without it `draw.text` knows the bundled face only. Returns the JSON
/// reply of `pincel_agent::live::LiveReply::to_json`; the files a writing
/// tool produced are in it for the app to save where the user confirms.
/// Errs only on an unknown handle.
#[wasm_bindgen]
pub fn agent_call(
    handle: u32,
    doc_id: &str,
    name: &str,
    args_json: &str,
    file_path: Option<String>,
    file_bytes: Option<Vec<u8>>,
    fonts_json: Option<String>,
) -> Result<String, JsError> {
    let input = match (&file_path, &file_bytes) {
        (Some(path), Some(bytes)) => Some(LiveFile { path, bytes }),
        _ => None,
    };
    with_bus(handle, |bus| {
        STROKES.with(|strokes| {
            AGENT.with(|agent| {
                let mut strokes = strokes.borrow_mut();
                let mut agent = agent.borrow_mut();
                Ok(call_image_wire(
                    doc_id,
                    ImageLoan {
                        bus,
                        scratch: strokes.entry(handle).or_default(),
                        state: agent.entry(handle).or_default(),
                    },
                    input,
                    fonts_json.as_deref().map(|json| WireFonts {
                        json,
                        registered: &crate::fonts::face,
                    }),
                    name,
                    args_json,
                ))
            })
        })
    })
}

/// An Assistant answer starts on the document `handle`: the steps until
/// [`assistant_end`] become one undo step (docs/specs/ai-chat-panel.md
/// AC-08).
#[wasm_bindgen(js_name = assistantBegin)]
pub fn assistant_begin(handle: u32) -> Result<(), JsError> {
    with_bus(handle, |bus| {
        bus.history.begin_group();
        Ok(())
    })
}

/// The Assistant answer on `handle` ended: its changes become one undo step
/// of kind `assistant`. Returns `false`, adding no step, when it changed
/// nothing (AC-09).
#[wasm_bindgen(js_name = assistantEnd)]
pub fn assistant_end(handle: u32) -> Result<bool, JsError> {
    with_bus(handle, |bus| {
        Ok(bus.history.end_group(HistoryKind::Assistant))
    })
}

/// Drop the agent state of a closed document.
pub(crate) fn forget(handle: u32) {
    AGENT.with(|agent| {
        agent.borrow_mut().remove(&handle);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{close_document, create_document, undo, with_bus};
    use pincel_image_core::command::CommandBus;

    fn call(handle: u32, name: &str, args: &str) -> serde_json::Value {
        let text = agent_call(handle, "tab-3", name, args, None, None, None).expect("handle");
        serde_json::from_str(&text).expect("json")
    }

    fn doc(handle: u32) -> pincel_image_core::Document {
        with_bus(handle, |bus: &mut CommandBus| Ok(bus.document.clone())).expect("doc")
    }

    #[test]
    fn agent_call_stroke_is_one_undo_step_on_the_open_document() {
        let handle = create_document(16, 8).expect("doc");
        let before = doc(handle);
        let r = call(
            handle,
            "draw.stroke",
            r##"{"points":[[1,1],[12,6]],"size":3,"color":"#00ff00"}"##,
        );
        assert_eq!(r["ok"], true, "{r}");
        assert_ne!(doc(handle), before);
        assert!(undo(handle).expect("undo"));
        assert_eq!(doc(handle), before);
        assert!(!undo(handle).expect("undo"), "exactly one step");
        close_document(handle);
    }

    #[test]
    fn assistant_answer_of_several_calls_is_one_assistant_step() {
        let handle = create_document(16, 8).expect("doc");
        let before = doc(handle);
        assistant_begin(handle).expect("handle");
        for color in ["#00ff00", "#ff0000"] {
            let args = format!(r#"{{"points":[[1,1],[12,6]],"size":3,"color":"{color}"}}"#);
            assert_eq!(call(handle, "draw.stroke", &args)["ok"], true);
        }
        assert!(assistant_end(handle).expect("handle"));
        assert_eq!(
            crate::history::undo_kinds(handle).expect("kinds"),
            ["assistant"]
        );
        assert!(undo(handle).expect("undo"));
        assert_eq!(doc(handle), before);
        assert!(!undo(handle).expect("undo"), "exactly one step");
        assistant_begin(handle).expect("handle");
        assert_eq!(call(handle, "doc.info", "")["ok"], true);
        assert!(
            !assistant_end(handle).expect("handle"),
            "no change, no step"
        );
        close_document(handle);
    }

    #[test]
    fn agent_call_keeps_layer_ids_until_the_document_closes() {
        let handle = create_document(4, 4).expect("doc");
        let a = call(handle, "layer.add", r#"{"name":"A"}"#)["json"]["layer"].clone();
        call(handle, "layer.add", r#"{"name":"B"}"#);
        let r = call(
            handle,
            "layer.set",
            &format!(r#"{{"layer":{a},"name":"A2"}}"#),
        );
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(r["json"]["doc"], "tab-3");
        close_document(handle);
        assert!(AGENT.with(|a| !a.borrow().contains_key(&handle)));
    }

    const SERIF_TTC: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pincel-core/tests/fixtures/PincelTestSerif.ttc"
    ));

    #[test]
    fn agent_call_draws_text_with_a_face_the_editor_registered() {
        let font = crate::register_font(SERIF_TTC, Some(1));
        let fonts = serde_json::json!({
            "faces": [
                { "family": "Pincel Test Serif", "style": "Regular", "weight": 400, "italic": false, "stretch": 5 },
                { "family": "Pincel Test Serif", "style": "Bold", "weight": 700, "italic": false, "stretch": 5 },
            ],
            "read": [{ "face": 1, "font": font }],
        })
        .to_string();
        let args = r#"{"text":"Ag","x":2,"y":2,"size":24,"font":"Pincel Test Serif","bold":true}"#;
        let handle = create_document(48, 32).expect("doc");
        let text = agent_call(handle, "tab-3", "draw.text", args, None, None, Some(fonts))
            .expect("handle");
        let r: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(r["ok"], true, "{r}");
        // The editor's Text tool with the same face.
        let mut editor = CommandBus::new(pincel_image_core::Document::new(48, 32).expect("doc"));
        let spec: pincel_image_core::command::spec::CommandSpec =
            serde_json::from_value(serde_json::json!({
                "type": "draw_text", "layer": 0, "font_id": 0, "text": "Ag", "x": 2, "y": 2,
                "size": 24, "color": [0, 0, 0, 255], "bold": false, "italic": false,
                "anti_alias": true, "align": "left"
            }))
            .expect("spec");
        pincel_image_core::command::spec::run_command(
            &mut editor,
            spec,
            &mut pincel_image_core::command::spec::StrokeScratch::default(),
            &[pincel_image_core::FontFace::new(SERIF_TTC, 1)],
        )
        .expect("runs");
        let drawn = doc(handle).layers()[0].pixels.data().to_vec();
        assert!(drawn.iter().any(|&v| v != 0), "text was drawn");
        assert_eq!(drawn, editor.document.layers()[0].pixels.data());
        close_document(handle);
    }
}
