---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §5.4, pincel.md §6, pincel.md §9.4, pincel.md §15 (2026-10-12 effects honour the selection), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Image mode: Fill Selection

## Context

paint.net has Edit ▸ Fill Selection (Backspace): it paints the selected area of the current layer with the primary color, the whole layer without a selection. Image mode has the opposite command, Edit ▸ Delete Selection (`delete_selection` in `pincel-image-core`, Delete and Backspace in `shortcuts.ts`), but no way to fill a selection short of a bucket fill with tolerance 255, which still depends on the layer's content and does not reach every selected pixel. Users coming from paint.net, Photoshop (Alt+Backspace) or GIMP expect the command.

This is the first of two specs for paint.net's Edit and options-bar extras. The second, `image-edit-extras-tool-options.md`, adds the Antialiasing and Blending (Normal / Overwrite) switches to the options bar.

Delete Selection already sets the pattern this spec follows: one undoable command over the active layer, "no selection = everything selected" (spec §5.4, §8.1 of the archived `fineliner.md`), and feathered coverage scales the change instead of cutting a hard edge (Decision Log 2026-10-12).

Decision Log 2026-11-05 "MCP tool parity" requires an MCP tool for every document-changing capability of the editor, so this spec adds one (AC-10 to AC-12).

"The foreground color" means the color of the foreground swatch, with its alpha once `color-picker.md` gives Image mode color alpha; until then it is opaque.

## Scope

### In scope

- Edit ▸ Fill Selection in Image mode, below Delete Selection
- Default shortcut: Backspace on Windows / Linux, Option+Backspace on macOS, where Backspace stays Delete Selection
- Filling the selection, or the whole active layer without one, with the foreground color, feathered and partial coverage blending proportionally, as one undo step
- The locked-layer and group refusals the other Edit commands have
- The MCP tool `select.fill` for Image documents

### Out of scope

