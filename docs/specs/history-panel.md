---
status: todo
created: 2026-11-05
size: L
mode: both
design: [pincel.md §6.2, pincel.md §9.2, pincel.md §15 (2026-11-05 history entries carry a kind from the core)]
depends_on: [pixel-undo-history]
---

# History panel

## Context

Both editors have a linear undo history of 100 steps (`pixel-undo-history` AC-05; `pincel-image-core` `DEFAULT_UNDO_CAPACITY`), but the user only sees an undo / redo depth. paint.net shows a History window: a list of named steps, where a click jumps back or forward to any of them. The owner asked for the paint.net parity gap "History panel" (2026-11-05), with these choices: a sidebar panel in both modes, collapsed by default; step names from the core; behavior as in paint.net, with a start entry and Rewind / Undo / Redo / Fast-forward buttons.

Today neither core knows what a step was: `Bus` (Pixel) stores `AnyCommand`s and `UndoStack` (Image) stores `Box<dyn Command>`s, with no name. Decision Log 2026-11-05 ("History entries carry a kind from the core") decides that each step reports a kind id that the UI translates.

What counts as one step, what merges and what adds no step (selection in Pixel mode, eyedropper, view changes) stays as `pixel-undo-history` and `image-editing` AC-26 specify; this spec only shows those steps and lets the user move between them.

**Terms.** *Step*: one entry of the undo history, i.e. what one Undo reverts. *Start entry*: the first row of the list, standing for the document state before the oldest step still in the history. *Current state*: the step after which the document stands now (or the start entry when every step is undone). *Jump*: making a chosen row the current state through as many undos or redos as needed.

## Scope

### In scope

- A "History" panel (German "Verlauf") in the sidebar, drawer and bottom sheet of both modes, collapsed by default
- One row per step with a translated name, a start entry, undone steps shown dimmed
- A click or Enter on a row jumps there; Rewind, Undo, Redo and Fast-forward buttons
- A kind id per step from both cores, exposed through `pincel-wasm` and `pincel-image-wasm`
- en + de strings for the panel and every step kind

### Out of scope

- Branching history, or keeping undone steps after a new edit — because the history stays linear (pincel.md §6.2); a new edit discards the redo steps as before
- Thumbnails per step — because a thumbnail per step costs a compose per step and memory the undo budget (pincel.md §5, `performance-image-undo-memory`) does not have
- History that survives closing the document or the app — because pincel.md §6.2 lists it as Phase 2
- Deleting or clearing the history from the panel — because the owner did not ask for it and it destroys work that cannot be recovered
- Changing which actions are steps or how they merge — because `pixel-undo-history` and `image-editing` own those rules
- Showing which steps an agent made in live mode — because `mcp-live-desktop` owns live mode; an MCP call's step is named like the editor action it mirrors (AC-15)

## Acceptance criteria

