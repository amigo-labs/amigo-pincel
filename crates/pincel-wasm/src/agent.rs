//! Live agent calls (docs/specs/mcp-live-desktop.md): the desktop app runs
//! an MCP client's tool call on the document open in the editor, through
//! the same `pincel-agent` tools as the headless server.

use pincel_agent::live::{LiveFile, PixelLoan, WireFonts, call_pixel_wire};
use pincel_core::HistoryKind;
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

#[wasm_bindgen]
impl Document {
    /// Run the agent tool `name` with `args_json` (a JSON object, or empty
    /// for none) on this document, which the reply calls `doc_id`. A
    /// mutating call is exactly one undo step. `file_path` / `file_bytes`
    /// carry the file the user picked for a reading tool (`layer.import`,
    /// `palette.import`). `fonts_json` is the editor's font list with the
    /// faces read for this call, by their [`register_font`] ids
    /// (`pincel_agent::live::WireFonts`, docs/specs/system-fonts-mcp.md
    /// AC-11); without it `draw.text` knows the bundled face only. Returns
    /// the JSON reply of `pincel_agent::live::LiveReply::to_json`; the files
    /// a writing tool produced are in it for the app to save where the user
    /// confirms.
    ///
    /// [`register_font`]: crate::register_font
    #[wasm_bindgen(js_name = agentCall)]
    pub fn agent_call(
        &mut self,
        doc_id: &str,
        name: &str,
        args_json: &str,
        file_path: Option<String>,
        file_bytes: Option<Vec<u8>>,
        fonts_json: Option<String>,
    ) -> String {
        let depth = (self.bus.undo_depth(), self.bus.redo_depth());
        let input = match (&file_path, &file_bytes) {
            (Some(path), Some(bytes)) => Some(LiveFile { path, bytes }),
            _ => None,
        };
        let reply = call_pixel_wire(
            doc_id,
            PixelLoan {
                sprite: &mut self.sprite,
                cels: &mut self.cels,
                bus: &mut self.bus,
                active_layer: &mut self.active_layer,
                active_frame: &mut self.current_frame,
            },
            input,
            fonts_json.as_deref().map(|json| WireFonts {
                json,
                registered: &crate::text::font_face,
            }),
            name,
            args_json,
        );
        self.clamp_current_frame();
        if (self.bus.undo_depth(), self.bus.redo_depth()) != depth {
            // The tools do not report which cels they touched.
            self.events.push(Event::dirty_canvas());
        }
        reply
    }

    /// An Assistant answer starts: the steps until
    /// [`Document::assistant_end`] become one undo step
    /// (docs/specs/ai-chat-panel.md AC-08).
    #[wasm_bindgen(js_name = assistantBegin)]
    pub fn assistant_begin(&mut self) {
        self.bus.begin_group();
    }

