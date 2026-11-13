//! History surface for the History panel (`docs/specs/history-panel.md`):
//! the kind of every undo / redo step and a multi-step jump.

use pincel_core::HistoryKind;
use wasm_bindgen::prelude::*;

use crate::Document;
use crate::events::Event;

/// Every history kind id the Pixel core reports, fixed kinds first, then
/// `effect.<name>` per effect (`docs/specs/history-panel.md` AC-15).
#[wasm_bindgen(js_name = historyKindIds)]
pub fn history_kind_ids() -> Vec<String> {
    HistoryKind::all().map(|k| k.id().into_owned()).collect()
}

#[wasm_bindgen]
impl Document {
    /// The kind ids of the undo steps, oldest first.
    #[wasm_bindgen(js_name = undoKinds)]
    pub fn undo_kinds(&self) -> Vec<String> {
        self.bus.undo_kinds().map(|k| k.id().into_owned()).collect()
    }

    /// The kind ids of the redo steps, in the order Redo restores them.
    #[wasm_bindgen(js_name = redoKinds)]
    pub fn redo_kinds(&self) -> Vec<String> {
        self.bus.redo_kinds().map(|k| k.id().into_owned()).collect()
    }

    /// Changes whenever the list of steps changes, so the UI re-reads
    /// [`Document::undo_kinds`] / [`Document::redo_kinds`] only then.
    #[wasm_bindgen(getter, js_name = historyRevision)]
    pub fn history_revision(&self) -> u32 {
        self.bus.revision()
    }

    /// `true` once the 100-step cap has dropped a step from the history.
    #[wasm_bindgen(getter, js_name = historyDropped)]
    pub fn history_dropped(&self) -> bool {
        self.bus.dropped_steps()
    }

    /// Undo (`delta < 0`) or redo (`delta > 0`) `|delta|` steps in one
    /// call, as that many `undo` / `redo` calls would, and queue one dirty
    /// event for the result. Returns the steps moved. When a redo fails the
    /// document stays at the last state reached, the failed step stays
    /// redoable, and the error is returned (`docs/specs/history-panel.md`
    /// AC-05, AC-12, AC-13).
    #[wasm_bindgen(js_name = jumpHistory)]
    pub fn jump_history(&mut self, delta: i32) -> Result<u32, String> {
        let result = self
            .bus
            .jump(i64::from(delta), &mut self.sprite, &mut self.cels);
        let moved = match &result {
            Ok(n) | Err((n, _)) => *n,
        };
        if moved > 0 {
            self.clamp_current_frame();
            if let Some(ev) = Event::from_dirty(self.bus.last_dirty_region()) {
                self.events.push(ev);
            }
        }
        result
            .map(|n| n as u32)
            .map_err(|(_, e)| format!("failed to redo: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn doc() -> Document {
        Document::new(8, 8).expect("dims")
    }

    #[test]
    fn undo_and_redo_kinds_name_tool_and_menu_steps() {
        let mut d = doc();
        d.apply_tool("pencil", 1, 1, 0xff00_00ff).expect("pencil");
        d.end_stroke();
        d.apply_tool("eraser", 1, 1, 0).expect("eraser");
        d.end_stroke();
        d.apply_tool("pencil", 2, 2, 0xff00_00ff).expect("pencil");
        d.end_stroke();
        d.set_selection(0, 0, 4, 4);
        d.cut_selection().expect("cut");
        d.apply_tool("pencil", 3, 3, 0x00ff_00ff).expect("pencil");
        d.end_stroke();
        assert!(d.apply_effect("invert", &[]).expect("effect"));
        d.set_selection(0, 0, 6, 6);
        assert!(d.crop_to_selection().expect("crop"));
        d.add_layer("Two").expect("layer");
        assert_eq!(
            d.undo_kinds(),
            [
                "pencil",
                "eraser",
                "pencil",
                "cut",
                "pencil",
                "effect.invert",
                "crop_to_selection",
                "add_layer"
            ]
        );
        assert!(d.undo());
        assert!(d.undo());
        assert_eq!(d.redo_kinds(), ["crop_to_selection", "add_layer"]);
    }

    /// Three separate pencil steps on a fresh document.
    fn three_dots() -> Document {
        let mut d = doc();
        for i in 0..3 {
            d.apply_tool("pencil", i, i, 0xff00_00ff).expect("pencil");
            d.end_stroke();
        }
        d
    }

    #[test]
    fn jump_history_equals_repeated_undo_and_queues_one_event() {
        let mut jumped = three_dots();
        let mut stepped = three_dots();
        jumped.drain_events();
        assert_eq!(jumped.jump_history(-2), Ok(2));
        assert!(stepped.undo());
        assert!(stepped.undo());
        assert_eq!(jumped.sprite, stepped.sprite);
        assert_eq!((jumped.undo_depth(), jumped.redo_depth()), (1, 2));
        let events = jumped.drain_events();
        assert_eq!(events.len(), 1, "one redraw for the whole jump");
        assert_eq!(events[0].kind(), "dirty-canvas");
        assert_eq!(jumped.jump_history(5), Ok(2), "clamped to the redo steps");
        assert_eq!(jumped.sprite, three_dots().sprite);
    }

    #[test]
    fn history_dropped_turns_true_when_the_cap_drops_a_step() {
        let mut d = doc();
        for i in 0..100 {
            d.apply_tool("pencil", i % 8, 0, 0xff00_00ff)
                .expect("pencil");
            d.end_stroke();
        }
        assert!(!d.history_dropped());
        d.apply_tool("pencil", 0, 1, 0xff00_00ff).expect("pencil");
        assert!(d.history_dropped());
        assert_eq!(d.undo_depth(), 100);
    }

    /// The `'key'` names a TypeScript dictionary file declares.
    fn keys_in(path: &str) -> BTreeSet<String> {
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

    /// Every kind the Pixel core reports has a name in every UI language
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
