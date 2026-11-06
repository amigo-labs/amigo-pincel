---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6.1, pincel.md §6.2, pincel.md §9.2, pincel.md §9.3, pincel.md §9.4, pincel.md §15 2026-09-06 ab_glyph, pincel.md §15 2026-11-05 MCP tool parity]
depends_on: [image-live-objects]
---

# Image mode live text

## Context

paint.net keeps text editable until it is finished: the text sits on the canvas with a caret, can be edited, selected, restyled and moved by a handle, and becomes plain pixels only when it is committed. Pincel's Image-mode Text tool (pincel.md §5.4, "Rasterised on commit") has only part of this today:

- A press with the Text tool opens a floating `<textarea>` at the point (`placeText` in `ui/src/modes/image/components/canvas/MainCanvas.svelte`); `commitText` sends one `draw_text` command (`renderText` in `ui/src/modes/image/core/controller.ts`), rasterised by `crates/pincel-image-core/src/tools/text.rs` with the bundled Liberation Sans (Decision Log 2026-09-06, faux bold / italic).
- What the user sees while typing is the browser's CSS rendering of the textarea, not the pixels the commit paints: faux bold and italic, anti-alias off and the exact line height differ.
- The textarea commits on `blur`, so clicking the options bar (size, bold, italic, alignment, anti-alias) or a color control commits the text before the change can apply to it. Options therefore never apply to the text being typed.
- Enter inserts a new line; Ctrl+Enter / ⌘Enter **and Escape** commit. There is no cancel, no move handle, no underline or strikethrough.

This spec turns the text entry into a **live text**, a live object in the sense of `image-live-objects`, and follows that spec's live-object architecture (owner's choice, 2026-11-05): the core draws the live object into its layer at every change, its editable settings live beside the document on the `CommandBus` (not in the `.ora` file), every edit is a normal undo step, a commit folds the live object's edit steps into one undo step, a cancel removes them, and any other command or a change of the active layer commits it first. What this spec adds is text-specific: caret, in-text selection, keyboard editing, the move handle, the text options, the new underline and strikethrough styles, and a text-layout query in `pincel-image-wasm` for caret and hit test.

