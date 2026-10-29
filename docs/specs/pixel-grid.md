---
status: todo
created: 2026-10-28
size: M
mode: Pixel
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §7.1, pincel.md §17.3]
---

# Pixel grid and custom grid in Pixel mode

## Context

A pixel grid and a configurable grid are basic pixel-art equipment: Aseprite, Pixelorama and Pro Motion have both, Piskel has the pixel grid. In Pincel a grid exists only as the tile grid while stamping tiles. §17.3 (Piskel #636) already maps the pixel grid to "standard view toggle; on by default at zoom ≥ 8".

- **View ▸ Show Pixel Grid** draws thin lines between sprite pixels once the zoom reaches 8 screen px per pixel.
- **View ▸ Show Grid** draws a configurable grid: cell size W×H (default 16×16), offset X/Y, line colour and opacity.
- **View ▸ Grid Settings…** edits the custom grid.

The grid geometry belongs to the sprite and already round-trips through the `.aseprite` header (§7.1: "the header's … grid (`Metadata::grid`)", implemented 2026-10-11); what is missing is editing it and drawing it.

## Scope

### In scope

- View menu entries Show Pixel Grid, Show Grid (checkbox entries) and Grid Settings…
- Pixel grid drawn on the canvas overlay, hidden below zoom 8
- Custom grid drawn on the canvas overlay, on top of the pixel grid
- Grid Settings dialog: cell size, offset, line colour, opacity
- Editing the sprite's grid size and offset as an undoable document change, saved in the `.aseprite` file
- Show / hide states persisted per user, not in the file
- Keyboard shortcuts Ctrl+' (grid) and Ctrl+Shift+' (pixel grid)
- en + de strings

### Out of scope

- Snap to grid for moves and shapes — because it is a follow-up feature on top of the drawn grid.
- An isometric grid — because the `.aseprite` header grid is rectangular and the legacy plan defers it.
- Grids in Image mode — because rulers and guides for Image mode are a separate backlog item.

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses View ▸ Show Pixel Grid, then the entry's checked state flips and the pixel grid is shown or hidden accordingly.
- [ ] AC-02 (state): While Show Pixel Grid is on and the zoom is 8 or more screen px per sprite pixel, the canvas shows lines between sprite pixels (an element screenshot of the canvas area differs from the same view with Show Pixel Grid off).
- [ ] AC-03 (state): While the zoom is below 8 screen px per sprite pixel, the pixel grid is not drawn (an element screenshot is identical with Show Pixel Grid on and off).
- [ ] AC-04 (always): On a profile with no stored grid preference, Show Pixel Grid is on.
- [ ] AC-05 (event): When the user chooses View ▸ Show Grid, then the entry's checked state flips and the custom grid is shown or hidden accordingly, at every zoom level.
- [ ] AC-06 (state): While Show Grid is on, the custom grid draws lines at `offset + k × cell size` in sprite coordinates on both axes, in the configured line colour and opacity.
- [ ] AC-07 (event): When the user opens View ▸ Grid Settings…, changes cell size, offset, colour or opacity and confirms, then the custom grid is drawn with the new values.
- [ ] AC-08 (always): A new Pixel document's grid is 16×16 cells at offset 0, 0.
- [ ] AC-09 (event): When the user changes the grid's cell size or offset, saves the document as `.aseprite` and reopens it, then the grid has the changed cell size and offset.
- [ ] AC-10 (event): When the user changes the grid's cell size or offset and then undoes, then the previous cell size and offset return in one undo step, and redo re-applies the change.
- [ ] AC-11 (event): When the user toggles Show Grid or Show Pixel Grid and reloads the app, then both entries keep their checked state, and saving a document does not write these states into the file.
- [ ] AC-12 (event): When the user presses Ctrl+', then Show Grid toggles; when the user presses Ctrl+Shift+', then Show Pixel Grid toggles.
- [ ] AC-13 (error): If the user enters a cell width or height outside 1–65535, or an offset outside −32768–32767, in Grid Settings, then the values are not applied and the document grid stays unchanged.
- [ ] AC-14 (always, manual: visual rendering quality): Grid lines are 1 device pixel wide and crisp at every zoom level and device pixel ratio, the pixel grid uses a low-contrast theme colour, and the custom grid draws over the pixel grid.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names.

**Already there (drift since the legacy plan).** The grid round-trips today: `Metadata::grid: Option<Rect>` (`crates/pincel-core/src/document/mod.rs` ≈L43) is read from the header in `codec/aseprite_read.rs` (≈L145, always `Some`) and written in `codec/aseprite_write.rs` (≈L170; `None` writes Aseprite's default 16×16 at the origin). `aseprite-writer`'s `Header` has `grid_x/grid_y: i16`, `grid_width/grid_height: u16` (`crates/aseprite-writer/src/file.rs` ≈L62). So no `aseprite-writer` change is needed; missing are an undoable command to set the grid, wasm accessors, and the UI.

**Do not use** `render::request::Overlays.grid` in `pincel-core` — `compose()` returns `OverlaysUnsupported` when it is set; grids are drawn on the UI overlay canvas instead (app code never reads canvas pixels, AGENTS.md §9).

**Overlays.** `ui/src/modes/pixel/overlay-painter.ts`: `OverlayState` (≈L29), `paintOverlays` (≈L53), `paintTileGrid` (≈L212) already draws a grid at the tileset tile size in screen space — reuse its sprite → screen mapping (zoom, pan) and line style. The overlay is a separate canvas above the render target (`ui/src/lib/render/types.ts`).

**Rendering loop.** `ui/src/modes/pixel/render.ts`: `overlayState()` (≈L84), `paintOverlays` (≈L120), rAF `tick` (≈L168); the overlay redraws on zoom, pan and resize. Draw lines only over the visible viewport, so the cost grows with visible lines, never with sprite size; snap lines to device pixels.

**View state.** `viewInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L62) has no grid fields yet.

**View menu.** `ui/src/modes/pixel/menus.ts` (≈L223–235); the Probe item (`checked: c.probeOn`) is the checkbox pattern.

**Keys.** `ui/src/lib/shell/shortcuts.ts` has no `'` binding yet; `power-user-shortcuts-coverage.md` lists the same keys for this feature.

**Persistence.** IndexedDB prefs (`ui/src/lib/idb/prefs.ts`: `getPref` / `setPref`); the per-mode layout pref lives in `ui/src/lib/components/layout/panels.svelte.ts`.

**e2e.** Proposed `ui/e2e/grid.spec.ts`: menu checkbox state; at zoom ≥ 8 the canvas element screenshot differs with the pixel grid on vs off; grid settings survive save and reopen (download then reopen, pattern in `ui/e2e/ora.spec.ts`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Zoom exactly 8 | Pixel grid drawn |
| Zoom changes from 8 to 7 with Show Pixel Grid on | Pixel grid disappears; the menu entry stays checked |
| Offset larger than the cell size (e.g. offset 20, cell 16) | Lines at `20 + k × 16` for every integer k, which equals offset 4 |
| Cell size larger than the sprite | At most the offset lines fall inside the sprite; nothing else drawn |
| `.aseprite` file whose header grid is 0 wide or high | The custom grid draws nothing; the value is saved back unchanged |
| Show Grid on while the tilemap stamp tile grid is shown | Both are drawn |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Valid cell size 1–65535 and offset −32768–32767 (AC-13) | The `.aseprite` header stores the grid as `u16` size and `i16` offset; a 0-size grid has no cells | Low |
| A header grid of size 0 is drawn as nothing and kept as is | Avoids a division by zero without rewriting user data | Low |

## Open questions

- The Decision Log entry is not written yet: grid size and offset stored in the sprite through the `.aseprite` header (undoable), show / hide flags kept in user prefs.
- Where are the custom grid's line colour and opacity stored? The `.aseprite` header has no field for them — user prefs (per user, all documents) or per document?
- What is the default of Show Grid (the custom grid) on a profile with no stored preference — on or off? (§17.3 only fixes the pixel grid default.)
- Should Grid Settings offer the active tileset's tile size as a preset? The legacy plan says it "can be offered".

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

- 2026-10-28 (legacy plan): Both grids are drawn on the overlay canvas, not composed into the frame — they are view decoration, not document pixels, and the document state stays in Rust.
- 2026-10-28 (legacy plan): Shortcuts Ctrl+' / Ctrl+Shift+' — Aseprite uses them, and `21-power-user-shortcuts.md` plans the same keys.
- 2026-10-29 (migration): The pixel grid is on by default — `pincel.md` §17.3 (#636) says "on by default at zoom ≥ 8", and a feature spec never contradicts the design spec.
- 2026-10-29 (migration): No `aseprite-writer` change — the header grid already round-trips through `Metadata::grid` (§7.1).
