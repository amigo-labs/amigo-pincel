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
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |  | open |  |
| AC-04 |  | open |  |
| AC-05 |      | open   |        |
| AC-06 |  | open |  |
| AC-07 |      | open   |        |
| AC-08 |  | open |  |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |  | open |  |
| AC-12 |  | open |  |
| AC-13 |  | open |  |
| AC-14 |  | open |  |
| AC-15 |  | open |  |
| AC-16 |      | open   |        |
| AC-17 |  | open |  |
| AC-18 |  | open |  |

## Decisions

- 2026-11-05: Sidebar panel in both modes, collapsed by default — owner's choice, because a collapsed panel costs one header row and the list is consulted, not watched.
- 2026-11-05: Step names come from the cores as kind ids that the UI translates (Decision Log 2026-11-05) — owner's choice, because a name given by the UI would leave steps from paths without UI (MCP, recovery) unnamed and could drift from what the command really did.
- 2026-11-05: paint.net behavior: a start entry, dimmed redo rows, Rewind / Undo / Redo / Fast-forward — owner's choice, because users switching from paint.net expect it.
