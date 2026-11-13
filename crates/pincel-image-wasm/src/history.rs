//! History surface for the History panel (`docs/specs/history-panel.md`):
//! the kind of every undo / redo step and a multi-step jump.

use pincel_image_core::command::{CommandBus, HistoryKind};
use wasm_bindgen::prelude::*;

use crate::{to_js, with_bus};

/// Every history kind id the Image core reports, fixed kinds first, then
/// `effect.<name>` per effect (`docs/specs/history-panel.md` AC-15).
#[wasm_bindgen]
pub fn history_kind_ids() -> Vec<String> {
    HistoryKind::all().map(|k| k.id().into_owned()).collect()
}

/// The kind ids of the undo steps (oldest first) and the redo steps (in the
/// order Redo restores them).
fn kinds(bus: &CommandBus) -> (Vec<String>, Vec<String>) {
    let ids = |k: HistoryKind| k.id().into_owned();
    (
        bus.history.undo_kinds().map(ids).collect(),
        bus.history.redo_kinds().map(ids).collect(),
    )
}

/// The kind ids of the document's undo steps, oldest first.
#[wasm_bindgen]
pub fn undo_kinds(handle: u32) -> Result<Vec<String>, JsError> {
    with_bus(handle, |bus| Ok(kinds(bus).0))
}

/// The kind ids of the document's redo steps, in the order Redo restores
/// them.
#[wasm_bindgen]
pub fn redo_kinds(handle: u32) -> Result<Vec<String>, JsError> {
    with_bus(handle, |bus| Ok(kinds(bus).1))
}

/// `true` once the step limit or the undo memory budget has dropped a step
/// from the document's history.
#[wasm_bindgen]
pub fn history_dropped(handle: u32) -> Result<bool, JsError> {
    with_bus(handle, |bus| Ok(bus.history.dropped_steps()))
}

/// Undoes (`delta < 0`) or redoes (`delta > 0`) `|delta|` steps in one call,
/// as that many `undo` / `redo` calls would. Returns the steps moved. When a
/// step fails the document stays at the last state reached, the failed
/// step stays where it was, and the error is returned
/// (`docs/specs/history-panel.md` AC-05, AC-12, AC-13).
#[wasm_bindgen]
pub fn jump_history(handle: u32, delta: i32) -> Result<u32, JsError> {
    with_bus(handle, |bus| {
        bus.jump(i64::from(delta))
            .map(|n| n as u32)
            .map_err(|(_, e)| to_js(e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_document, undo};

    fn run(handle: u32, json: &str) {
        crate::apply_command(handle, json).expect("command");
    }

    #[test]
    fn kinds_and_jump_follow_the_documents_history() {
        let h = create_document(8, 8).expect("doc");
        run(h, r#"{"type":"add_layer","active":0}"#);
        run(h, r#"{"type":"select_all"}"#);
        run(h, r#"{"type":"flip_canvas","horizontal":true}"#);
        assert!(crate::apply_effect(h, 1, "invert", &[]).expect("effect"));
        assert_eq!(
            undo_kinds(h).expect("kinds"),
            [
                "add_layer",
                "select_all",
                "flip_canvas_horizontal",
                "effect.invert"
            ]
        );
        assert_eq!(jump_history(h, -3).expect("jump"), 3);
        assert_eq!(undo_kinds(h).expect("kinds"), ["add_layer"]);
        assert_eq!(
            redo_kinds(h).expect("kinds"),
            ["select_all", "flip_canvas_horizontal", "effect.invert"]
        );
        assert!(undo(h).expect("undo"));
        assert!(!history_dropped(h).expect("dropped"));
    }

    /// The `'key'` names a TypeScript dictionary file declares.
    fn keys_in(path: &str) -> std::collections::BTreeSet<String> {
        let full = format!(
            "{}/../../ui/src/lib/i18n/{path}",
            env!("CARGO_MANIFEST_DIR")
        );
        let text = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{full}: {e}"));
        text.lines()
            .filter_map(|l| l.trim().strip_prefix('\''))
            .filter_map(|l| l.split_once("':").map(|(k, _)| k.to_string()))
            .collect()
    }

    /// Every kind the Image core reports has a name in every UI language
    /// (`docs/specs/history-panel.md` AC-04, AC-16, AC-17): `history.<id>`
    /// in the history dictionaries, the effect menu's `effect.<name>` for
    /// an effect.
    #[test]
    fn every_history_kind_has_a_name_in_every_language() {
        for (history, main) in [
            ("history-en.ts", "en.ts"),
            ("history-de.ts", "de.ts"),
            ("history-es.ts", "es.ts"),
            ("history-ja.ts", "ja.ts"),
            ("history-fr.ts", "fr.ts"),
            ("history-pt-br.ts", "pt-br.ts"),
        ] {
            let history_keys = keys_in(history);
            let main_keys = keys_in(main);
            for id in history_kind_ids() {
                let named = match id.strip_prefix("effect.") {
                    Some(_) => main_keys.contains(&id),
                    None => history_keys.contains(&format!("history.{id}")),
                };
                assert!(named, "{history}: no name for history kind `{id}`");
            }
        }
    }
}