- [ ] AC-01 (always): Pixel mode and Image mode both have a panel titled "History" in the sidebar (and in the drawer and the bottom sheet of the smaller layouts); on first use it is collapsed, and whether it is collapsed is remembered per mode like the other panels' state.
- [ ] AC-02 (always): The panel lists the start entry first, then one row per step from the oldest to the newest undoable step, then the redoable steps in the order Redo would restore them. The row of the current state is marked as selected; the redoable rows are dimmed.
- [ ] AC-03 (always): The start entry is named after how the document began: "New" for a document created in the app, "Open" for one opened from a file, "Paste" for one made by Paste as New Image, "Recovered" for one restored by autosave recovery, and "Start" for any other origin. Once any step has been dropped from the undo history (by the 100-step limit, or in Image mode by the undo memory budget of `performance-image-undo-memory`), it is named "Oldest available state".
- [ ] AC-04 (always): Every row names the action that made the step, in the UI language — for example "Pencil", "Brush", "Add Layer", "Merge Down", "Resize Canvas"; an effect or adjustment step is named after that effect ("Gaussian Blur"), as the effect menus name it. Every kind of step either mode can produce has its own name in English and German, so no row shows an empty name, a raw kind id, or a catch-all such as "Edit" that does not say which action it was.
- [ ] AC-05 (event): When the user clicks a row, or presses Enter or Space on a focused row, then the document becomes exactly the state after that step (the state before the oldest step for the start entry), as the same number of Undo or Redo presses would make it; that row becomes the current state, the rows after it are dimmed, and the undo / redo buttons and depths elsewhere in the UI match.
- [ ] AC-06 (event): When the user makes a new edit while rows are dimmed, then the dimmed rows disappear and the new step's row follows the current state's row, because a new edit after an Undo discards the redo steps in both modes (`pixel-undo-history` AC-03, `image-editing` AC-26).
- [ ] AC-07 (event): When the user presses the panel's Rewind, Undo, Redo or Fast-forward button, then the document jumps to the start entry, one step back, one step forward, or the newest step; each button is disabled while it cannot change anything (Rewind and Undo with nothing to undo, Redo and Fast-forward with nothing to redo).
- [ ] AC-08 (event): When a step is added, undone, redone or dropped in any way — a tool, a menu command, a shortcut, the header's undo / redo buttons, the memory budget — then the panel shows the new list and current state by the time the canvas shows the new document state, and the current state's row is scrolled into view; redo steps the memory budget drops disappear from the list.
- [ ] AC-09 (event): When the user switches to another document tab, then the panel shows that document's history; each open document keeps its own history.
- [ ] AC-10 (always): The step list is one keyboard tab stop: Arrow Up / Down move the focus between rows without jumping, Home / End to the first / last row, Enter or Space jump (AC-05). The list has the accessible role `listbox`, each row the role `option` with its name, and the current state's row `aria-selected="true"`.
- [ ] AC-11 (state): While an effect is open, the panel is inert like the other panels (`effect-panel` AC-03); while a stroke or another pointer gesture is in progress, or a jump is running, a click, Enter or Space on a row and the panel's buttons do nothing (the request is ignored, not queued). While a text entry or another uncommitted edit is open, a jump does to it first what Undo does in that state today.
- [ ] AC-12 (error): If an undo or redo fails during a jump, then the jump stops at the last state it reached, the panel marks that state as current, the failed step stays on the side it was on (undo or redo), the error shows where the mode shows errors (status bar in Pixel mode, notice in Image mode), and no step is lost.
- [ ] AC-13 (event): When a jump crosses several steps, then the canvas is redrawn once, with the target state, and shows no state in between.
- [ ] AC-14 (always): A jump adds no step and writes no file on disk; the document's dirty state and its autosave after a jump equal those after the same number of Undo or Redo presses.
- [ ] AC-15 (always): Each core reports, for every step in its undo and redo history, a kind id from a fixed set it documents, so that steps made without the UI (MCP calls, batches) are named too. A step that combines several commands (a merged stroke, a batch) reports one kind: the kind of the action that made it. An MCP tool call reports the kind of the editor action it mirrors (e.g. `layer.duplicate` → Duplicate Layer); a tool without an editor counterpart (e.g. `draw.pixels`) has a kind of its own.
- [ ] AC-16 (option): Where the UI language is German, the panel title ("Verlauf"), the start entry names, the four buttons' accessible names and every step name are German.
- [ ] AC-17 (error): If a step reports a kind the UI has no name for, then its row reads "Unknown step" followed by the kind id, and the panel keeps working; the AC-04 test keeps such a build from shipping.
- [ ] AC-18 (event): When a row has keyboard focus and the user presses Space, then the jump of AC-05 happens and the canvas does not start a Space-pan.

## Technical notes

Hints, not contracts. Grep for the names, because line numbers drift.