    /// The Assistant answer ended: its changes become one undo step of kind
    /// `assistant`. Returns `false`, adding no step, when it changed
    /// nothing (AC-09).
    #[wasm_bindgen(js_name = assistantEnd)]
    pub fn assistant_end(&mut self) -> bool {
        self.bus.end_group(HistoryKind::Assistant)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(doc: &mut Document, name: &str, args: &str) -> serde_json::Value {
        serde_json::from_str(&doc.agent_call("tab-1", name, args, None, None, None)).expect("json")
    }

    #[test]
    fn agent_call_draw_is_one_undo_step_and_queues_a_redraw() {
        let mut doc = Document::new(8, 8).expect("doc");
        let before = doc.compose(0, 1).expect("compose").pixels();
        let r = reply(
            &mut doc,
            "draw.rect",
            r##"{"x0":1,"y0":1,"x1":4,"y1":4,"color":"#ff0000","filled":true}"##,
        );
        assert_eq!(r["ok"], true, "{r}");
        assert_ne!(doc.compose(0, 1).expect("compose").pixels(), before);
        assert_eq!(doc.undo_depth(), 1);
        assert!(
            doc.drain_events()
                .iter()
                .any(|e| e.kind() == "dirty-canvas")
        );
        assert!(doc.undo());
        assert_eq!(doc.compose(0, 1).expect("compose").pixels(), before);
    }

    #[test]
    fn assistant_answer_of_several_calls_is_one_assistant_step() {
        let mut doc = Document::new(8, 8).expect("doc");
        let before = doc.compose(0, 1).expect("compose").pixels();
        doc.assistant_begin();
        for (x, color) in [(1, "#ff0000"), (4, "#0000ff")] {
            let args = format!(
                r##"{{"x0":{x},"y0":1,"x1":{},"y1":3,"color":"{color}","filled":true}}"##,
                x + 1
            );
            assert_eq!(reply(&mut doc, "draw.rect", &args)["ok"], true);
        }
        assert!(doc.assistant_end());
        assert_eq!(doc.undo_kinds(), ["assistant"]);
        let after = doc.compose(0, 1).expect("compose").pixels();
        assert!(doc.undo());
        assert_eq!(doc.compose(0, 1).expect("compose").pixels(), before);
        assert!(doc.redo().expect("redo"));
        assert_eq!(doc.compose(0, 1).expect("compose").pixels(), after);
        doc.assistant_begin();
        assert_eq!(reply(&mut doc, "doc.info", "")["ok"], true);
        assert!(
            !doc.assistant_end(),
            "an answer without a change adds no step"
        );
        assert_eq!(doc.undo_depth(), 1);
    }

    #[test]
    fn agent_call_failure_changes_nothing() {
        let mut doc = Document::new(4, 4).expect("doc");
        let r = reply(&mut doc, "doc.save", "{}");
        assert_eq!(r["ok"], false);
        let r = reply(&mut doc, "draw.pixels", r#"{"pixels":"x"}"#);
        assert_eq!(r["ok"], false);
        assert_eq!(doc.undo_depth(), 0);
        assert!(doc.drain_events().is_empty());
    }

    #[test]
    fn agent_call_reads_the_picked_file() {
        let mut doc = Document::new(4, 4).expect("doc");
        let gpl = b"GIMP Palette\n10 20 30 Dark\n200 100 0\n".to_vec();
        let r: serde_json::Value = serde_json::from_str(&doc.agent_call(
            "tab-1",
            "palette.import",
            r#"{"path":"p.gpl"}"#,
            Some("p.gpl".to_owned()),
            Some(gpl),
            None,
        ))
        .expect("json");
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(doc.undo_depth(), 1);
    }

    const SERIF_TTC: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pincel-core/tests/fixtures/PincelTestSerif.ttc"
    ));

    /// The editor's font list with Pincel Test Serif, whose Bold (face 1 of
    /// the collection) the webview registered as `font` (or failed to read).
    fn serif_fonts(read: serde_json::Value) -> String {
        serde_json::json!({
            "faces": [
                { "family": "Pincel Test Serif", "style": "Regular", "weight": 400, "italic": false, "stretch": 5 },
                { "family": "Pincel Test Serif", "style": "Bold", "weight": 700, "italic": false, "stretch": 5 },
            ],
            "read": read,
        })
        .to_string()
    }

    #[test]
    fn agent_call_draws_text_with_a_face_the_editor_registered() {
        let font = crate::text::register_font(SERIF_TTC, Some(1));
        let args = r#"{"text":"Ag","x":1,"y":1,"size":16,"font":"Pincel Test Serif","bold":true}"#;
        let mut doc = Document::new(40, 24).expect("doc");
        let fonts = serif_fonts(serde_json::json!([{ "face": 1, "font": font }]));
        let r: serde_json::Value = serde_json::from_str(&doc.agent_call(
            "tab-1",
            "draw.text",
            args,
            None,
            None,
            Some(fonts),
        ))
        .expect("json");
        assert_eq!(r["ok"], true, "{r}");
        let drawn = doc.compose(0, 1).expect("compose").pixels();
        // The editor's Text dialog with the same face.
        let mut editor = Document::new(40, 24).expect("doc");
        editor
            .draw_text(
                font,
                "Ag",
                1,
                1,
                16.0,
                0x0000_00ff,
                false,
                false,
                false,
                "left",
            )
            .expect("draw");
        assert_eq!(drawn, editor.compose(0, 1).expect("compose").pixels());
        // Without the editor's fonts the family is unknown.
        let mut other = Document::new(40, 24).expect("doc");
        let r = reply(&mut other, "draw.text", args);
        assert_eq!(r["ok"], false);
        assert_eq!(other.undo_depth(), 0);
    }
}