- Fill Selection in Pixel mode — because Pixel mode's marquee is a hard-edged rectangle, indexed sprites need palette handling, and `pincel-core` has no fill-region command; it is cheap to add later as its own spec, and until then Pixel mode does not bind the action
- Filling with the background color, a pattern or a gradient (Photoshop's Fill dialog) — because paint.net fills with the primary color only, and the Gradient tool covers ramps
- Stroke Selection (outline the selection) — because paint.net has no such command; it belongs to a separate spec if wanted
- Honoring the options bar's Blending mode (Normal / Overwrite) — because Fill Selection is an Edit command, not a tool, and has its own fixed semantics (AC-03); the Blending switch arrives with `image-edit-extras-tool-options.md`
- Making Delete Selection's menu entry enabled without a selection — because that is existing behavior of another command; this spec only notes the asymmetry under Decisions

## Acceptance criteria

- [ ] AC-01 (always): The Image editor's Edit menu has a Fill Selection entry directly below Delete Selection, labelled in every UI language, showing its shortcut hint from the keymap, and enabled whenever a document is open.
- [ ] AC-02 (event): When the user chooses Edit ▸ Fill Selection or presses its shortcut while a selection exists, then every pixel of the active layer with selection coverage 255 becomes exactly the foreground color (its red, green, blue and alpha), and every pixel with coverage 0 is unchanged.
- [ ] AC-03 (event): When Fill Selection runs over a pixel with partial selection coverage c (1–254, from feathering or an anti-aliased selection edge), then the pixel moves from its previous value toward the foreground color by c/255: its alpha equals previous alpha + (foreground alpha − previous alpha) × c/255 within ±1, and its color lies between the previous and the foreground color; for an opaque foreground over an opaque pixel, each color channel equals previous + (foreground − previous) × c/255 within ±1.
- [ ] AC-04 (event): When the user runs Fill Selection without a selection, then every pixel of the active layer becomes exactly the foreground color.
- [ ] AC-05 (event): When Fill Selection has changed the layer, then it is exactly one undo step: one Undo restores every pixel of the layer to its previous value, Redo repeats the fill, and the selection is the same before and after the fill.
- [ ] AC-06 (event): When Fill Selection would leave every pixel as it was (the selected pixels already hold the foreground color), then no undo step is added and the document's unsaved-changes state is the same as before the command.
- [ ] AC-07 (error): If the selection exists but covers no pixel, then Fill Selection changes nothing and adds no undo step.
- [ ] AC-08 (error): If the active layer is locked, lies in a locked group, or is a group, then Fill Selection changes no pixel, adds no undo step, and the editor shows the same locked-layer notice as Delete Selection.
- [ ] AC-09 (always): Fill Selection changes only the active layer: the pixels of every other layer, layer properties, the canvas size and the selection stay unchanged, also when the active layer is hidden or has an opacity below 100 %.
- [ ] AC-10 (always): `tools/list` of `pincel-mcp` contains `select.fill`, meeting `mcp-headless-pixel` AC-02, with the arguments `doc`, `layer` (default: the active layer, as for `select.delete` in `mcp-parity-selection-transform`) and `color` (default `#000000`, color syntax of `mcp-headless-pixel` AC-21); it takes no `frame`, since Image documents have none.
- [ ] AC-11 (event): When the client calls `select.fill` on an Image document, then the document's pixels equal the editor's after Fill Selection with that color on that layer and the same selection (the whole layer without a selection), the selection stays, the call is one undo step, a call that changes nothing adds none, and the result reports `changed` (`true` or `false`) as `select.delete` does, without the selection's bounds.
- [ ] AC-12 (error): If `select.fill` is called on a Pixel document, names a layer that does not exist, targets a locked layer or a group, gets a `color` that is not a valid color, or gets a `frame` argument, then the call fails with `isError: true` (naming the document's mode for a Pixel document, the layer for a locked layer or group, otherwise the argument and its value), and the document is unchanged.
- [ ] AC-13 (always): The `pincel://guide` resource names `select.fill`, that it works on Image documents only, and that it fills the whole layer without a selection.
- [ ] AC-14 (always): Fill Selection's default shortcut is Backspace on Windows and Linux and Option+Backspace on macOS. On Windows and Linux, Delete alone runs Delete Selection; on macOS, Backspace and Delete both run Delete Selection, in both modes. In Pixel mode on Windows and Linux, Backspace runs neither Fill Selection nor Clear, and Delete still clears the marquee.

## Technical notes

- The core command mirrors `tools::delete_selection` (`crates/pincel-image-core/src/tools/delete.rs`): one `SetPixels` over the layer, so undo restores the layer exactly and the existing lock refusal in `CommandSpec::refused` applies once the new variant reports its target layer. The undo label is "Fill Selection" (the history panel spec on `feat/history-panel` gives undo steps kind ids from the core; this command gets its own).
- `CommandSpec` gains a `fill_selection { layer, color }` variant; the ts-rs mirror under `ui/src/modes/image/core/generated/` is regenerated (`cargo test -p pincel-image-wasm export_command_spec`).
- The menu entry goes through `layerEdit(false, …)` in `ImageEditor.svelte`, as Delete Selection does, which shows the locked notice (AC-08).
- The tool name `select.fill` pairs with `select.delete` from `mcp-parity-selection-transform` (its AC-21 to AC-23); share its target resolution and lock / group refusal.
- The platform split of the default key uses the existing `isMac` check in `shortcuts.ts`; the keymap overview and menu hints print the platform's chord.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Selection extends to the canvas edge after Select All | Every pixel of the layer is filled, as without a selection |
| Feathered selection with radius 20 | Pixels in the feather band blend by coverage (AC-03); none outside the band changes |
| Foreground color with alpha 128 (after `color-picker.md`) over a transparent pixel at coverage 255 | The pixel becomes the foreground color with alpha 128 |
| Foreground fully transparent (alpha 0, after `color-picker.md`) | Every selected pixel loses alpha in proportion to its coverage, exactly as with Delete Selection (AC-02, AC-03) |
| A user keymap override from before this spec binds Backspace to Delete Selection | The override wins: Backspace keeps running Delete Selection, and Fill Selection has no effective key on that platform until the user rebinds it, because a stored user choice is never overridden by a new default |
| Fill Selection twice in a row with the same color | The second run changes nothing and adds no undo step (AC-06) |
| Active layer hidden | The fill happens on the hidden layer; the composite shows no change until it is shown |
| Fill Selection while the Move tool has lifted selected pixels mid-drag | Not reachable: the drag ends on pointer-up before a menu or key action runs |
| `select.fill` with `color: "#ff000080"` | Fills with alpha 128 as in AC-03 / AC-02, independent of the editor's color picker |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Fill Selection moves each pixel toward the foreground by its coverage, alpha included (fully covered pixels become exactly the foreground), instead of compositing over the old pixel | paint.net's Fill Selection replaces the selected pixels; the rule makes Fill with a transparent color equal Delete Selection, and it is the Overwrite rule of `image-edit-extras-tool-options.md` AC-11. For an opaque foreground over an opaque pixel the result equals compositing | Low: differs from compositing only over transparent pixels or once color alpha exists |
| `select.fill` defaults to `color: #000000` instead of requiring a color | The MCP server has no foreground swatch, and `draw.stroke`, `draw.shape` and `draw.text` default to `#000000` too | Low |
| The menu entry stays enabled without a selection | paint.net fills the whole layer then; AC-04 makes that behavior reachable from the menu | Low |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |

## Decisions

- 2026-11-05: Delete Selection's menu entry is disabled without a selection although its key clears the whole layer then; Fill Selection's entry is enabled without one (AC-01, AC-04). The asymmetry is left for a separate small fix rather than widened here.
- 2026-11-05: Default key (owner's choice): Backspace runs Fill Selection on Windows / Linux; on macOS Backspace stays Delete Selection and Fill Selection gets Option+Backspace, because that gives paint.net parity where paint.net users are without taking the only delete key from Mac laptops (Photoshop's Mac fill key). Pixel mode does not bind Fill Selection (AC-14).
- 2026-11-05: MCP name (owner's choice): `select.fill`, not `draw.fill_selection`, because it pairs with `select.delete` from `mcp-parity-selection-transform` and takes the same arguments.
- 2026-11-05 (clarification pass): Fill Selection interpolates toward the foreground by coverage, alpha included, because then a transparent fill equals Delete Selection with no hard step at the feather edge and the rule matches Overwrite in `image-edit-extras-tool-options.md`. `select.fill` takes no `frame` (Image only) and reports `changed` like `select.delete`, because one result shape for the pair keeps clients simple. A stored keymap override keeps its chord over the new default, because user choices are never silently replaced.