- Pixel: `crates/pincel-core/src/command/bus.rs` (`Bus { undo: VecDeque<AnyCommand>, redo: Vec<AnyCommand>, cap }`, `undo_depth` / `redo_depth`). A kind can come from the `AnyCommand` variant, with `CommandBatch` carrying the kind of what made it (e.g. a layer duplicate, an MCP call). wasm today: `undo_depth` / `redo_depth` in `crates/pincel-wasm/src/lib.rs`.
- Image: `crates/pincel-image-core/src/command/mod.rs` (`UndoStack { undo: Vec<Box<dyn Command>>, redo, capacity }`). The `CommandSpec` tags (`image-editing` Technical notes) are a natural kind set. Effect and adjustment steps do **not** carry their effect id today (`apply_effect` in `crates/pincel-image-wasm/src/lib.rs` pushes a plain pixel command); AC-04 needs the id attached in both modes, and the effect catalog already translates the ids.
- Kind ids are snake_case strings in a fixed list per core; the UI maps each to an i18n key (`history.<kind>`), so a test can check that every kind has an `en` and `de` string (AC-04, AC-16).
- A jump of n steps is n undos or redos in one call, followed by one render (AC-13). A dedicated wasm call (e.g. `jumpHistory(delta)`) avoids n round trips and n recomposes.
- The panel joins each mode's `PanelDef` list (`PixelEditor.svelte`, `ImageEditor.svelte`) and renders through `Panel` in `EditorFrame`; collapsed state persists through the existing `FrameLayout` prefs (IndexedDB).
- E2e: `ui/e2e/` with `createDocument` / `waitForEditor` from `helpers.ts`. Image mode's Deselect is an undo step (AGENTS.md Gotchas), so count rows after it.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| New document, nothing done | One row: the start entry, selected; all four buttons disabled |
| 101st step | The oldest step drops; the start entry is renamed "Oldest available state" (AC-03); the list still has 101 rows (fewer in Image mode when the memory budget dropped more) |
| Click on the row that is already current | Nothing changes |
| Jump back 50 steps on a 4096 × 4096 Image document | One jump, one render at the end (AC-13) |
| Document saved, then a jump back past the save | Allowed (`pixel-undo-history` AC-21); the document becomes dirty as after the same undos |
| A redo row clicked after the document was changed by an autosave recovery | Autosave recovery opens a new document with an empty history (`pixel-undo-history` AC-20); there are no redo rows |
| Selection change in Pixel mode | No row (selection is not a step there); in Image mode it is a step and has its row ("Select", "Deselect", …) |
| Panel collapsed while edits happen | The list is up to date when the panel is expanded |
| Step kinds added later by another feature | That feature adds the kind's name in `en` and `de`; the AC-04 test fails until it does |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Collapsed by default, state remembered per mode | The owner's choice; collapsed it takes one header row, so the panels above it keep their room | Low |
| A merged stroke is one row named after its tool | It is one step for Undo (`pixel-undo-history` AC-06), so the panel shows what Undo would revert | Low |
| No step thumbnails or timestamps | Not asked for; thumbnails cost memory (Out of scope) | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/history-panel.spec.ts` › both modes have a History panel, collapsed at first, its state kept per mode; › phone › the History panel is a tab of the bottom sheet; › tablet › the History panel is in the drawer | built |  |
| AC-02 | `ui/e2e/history-panel.spec.ts` › rows list the start entry, the undo steps and the dimmed redo steps …; `ui/e2e/history-panel-image.spec.ts` › Image steps are named after their tools and menu commands …; `crates/pincel-core/src/command/bus.rs` `kinds_list_undo_oldest_first_and_redo_in_redo_order`; `crates/pincel-image-core/src/command/history.rs` `kinds_list_undo_oldest_first_and_redo_in_redo_order` | built |  |
| AC-03 | `ui/e2e/history-panel-image.spec.ts` › pixel / image: the start entry names a new and an opened document; › clipboard › … Paste as New Image is "Paste"; › … a recovered document starts with "Recovered"; › … once the 100-step limit drops a step …; `ui/e2e/history-names.spec.ts` › the start entry is named after how the document began …; `bus.rs` `dropped_steps_is_set_once_the_cap_drops_the_oldest`; image `history.rs` `dropped_steps_is_set_when_the_capacity_or_the_byte_cap_drops_a_step`; `crates/pincel-wasm/src/history.rs` `history_dropped_turns_true_when_the_cap_drops_a_step` | built |  |
| AC-04 | `crates/pincel-wasm/src/history.rs` and `crates/pincel-image-wasm/src/history.rs` `every_history_kind_has_an_english_and_a_german_name`; `ui/e2e/history-panel.spec.ts` › an effect step is named like the effect menu names it; › rows list the start entry …; `ui/e2e/history-panel-image.spec.ts` › Image steps are named …; `ui/e2e/history-names.spec.ts` › an effect step is named as the effect menu names it | built |  |
| AC-05 | `ui/e2e/history-panel.spec.ts` › a click, Enter or Space on a row jumps there as that many Undo / Redo presses would; `ui/e2e/history-panel-image.spec.ts` › Image steps are named … a row click jumps; `bus.rs` `jump_matches_the_same_number_of_undos_and_redos`; image `history.rs` `jump_matches_the_same_number_of_undos_and_redos`; `crates/pincel-wasm/src/history.rs` `jump_history_equals_repeated_undo_and_queues_one_event` | built |  |
| AC-06 | `ui/e2e/history-panel.spec.ts` › rows list the start entry … a new edit drops the redo rows; `ui/e2e/history-panel-image.spec.ts` › Image steps are named … (Select All after a jump back) | built |  |
| AC-07 | `ui/e2e/history-panel.spec.ts` › Rewind, Undo, Redo and Fast-forward jump and are disabled when they cannot change anything | built |  |
| AC-08 | `ui/e2e/history-panel.spec.ts` › the panel follows steps from tools, menus, shortcuts and the header buttons, with the current row in view; `bus.rs` `revision_moves_with_the_step_list_but_not_with_a_merge` | built |  |
| AC-09 | `ui/e2e/history-panel.spec.ts` › each document tab keeps its own history; `ui/e2e/history-panel-image.spec.ts` › Image: each document tab keeps its own history | built |  |
| AC-10 | `ui/e2e/history-panel.spec.ts` › the list is one tab stop with listbox / option roles; arrows, Home and End move the focus without jumping | built |  |
| AC-11 | `ui/e2e/history-panel.spec.ts` › requests are ignored while a stroke is in progress and the panel is inert while an effect is open; `ui/e2e/history-panel-image.spec.ts` › Image: a click mid-stroke is ignored | built |  |
| AC-12 | `ui/e2e/history-panel.spec.ts` › a jump that fails stops at the last state reached and reports in the status bar; `bus.rs` `jump_stops_at_a_failing_redo_and_keeps_the_failed_step`; image `history.rs` `jump_stops_at_a_failing_step_and_keeps_it` | built |  |
| AC-13 | `ui/e2e/history-panel.spec.ts` › a jump of many steps redraws the canvas once; `bus.rs` `jump_of_several_steps_reports_the_whole_canvas_dirty`; `crates/pincel-wasm/src/history.rs` `jump_history_equals_repeated_undo_and_queues_one_event` | built |  |
| AC-14 | `ui/e2e/history-panel.spec.ts` › a jump writes no file and leaves the dirty state of as many undos; › a jump autosaves as the same undos and redos would | built |  |
| AC-15 | `crates/pincel-agent/src/tests/history.rs` `pixel_calls_name_their_step_after_the_editor_action`, `image_calls_name_their_step_after_the_editor_action`; `bus.rs` `default_kinds_name_the_tool_or_menu_command`, `execute_as_names_the_step_and_a_merge_keeps_the_top_kind`; `crates/pincel-wasm/src/history.rs` `undo_and_redo_kinds_name_tool_and_menu_steps`; `crates/pincel-image-wasm/src/history.rs` `kinds_and_jump_follow_the_documents_history`; image `history.rs` `a_pixel_edit_reports_the_kind_it_was_given`; both `history.rs` `all_ids_are_unique_snake_case` | built |  |
| AC-16 | `ui/e2e/history-panel.spec.ts` › in German, title, start entry, buttons and step names are German; both wasm `every_history_kind_has_an_english_and_a_german_name` | built |  |
| AC-17 | `ui/e2e/history-names.spec.ts` › a kind the UI has no name for reads "Unknown step" and its id; both wasm `every_history_kind_has_an_english_and_a_german_name` (the build-blocking half) | built |  |
| AC-18 | `ui/e2e/history-panel.spec.ts` › Space on a focused row jumps and starts no Space-pan | built |  |

## Decisions

- 2026-11-05: Sidebar panel in both modes, collapsed by default — owner's choice, because a collapsed panel costs one header row and the list is consulted, not watched.
- 2026-11-05: Step names come from the cores as kind ids that the UI translates (Decision Log 2026-11-05) — owner's choice, because a name given by the UI would leave steps from paths without UI (MCP, recovery) unnamed and could drift from what the command really did.
- 2026-11-05: paint.net behavior: a start entry, dimmed redo rows, Rewind / Undo / Redo / Fast-forward — owner's choice, because users switching from paint.net expect it.
- 2026-11-06: Pixel kinds live in `pincel-core` `HistoryKind`, Image kinds in `pincel-image-core` `HistoryKind` (the Image `Command::label` English text became `Command::kind`); effects are `effect.<name>` over `pincel_effects::EFFECT_NAMES` — one fixed, documented list per core, as the Decision Log entry asks, and the effect menu's existing `effect.<name>` strings name them (AC-04).
- 2026-11-06: `Bus::execute` derives a kind from the command; `Bus::execute_as` / `Doc::commit_as` / `ImageDoc::apply_as` name it where one command type serves several actions (effects, Cut, Crop to Selection, MCP draw tools, frame duplicate) — the smallest change that names every step without touching every caller.
- 2026-11-06: MCP `layer.set` / `tag.set` that change more than one property at once are one step named "Layer Properties" / "Tag Properties" (new kinds), because no single editor action changes several at once; a single property keeps that property's name.
- 2026-11-06: Image `move_layer` (stack order) reads "Reorder Layer", the Move tool's `translate_layer` reads "Move Layer", so the two never share a name.
- 2026-11-06: `Bus::revision` (wasm `historyRevision`) tells Pixel mode when to re-read the kinds, so the per-frame `syncMeta` does not copy 100 strings during a stroke while the list cannot change (AC-08); Image mode re-reads in `syncInfo`, which runs after every command.
- 2026-11-06: A jump keeps a step that fails on its side in both cores (Pixel `Bus::redo` now always does; Image only in `CommandBus::jump`, whose plain `undo` / `redo` keep dropping a failed step as `performance-image-undo-memory` built it).
- 2026-11-06: `OpenedFile.origin = 'paste'` (set by `pastedImageFile`) carries Paste as New Image to both editors' open paths; every other way into a tab sets its origin there (New, Open / recent / drop, Recovered).
- 2026-11-06: AC-11's "uncommitted edit" needs no extra code: Pixel text is not in the history until committed and Undo does a plain undo then, Image live-object edits are history steps the jump moves through like Undo, and the one special Undo path (dropping a live-object handle drag) belongs to a pointer gesture, during which the panel ignores requests. A jump is synchronous, so "a jump is running" is guarded by a re-entry flag no test can reach.
- 2026-11-06: AC-12's Image notice has no e2e test: no Image step fails to redo through the UI (selections are steps there); the core's keep-and-stop is tested in `pincel-image-core`, the Pixel status-bar path end to end.
- 2026-11-06: Red runs (temporary breaks, then reverted): core — redo dropping a failed step, no `dropped` flag, no dirty widening, unreversed redo kinds, `execute_as` ignoring its kind, a one-step jump, Eraser read as Pencil; Image core — same plus a failed step not pushed back and `with_kind` ignored; wasm — Cut / effect / crop / eraser kinds replaced by the defaults, a dropped flag stuck at false, a one-step jump, a missing `history-de.ts` name; agent — `commit_as` / `apply_as` ignoring the kind; revision not bumped by undo. UI batch A (panel expanded by default, buttons never disabled, busy ignored, no status message, jump marking dirty, English title in German, Space not consumed, raw ids for unknown kinds, no scroll into view) turned AC-01, 07, 08, 10, 11 (both), 12, 14, 16, 17, 18 red; batch B (no dimming, origins / dropped ignored, first tab's history everywhere, one redraw per step, arrows jumping) turned AC-02, 03 (all eight), 09 (both), 10, 13 red; batch C (one step per jump, effect kinds not mapped to the effect menu) turned AC-04 (three tests) and AC-05 red. The AC-14 autosave test stayed green under a jump that marked the document dirty: autosave keys off the core's undo depth, which only the jump itself moves, so it is a regression guard the jump cannot break.