**Escape changes meaning:** today Escape commits the text; with this spec Escape **cancels** the live text, as it cancels every live object (owner's choice). Ctrl+Enter / ⌘Enter commits, Enter inserts a line break. Committed text stays pixels (Decision Log 2026-09-06): there is no text layer.

## Scope

### In scope

- A live text opened by a press of the Text tool; caret, typing, multi-line text (Enter = line break), Backspace / Delete, arrow keys, Home / End, Shift-extended and pointer-dragged selection of characters inside the text, Ctrl+A / ⌘A inside the text, plain-text copy / cut / paste inside the text
- The live text drawn into its layer by the core at every change, with the dashed text box, the caret and the selection highlight shown over it
- A move handle that drags the live text to another position
- Size, bold, italic, underline, strikethrough, alignment, anti-alias and the foreground color applying to the live text while it is open
- Each edit of the live text as an undo step; commit folding them into one; Escape cancelling them
- Underline and strikethrough as new text styles in the core text tool, the `draw_text` command and the MCP `draw.text` tool
- A text-layout query in `pincel-image-wasm` (line boxes, caret positions) that the caret and the hit test use
- Commit triggers (Ctrl+Enter, the options bar's Finish button, tool change, a press outside the box, any other command, an active-layer change, tab switch / close, save / export)
- Selection clipping, the locked-layer refusal and the empty / off-canvas no-op carried over to the live text

### Out of scope

- Choosing a font family — because `system-fonts` is a separate spec; this spec keeps the font as one setting of the live text, like size, so that spec can add a family picker that applies to the live text the same way
- Editing text again after it is committed — because committed text is pixels, not a vector or text layer (Decision Log 2026-09-06); a text layer would be a document-model and file-format change
- Saving a live text's editability in the `.ora` project or the autosave snapshot — because the live settings live beside the document on the `CommandBus`, not in the file (owner's choice for all live objects); Save commits first, and an autosave written while a live text is open stores its pixels only
- Formatting individual characters (rich text: a bold word inside regular text) — because paint.net's Text tool also styles the whole text at once, and per-run styles would need a different command shape
- Rotating or scaling the live text by handles — because paint.net's Text tool has no such handles either; the layer rotate / scale commands cover it after commit
- Nudging the live text with the arrow keys — because the arrow keys move the caret while the text has focus; the handle drag is the way to move it
- A per-tool blend mode or rendering mode (paint.net's blend mode, "Clear type") — because no Image-mode tool has a blend mode option today; it would be a cross-tool change of its own
- Underline and strikethrough in Pixel mode — because Pixel mode's Text tool runs on `pincel-core`'s separate `DrawText` and its own dialog (Decision Log 2026-09-06 "Two cores, one shell"); adding them there is a separate change

## Acceptance criteria

- [ ] AC-01 (event): When the Text tool is pressed with the primary button on the canvas or the stage and no live text is open, then an empty dashed text box with a caret opens with its top-left corner at the pressed point (for every alignment) and takes keyboard focus; no live object is begun, the layer is unchanged and no undo step is added. The first typed or pasted character begins the live text as one undo step; while the box is still empty, option changes only change the tool options, and Escape, Undo or any commit trigger of AC-11 just closes the box (Undo then undoes the step before as usual).
- [ ] AC-02 (state): While a live text is open, the layer it was opened on holds its text with its current settings at its current position, pixel for pixel as the commit leaves it — the canvas, the layer thumbnail and the composite show it — and after every edit the layer equals the layer as it was before the live text began with that text drawn onto it.
- [ ] AC-03 (event): When a character key is typed in a live text, then the character is inserted at the caret (replacing the selected characters, if any) and the caret moves after it; Enter inserts a line break; Backspace removes the selected characters or else the character before the caret; Delete removes the selected characters or else the character after it. Consecutive insertions and deletions with no caret jump (arrow key, Home / End, pointer press), option change or move between them form one undo step. A live text whose characters are all deleted stays open with its steps.
- [ ] AC-04 (event): When Left / Right / Up / Down / Home / End is pressed in a live text, then the caret moves one character, one line, or to the start / end of the line, and with Shift held the selection extends from its anchor to the new caret position instead; Ctrl+A / ⌘A selects all characters of the live text and leaves the canvas selection unchanged.
- [ ] AC-05 (state): While a live text is open, the caret is shown at the position the text-layout query (AC-22) returns for its character boundary, within 1 document pixel, on every line and for every alignment, bold and italic setting, and the selected characters are highlighted.
- [ ] AC-06 (event): When the user presses inside the live text's hit area — its box (at least half the font size wide and one line high, even when empty) plus 4 CSS px around it, minus the move handle — then the caret goes to the character boundary nearest the press; when the user drags from there, then the characters between the press and the pointer are selected; neither commits the text, opens a new one or adds an undo step.
- [ ] AC-07 (event): When the user drags the live text's move handle, then the text moves by the drag distance rounded to whole document pixels, keeps its content, caret and selection, stays editable, and the drag adds one undo step; when Escape is pressed during that drag, then the drag is undone and the text stays live where it was before the drag.
- [ ] AC-08 (event): When the font size, bold, italic, underline, strikethrough, alignment, anti-alias or the foreground color changes while a live text with characters is open, then the live text is redrawn with the new value, the text is not committed, and keyboard focus returns to the live text with its caret and selection kept. Each change is one undo step, except that consecutive changes of the same option with no other step between them form one step, and moving a slider without letting go is one step.
- [ ] AC-09 (state): While a live text is open, its box keeps its left edge and top through every edit (typing, deleting, option changes); the box is as wide as the widest line, and the lines are aligned within it — left edges, centers or right edges of the lines line up for Left, Center and Right.
- [ ] AC-10 (option): Where underline is on, each line of the text has a solid stroke in the text color below its baseline, running from the line's left edge to the advance end of its last non-whitespace character; where strikethrough is on, each line has such a stroke through the middle of its lowercase letters, over the same span. A line of only whitespace gets neither. Both can be on together and combine with bold, italic, every alignment and anti-alias on or off.
- [ ] AC-11 (event): When a live text with visible characters is committed — by Ctrl+Enter / ⌘Enter, the options bar's Finish button, choosing another tool, a Text-tool press on the canvas or stage outside its hit area, any other document command (menu item, shortcut, layers panel, selection action, paste onto the canvas, an effect or adjustment, an MCP tool call), Edit ▸ Copy / Cut / Copy Merged, a change of the active layer, Save, Save As or Export, switching to another tab, or closing its tab — then its steps are replaced by one undo step that leaves the layer exactly as shown, the live text closes, and only then the triggering action runs; a Text-tool press outside the hit area then opens a new box at that point. Undo and Redo are not commit triggers (AC-13).
- [ ] AC-12 (state): While a live text is open, panning and zooming (wheel, pinch, middle-button drag, the zoom control, Fit and 100 %) and using the options bar or the color controls leave it open and editable.
- [ ] AC-13 (event): When Undo is invoked by any route (the editor's Undo keys inside or outside the text, Edit ▸ Undo) while a live text is open, then its last undo step is undone and the live text stays open with the text, settings and position of the step before; the caret goes to the end of the text and the in-text selection is cleared. When Undo removes the step that began the live text, the box closes and the layer is as it was before. Redo re-applies each step; redoing the beginning step opens the live text again with its box and handle, the caret at the end of the text and keyboard focus in it.
- [ ] AC-14 (event): When the caret is in a live text and the user presses Ctrl+C / Ctrl+X / Ctrl+V (⌘ on macOS), then the selected characters go to the clipboard as plain text, cut also removes them, and paste inserts the clipboard's plain text at the caret with its line breaks; with no characters selected, copy and cut do nothing, and a clipboard without text pastes nothing. No image is pasted as a layer and the canvas is not copied.
- [ ] AC-15 (error): If a live text is committed while it is empty or holds only whitespace, then it closes, its steps are removed, the layer is as it was before the live text began, and no undo step remains from it.
- [ ] AC-16 (error): If a live text lies entirely outside the canvas when it is committed, then it closes, its steps are removed and no undo step remains from it; a live text partly outside the canvas stays editable, and the layer holds only the part on the canvas.
- [ ] AC-17 (state): While a selection exists, the live text is drawn only inside it, feathered edges scaling the text's alpha by coverage as the other Image tools do.
- [ ] AC-18 (error): If the Text tool is pressed while the active layer is locked, inside a locked group, or a group, then no box opens and the same notice the other pixel tools show in that case appears, naming the layer.
- [ ] AC-19 (error): If the bundled font cannot be loaded, then no box opens, a status notice says that the text font could not be loaded, and the next Text-tool press tries to load it again; while the font is loading, a second press moves the pending box to the new point and choosing another tool drops it.
- [ ] AC-20 (always): The MCP `draw.text` tool accepts `underline` and `strikethrough` booleans (default false) and paints the same pixels as the editor's commit of the same text and settings, with `draw.text`'s `x` being the commit's alignment anchor (box left edge plus 0, ½ or 1 times the box width for Left, Center, Right).
- [ ] AC-21 (event): When Escape is pressed while a live text with characters is open and no handle drag is in progress, then the live text is cancelled: the box closes, the layer is as it was before the live text began, all its steps are gone from the undo history, the redo history holds nothing of it, and the undo step before it is untouched.
- [ ] AC-22 (always): The `pincel-image-wasm` text-layout query returns, for a text and its settings, each line's box and the caret position of every character boundary, and these agree with what `draw_text` paints: with anti-alias off, drawing a line's characters before a boundary at the line's position and the characters after it at that boundary's caret position paints the same pixels as drawing the whole line, for every boundary whose neighbouring glyphs do not overlap.
- [ ] AC-23 (state): While keyboard focus is in a live text, characters, Space, Enter, Backspace, Delete, the arrow keys, Home / End and Ctrl+A / C / X / V (⌘ on macOS) act on the text only, so tool keys and Space type instead of switching tools or panning; Tab is ignored and keeps focus in the text; the editor's Undo / Redo keys run AC-13, Escape runs AC-21 and Ctrl+Enter / ⌘Enter commits; every other shortcut with Ctrl / ⌘ or Alt (Save, Open, Alt+W, Alt+Page Down, …) runs as it does with focus on the canvas, committing the text first (AC-11).
- [ ] AC-24 (event): When the core's live text ends or returns without the user's input in the text — an MCP tool call or another command commits it, a history-panel jump or an Undo / Redo from outside the text removes or restores it — then the box, caret and handle follow the core's reported live object: they close (keyboard focus goes back to the canvas) when no live text is reported, and reappear at its reported position when one is.

## Technical notes

- **Live object** (`image-live-objects`): the live text is one live object kind. Its parameters — text, position (box left / top), size, bold, italic, underline, strikethrough, align, anti-alias, color, font id — sit beside the document on the `CommandBus`; caret and in-text selection are UI state, never part of an undo step, which is why Undo / Redo put the caret at the end (AC-13). The font id is the slot `system-fonts` fills. Begin, edit (with a gesture id for typing bursts, handle drags and slider drags), commit and cancel are that spec's operations; its rules for steps (AC-03 merge), commit-before-other-commands (AC-08), no-change commit (AC-12), save / export / autosave (AC-13) and undo capacity (AC-14) apply unchanged. `image-live-objects-ui` AC-09 / AC-11 / AC-14 / AC-15 give the UI side for commit triggers, Escape, autosave and closing.
- **Core** (`crates/pincel-image-core/src/tools/text.rs`): `TextStyle` gains `underline` and `strikethrough`. Stroke position and thickness come from the font's underline / strikeout metrics where `ab_glyph` / its `owned_ttf_parser` exposes them without a new dependency; otherwise Liberation Sans's own values are used — underline top 0.106 em below the baseline, 0.073 em thick; strikethrough centered 0.259 em above the baseline, 0.049 em thick (the Arial-compatible metrics Liberation Sans is built to match; to verify against the bundled file) — each at least 1 px and thickened like faux bold when bold is on. The layout `Text::layout` uses (pen positions with kerning, line height `ascent − descent + line_gap`) is factored out so the wasm query (AC-22) and the raster share it.
- **Wasm** (`crates/pincel-image-wasm`): a new layout export (owner's choice) takes the same text and settings as `draw_text`. A hint for its shape: per line top, baseline, left and width; per character boundary an x. It is new public API (AGENTS.md §3.3 stopping point).
- **Box position** (AC-01, AC-09, AC-20): the box's top-left is the live object's position; the `draw_text` anchor is derived as `x = box_left + fraction × box_width`. This changes today's placement, where a Center / Right entry is anchored about the press point (`entryShift` in `MainCanvas.svelte`).
- **Command / MCP**: `CommandSpec::DrawText` (`crates/pincel-image-core/src/command/spec.rs`) gains `underline` and `strikethrough` with `#[serde(default)]`, so existing JSON stays valid; the generated `ui/src/modes/image/core/generated/CommandSpec.ts` follows. `draw.text` in `crates/pincel-agent/src/image/draw.rs` forwards both (Decision Log 2026-11-05 MCP tool parity).
- **Focus and keys** (AC-12, AC-23): the commit must no longer hang off the text box's `blur`. Today `isTypingTarget` turns every editor shortcut off inside a `<textarea>`; the live text needs its own key routing per AC-23 instead.
- **Locked notice** (AC-18): `pointer.ts` shows `layers.locked` for every `editRefused` target, groups included.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Live text open, the user switches the active layer in the layers panel | The text commits onto the layer it was opened on (AC-11), then the other layer becomes active |
| Live text open, the user locks or hides its layer | Commit first (another command, AC-11), then the lock / hide applies |
| Live text dragged by its handle completely off the canvas, then committed | Closes, no undo step remains (AC-16); dragging it back before commit keeps it intact |
| Commit with only line breaks or spaces, underline on | Closes, layer as before, no undo step remains (AC-10, AC-15) |
| All characters deleted, then Ctrl+Z | The deletion burst is undone, the text comes back (AC-03, AC-13) |
| Box still empty, Bold clicked | Only the tool option changes; no step, no live object (AC-01) |
| Size changed to 400 px while the text is open | Box and text grow to the right and down from the same top-left (AC-09); text past the canvas edge is not on the layer (AC-16) |
| Center-aligned text, a longer line typed | The box widens to the right; all lines re-center within the new width (AC-09) |
| Live text open, an effect dialog is opened from the menu | The text commits first; the effect previews the layer with the text in it (AC-11) |
| Live text open, Ctrl+S with the caret in the text | The text commits first, then saves; the file contains it (AC-11, AC-23) |
| Live text open, Edit ▸ Copy from the menu | The text commits first, then the canvas copy runs (AC-11); Ctrl+C with the caret in the text copies characters instead (AC-14) |
| Live text open, the tab is closed | The text commits first, so the unsaved-changes prompt covers it (AC-11) |
| Autosave fires while a live text is open | It does not commit; the snapshot holds the layer with the text's pixels; after recovery the text is plain pixels |
| An MCP tool call arrives while the user is typing | The core commits the text first; box and caret close (AC-24) |
| Undo pressed repeatedly while a live text is open | Each press undoes one step; the press that removes the beginning step closes the box; the next press undoes the step before (AC-13) |
| Escape after undoing every step | No live text is open any more; Escape behaves as without one (`image-editing` AC-31) |
| Escape after a commit | Nothing of the committed text changes; the commit is one undo step (AC-11, AC-21) |
| Escape during a move-handle drag | The drag is undone, the text stays live (AC-07) |
| Pinch-zoom on a tablet with the live text open | The text stays open and keeps its caret (AC-12) |
| Selection exists, text placed half inside it | Only the part inside is on the layer (AC-17) |
| Underline on, text with descenders ("gjpqy") | The stroke runs at the font's underline position; it may cross descenders, as in paint.net |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `image-live-objects` provides begin / edit (with gesture ids) / commit / cancel and the reported live object, and accepts a text kind with its own parameters | Its technical notes reserve kind-specific parameters for `image-live-text` | Medium: if its record shape differs, the wiring changes, not the ACs |
| Options apply to the whole text, never to a character range | paint.net does the same; one `draw_text` per commit | Low |
| The text box's size follows its content (widest line × line count), no word wrap | Today's entry and the core layout have no wrap width; paint.net breaks only at explicit line breaks | Low |
| The move handle sits at the text box's bottom-right corner, like paint.net's "nub", and is grabbed within the reach of `image-live-objects-ui` AC-05 | Familiar to paint.net users, away from the caret's usual start; one grab rule for all handles | Low |
| Liberation Sans is the only face; bold / italic stay synthesised | Decision Log 2026-09-06; `system-fonts` changes it | Low |

## Open questions

<!-- none: owner answered on 2026-11-05, see Decisions -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/image-live-text.spec.ts` `a Text press opens an empty box; the first character begins the text` | built |        |
| AC-02 | `ui/e2e/image-live-text.spec.ts` `typing draws the text into the layer as the commit leaves it` | built |        |
| AC-03 | `ui/e2e/image-live-text.spec.ts` `typing draws the text into the layer as the commit leaves it`, `a typing burst is one step; a caret jump starts the next`, `Enter breaks the line; the box keeps its top-left and fits the lines` | built |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 | `ui/e2e/image-live-text.spec.ts` `Enter breaks the line; the box keeps its top-left and fits the lines`, `left-aligned lines line up inside the box`, `center-aligned lines line up inside the box`, `right-aligned lines line up inside the box` | built |        |
| AC-10 | `crates/pincel-image-core/src/tools/text.rs` `underline_runs_below_the_baseline_to_the_last_ink`, `strikethrough_crosses_the_lowercase_letters`, `a_whitespace_only_text_gets_no_decoration`, `decorations_combine_with_bold_italic_and_alignment` | built |        |
| AC-11 | `ui/e2e/image-live-text.spec.ts` `Ctrl+Enter, Finish, another tool and a press outside commit the text`, `a document command or an active-layer change commits the text first`; save, export, copy, tab switch and close commit through the shared live-object hooks (`ui/e2e/image-live-commit.spec.ts`) | built |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 | `ui/e2e/image-live-text.spec.ts` `a text of only whitespace or wholly off the canvas leaves no step` | built |        |
| AC-16 | `ui/e2e/image-live-text.spec.ts` `a text of only whitespace or wholly off the canvas leaves no step` | built |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 | `crates/pincel-agent/src/tests/image_draw.rs` `draw_text_underline_and_strikethrough_match_the_text_tool`; `crates/pincel-image-core/tests/live_text.rs` `begin_live_text_draws_draw_text_at_the_box_anchor` | built |        |
| AC-21 | `ui/e2e/image-live-text.spec.ts` `Escape cancels the live text and keeps the step before it` | built |        |
| AC-22 | `crates/pincel-image-core/src/tools/text.rs` `layout_carets_split_a_line_without_changing_its_pixels`, `layout_reports_aligned_lines_inside_one_box`; `crates/pincel-image-core/tests/live_text.rs` `text_layout_query_matches_the_drawn_box` | built |        |
| AC-23 |      | open   |        |
| AC-24 |      | open   |        |

## Decisions

- 2026-11-05 (owner's choice): The live text follows the live-object architecture of `image-live-objects` — the core draws it into its layer at every change, its settings live beside the document on the `CommandBus` (not in `.ora`), each edit is a normal undo step, commit folds them into one, cancel removes them, and any other command or an active-layer change commits first — because one model for all live objects keeps undo, autosave and MCP behaving the same for shapes, gradients and text, and the layer always shows what the user sees.
- 2026-11-05 (owner's choice): Escape cancels the live text; Ctrl+Enter / ⌘Enter commits; Enter inserts a line break; a press outside the box or choosing another tool commits — because one Escape rule for all live objects is easier to learn than paint.net's text-only "Esc finishes". This changes today's behavior, where Escape commits the text.
- 2026-11-05 (owner's choice): `pincel-image-wasm` gains a text-layout query (line boxes, caret positions) — because caret and hit test must come from the layout the pixels are painted with; a CSS textarea drifts with faux bold / italic and anti-alias off.
- 2026-11-05: The live object is begun by the first typed or pasted character, not by the press — because an empty text is degenerate like a zero-size box (`image-live-objects` AC-09: nothing is begun), and a stray Text click should leave no undo step. A text emptied later by deleting stays live, so Undo can bring the characters back.
- 2026-11-05: Undo while a live text is open undoes its last step instead of discarding the whole text — because each edit is a normal undo step under the owner's architecture (`image-live-objects` AC-04); Escape drops the whole text. Undo and Redo are therefore not commit triggers.
- 2026-11-05: After Undo / Redo the caret goes to the end of the text — because the caret is UI state outside the core's steps, and Undo can also come from outside the text (MCP, history panel), where no caret history exists.
- 2026-11-05: A typing burst is one undo step, and consecutive changes of the same option merge — because one step per character would make Undo useless for typing, and the merge rule is `image-live-objects` AC-03's.
- 2026-11-05: The press point is the box's top-left for every alignment, and the box keeps its left edge and top through every edit — because paint.net places text the same way, and a box that grows to the right never jumps under the caret; this changes today's Center / Right placement about the press point. `draw.text` keeps its anchor semantics (AC-20) so existing agent calls stay valid.
- 2026-11-05: Underline and strikethrough run to the last non-whitespace character and skip whitespace-only lines — because word processors and paint.net do not underline trailing spaces, and it keeps a whitespace-only text a no-op (AC-15).
- 2026-11-05: Chord shortcuts (Ctrl / ⌘ / Alt) run while the caret is in the text, plain keys type — because Save, tab switching and the other chords must stay reachable without leaving the text first, while tool keys and Space must type; today every shortcut is off in the textarea. Edit ▸ Copy from the menu is a canvas command and commits (`image-live-objects-ui` AC-09); Ctrl+C in the text is a text-field copy.
- 2026-11-05: Focus leaving the text box no longer commits — because otherwise no option or color change could reach the live text (today's `blur` commit); the commit triggers are listed in AC-11.
- 2026-11-05: The font load is retried on the next press, not automatically — because that is today's `ensureFont` behavior (a failed load resets the cache) and an automatic retry while offline would fail silently again; the notice tells the user why nothing opened.
- 2026-11-05: Underline and strikethrough are additive `#[serde(default)]` fields on the existing `draw_text` command and MCP tool — because existing commands and agent calls stay valid, and the MCP tool parity decision requires the agent tool to offer what the editor offers.
- 2026-11-06 (spec-build) — Underline and strikethrough use Liberation Sans's Arial-compatible metrics from the Technical notes (underline top 0.106 em below the baseline, 0.073 em thick; strikethrough centred 0.259 em above it, 0.049 em thick), at least 1 px and thickened by the faux-bold smear; they stay straight under faux italic. `ab_glyph`'s public API exposes no underline / strikeout metrics without reaching into `owned_ttf_parser`, and the bundled face is the only one until `system-fonts`.
- 2026-11-06 (spec-build) — Core and wire API (new public items): `TextStyle::{underline, strikethrough}`, `Text::layout_lines` → `TextLayout` / `LineLayout` (line top, baseline, left, width, caret x per boundary, ink end), `command::text_box`, `LiveText` / `LiveFont` (a live object kind; fonts compare by id), `LiveEdit` text fields, `CommandSpec::BeginLiveText` and the text fields of `edit_live`, `LiveObjectSpec::Text`, `live_spec::{TextLayoutRequest, text_layout}`, and `pincel-image-wasm` `text_layout(font_id, request)`, with generated `TextLayout.ts` / `LineLayout.ts` / `TextLayoutRequest.ts`.
- 2026-11-06 (spec-build) — The live text's points are its box top-left twice, so the generic reach check (image-live-objects AC-10) and a `points` edit (the move handle) work unchanged; an empty text is degenerate (nothing begun), an emptied one stays live (image-live-objects AC-16).
- 2026-11-06 (spec-build) — Red runs: the text-style and layout tests failed to compile without `underline` / `strikethrough` / `layout_lines` (7 errors); `tests/live_text.rs` failed to compile without the text kind (6 errors); the agent test failed to compile until `draw.text` forwarded the two new fields.
- 2026-11-06 (spec-build) — The box's top-left is the pressed document pixel (the press point floored), so the text sits on the pixel grid and a handle drag (whole pixels, AC-07) keeps it there.
- 2026-11-06 (spec-build) — Keyboard input goes to a hidden, focused `<textarea>` (`data-live-text`, labelled "Live text") that is emptied after every input event: typed and IME-composed characters arrive as input, the text itself lives in the core. Its keydown handler owns the text keys of AC-23 and stops them; chords it does not own bubble to the editor's shortcut handler, which no longer ignores this one field.
- 2026-11-06 (spec-build) — Ctrl+C / Ctrl+X in the text write the selected characters with `navigator.clipboard.writeText` from the keydown (user activation, as the canvas copy does); Ctrl+V lets the browser's `paste` event bring the clipboard's `text/plain`.
- 2026-11-06 (spec-build) — The first character's begin step stays its own step and the characters typed after it form the burst step (AC-01 makes the begin one undo step); a burst's gesture ids start at 2^30, clear of the pointer code's stroke and drag ids.
- 2026-11-06 (spec-build) — Focus returns to the live text after an option change once no pointer is down and no other text field (hex field, rename field) holds the focus, so a slider drag or a hex entry is not interrupted; a select or checkbox gives the focus back at once.
- 2026-11-06 (spec-build) — Red run (block 1, `ui/e2e/image-live-text.spec.ts`): all 12 tests failed against the textarea entry ("element(s) not found" for the live text field and box).
