//! Live agent calls (docs/specs/mcp-live-desktop.md): the desktop app runs
//! an MCP client's tool call on the document open in the editor, through
//! the same `pincel-agent` tools as the headless server.

use pincel_agent::live::{LiveFile, PixelLoan, call_pixel_wire};
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

#[wasm_bindgen]
impl Document {
    /// Run the agent tool `name` with `args_json` (a JSON object, or empty
    /// for none) on this document, which the reply calls `doc_id`. A
    /// mutating call is exactly one undo step. `file_path` / `file_bytes`
    /// carry the file the user picked for a reading tool (`layer.import`,
    /// `palette.import`). Returns the JSON reply of
    /// `pincel_agent::live::LiveReply::to_json`; the files a writing tool
    /// produced are in it for the app to save where the user confirms.
    #[wasm_bindgen(js_name = agentCall)]
    pub fn agent_call(
        &mut self,
        doc_id: &str,
        name: &str,
        args_json: &str,
        file_path: Option<String>,
        file_bytes: Option<Vec<u8>>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(doc: &mut Document, name: &str, args: &str) -> serde_json::Value {
        serde_json::from_str(&doc.agent_call("tab-1", name, args, None, None)).expect("json")
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
        ))
        .expect("json");
        assert_eq!(r["ok"], true, "{r}");
        assert_eq!(doc.undo_depth(), 1);
    }
}
