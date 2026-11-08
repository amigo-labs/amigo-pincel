---
status: todo
created: 2026-11-05
size: L
mode: Image
design: [pincel.md §3.1, pincel.md §9.2, pincel.md §9.4, pincel.md §9.5, pincel.md §10.3]
---

# Image view aids: rulers, pixel grid and units

## Context

paint.net's View menu has three aids Image mode lacks: **Rulers** along the top and left edge of the canvas area, a **Pixel Grid** drawn between pixels when zoomed in, and **Units** (pixels, inches, centimeters) used by the rulers and the status bar. Image mode today shows the document size in the status bar but not the pointer position, has no rulers and no grid; the README backlog lists "rulers / guides (Image)".

Pixel mode already has a pixel grid (`pixel-grid.md`), but there the custom grid is sprite state stored in the `.aseprite` header. In Image mode every aid in this spec is **view state**: it is drawn over the canvas, it never changes the document, it is not an undo step and it is not written into any file. The Decision Log 2026-11-05 "MCP tool parity" excludes view state from MCP, so no MCP tool is added.

The Image document already carries a resolution: `DocumentMetadata.dpi` in `pincel-image-core` (default 96.0, §3.1 of the Image core). Nothing reads it from a file, writes it, or exposes it to the UI yet. This spec only shows the stored resolution to the user through the units; reading it from files, storing a changed value and editing it belong to a later spec (`image-resize-autocrop` uses a dialog-only conversion factor and does not change the document's resolution).

## Scope

### In scope

- View ▸ Rulers (checkbox): a horizontal ruler above and a vertical ruler left of the canvas area, labelled in image coordinates at the current zoom and pan, with a marker at the pointer position
- View ▸ Pixel Grid (checkbox): 1-px lines between image pixels from 400 % zoom up
- View ▸ Units (submenu, one checked): Pixels, Inches, Centimeters — used by the rulers and the status bar
- Status bar: the pointer position in image coordinates, and the image size, in the chosen unit
- Inches and centimeters computed from the active document's own stored resolution, which the Image editor now reports for each document
- Keyboard: Ctrl+Shift+' toggles the pixel grid (the key Pixel mode already uses); Ctrl+Alt+R (⌥⌘R on macOS) toggles the rulers in the desktop app
- Rulers on / off, pixel grid on / off and the unit persisted per user for Image mode in IndexedDB prefs
- en + de strings

### Out of scope

- Guides dragged out of the rulers and snapping to them — because they need their own interaction and persistence design; a follow-up on top of the rulers.
- A configurable custom grid (cell size, colour) in Image mode — because paint.net has none and Pixel mode's custom grid is document state in the `.aseprite` header, which Image formats lack.
- Editing or storing a changed resolution, and reading or writing it from PNG / JPEG / TIFF / ORA files — because a later resolution spec owns the document's resolution; `image-resize-autocrop` only converts inside its dialog and leaves the stored value alone.
- Units in the New, Resize Canvas and Scale dialogs — because those dialogs belong to `image-resize-autocrop` and the start screen.
- A units selector in the status bar — because View ▸ Units and the command palette (which lists every menu entry) already reach it, and the phone status bar has no room.
- Selection size in the status bar — because it needs the selection bounds from the core and is independent of rulers and units; a separate parity item.
- Rulers and units in Pixel mode — because pixel art works in pixels and `.aseprite` stores no resolution.
- MCP tools — because view state is excluded from MCP (Decision Log 2026-11-05 "MCP tool parity").

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses View ▸ Rulers, then the entry's checked state flips and a horizontal ruler above and a vertical ruler left of the canvas area are shown or hidden accordingly.
- [ ] AC-02 (state): While the rulers are shown, every labelled value v has a tick on its ruler whose screen position is within 1 CSS px of the screen x (horizontal ruler) or screen y (vertical ruler) where image coordinate v lies, and the label's text begins 0–4 CSS px right of (horizontal) or below (vertical) its tick, with value 0 at the image's top-left corner and negative values left of and above it; after a zoom or pan, the labels move to the new positions.
- [ ] AC-03 (state): While the rulers are shown, consecutive labels on a ruler differ by one step of 1, 2 or 5 × 10ⁿ of the current unit, are at least 40 CSS px apart on screen, and never overlap, at every zoom from the minimum to the maximum.
- [ ] AC-04 (state): While the rulers are shown and a pointer is over the canvas area (AC-24), each ruler shows a marker at the pointer's position along its axis (within 1 CSS px); while the pointer is outside the canvas area, neither marker is shown.
- [ ] AC-05 (state): While the rulers are shown, they cover no part of the canvas area: Fit shows the whole image beside the rulers, and turning the rulers on or off while the view is in fit mode re-fits the image to the remaining area.
- [ ] AC-06 (event): When the user chooses View ▸ Pixel Grid, then the entry's checked state flips and the pixel grid is shown or hidden accordingly.
- [ ] AC-07 (state): While the pixel grid is on and the zoom is 400 % or more, the canvas shows lines between image pixels inside the image's bounds (an element screenshot of the canvas area differs from the same view with the pixel grid off), and nothing beside the image (a screenshot of a region of the canvas area outside the image is identical with the pixel grid on and off).
- [ ] AC-08 (state): While the zoom is below 400 %, the pixel grid is not drawn (an element screenshot of the canvas area is identical with the pixel grid on and off).
- [ ] AC-09 (always, manual: visual rendering quality): Pixel-grid lines are 1 device pixel wide, crisp and on the pixel edges at every zoom and device pixel ratio, visible over both black and white pixels, and drawn under the selection outline.
- [ ] AC-10 (state): While a pointer is over the canvas area (AC-24), the status bar shows the position of the image pixel under the pointer (x and y rounded down), also for a pointer beside the image (negative values, or values at or past the width / height); while the pointer is outside the canvas area, the position is not shown.
- [ ] AC-11 (event): When the user chooses an entry of View ▸ Units (Pixels, Inches, Centimeters), then exactly that entry is checked and the rulers, the pointer position and the image size in the status bar switch to that unit.
- [ ] AC-12 (option): Where the unit is Pixels, values are whole numbers; where it is Inches, a pixel value p shows as p ÷ dpi, and where it is Centimeters as p ÷ dpi × 2.54, both rounded to two decimals, dpi being the active document's resolution (AC-23). The status bar adds the unit's suffix — English `px` / `in` / `cm`, German `px` / `Zoll` / `cm` — and the status bar and ruler labels use the UI language's decimal separator (`0.01 in`, `0,01 Zoll`); ruler labels carry no suffix.
- [ ] AC-13 (always): On a profile with no stored Image view preference, the rulers are hidden, the pixel grid is off and the unit is Pixels.
- [ ] AC-14 (event): When the user changes the rulers, the pixel grid or the unit and reloads the app, then all three keep their state and apply to every Image tab; Pixel mode's grid settings are unaffected, and a document saved with any combination of them on is byte-identical to one saved with all of them off.
- [ ] AC-15 (always): Turning the rulers or the pixel grid on or off, or changing the unit, adds no undo step and does not mark the document as having unsaved changes.
- [ ] AC-16 (event): When the user presses Ctrl+Shift+' (⇧⌘' on macOS) in Image mode, then the pixel grid toggles; Ctrl+' does nothing in Image mode.
- [ ] AC-17 (event): When the user presses Ctrl+Alt+R (⌥⌘R on macOS, matched by the physical R key) in the desktop app's Image mode, then the rulers toggle and no other command runs — the image is not rotated; in the web build the chord has no default binding and does nothing in Image mode.
- [ ] AC-18 (always): Help ▸ Keyboard Shortcuts in Image mode lists the Rulers entry (with Ctrl+Alt+R in the desktop app, without a chord on the web) and the Pixel Grid entry with its chord, no two of its entries list the same default chord, and no Image-mode default chord is one a browser tab cannot capture (Ctrl+N / T / W, Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+R, Ctrl+Shift+R, F5).
- [ ] AC-19 (event): When the user switches to another Image tab, then the rulers, the pixel grid and the status bar follow that tab's zoom, pan, size and resolution, while the on / off states and the unit stay as they were.
- [ ] AC-20 (error): If the stored Image view preference is missing, malformed or has an unknown unit, then the defaults of AC-13 apply and the editor opens normally.
- [ ] AC-21 (error): If the document's resolution is zero, negative or not a finite number, then Inches and Centimeters are computed with 96 DPI.
- ~~AC-22 (state): While no document is open, View ▸ Rulers, Pixel Grid and Units are disabled and no ruler, grid or pointer position is shown.~~ dropped: the state does not exist — closing the last Image tab releases the Image editor and shows the start screen or the Pixel editor (`document-tabs` AC-09).
- [ ] AC-23 (always): For every document, the resolution the Image editor reports is the document's stored resolution: 96 DPI for each newly created or opened document (no file's resolution is read yet), and the stored value for a document whose resolution differs (proved at the Image core's boundary, since no UI path sets one); the rulers and the status bar use the active document's value, taken again on every tab switch and every document change.
- [ ] AC-24 (event): When a mouse or pen hovers over the canvas area, or a touch, pen or mouse contact is down on it, then the pointer position and the ruler markers follow it; when the pointer leaves the canvas area, or a touch contact lifts, then both are hidden.
- [ ] AC-25 (error): If storing the Image view preference fails (IndexedDB unavailable or the write rejected), then the change still applies for the rest of the session, the editor keeps working without a message, and the next launch starts from the last successfully stored state or the defaults of AC-13.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names.

**Rendering.** Image mode does not use `ui/src/lib/render/`: it draws through its own viewport-sized Canvas2D adapter (`ui/src/modes/image/render/canvas2d.ts`) and a viewport-sized overlay canvas for the marching ants (`ui/src/modes/image/components/canvas/CanvasOverlay.svelte`, which already maps document paths through the view transform at one device pixel). The pixel grid belongs on a viewport-sized canvas of the same kind, below the ants, drawing only the lines inside the visible rect, so its cost grows with the visible lines, never with the image size. App code never reads canvas pixels (AGENTS.md §9).

**View transform.** `view` (`zoom`, `panX`, `panY`, `fit`) per tab in `ui/src/modes/image/stores/editor.svelte.ts` (`viewInit`); zoom is CSS px per image px, `MIN_ZOOM = 0.02`, `MAX_ZOOM = 64`. Fit lives in `MainCanvas.svelte` (`fitView`, 16 CSS px padding); the rulers' strips must be taken out of the area it fits into (AC-05). The ResizeObserver there already re-fits in fit mode.

**Rulers.** Ticks may be a canvas; drawing the labels and the pointer marker as positioned DOM elements (with test ids) keeps AC-02 – AC-04 testable from the DOM without reading canvas pixels. Pointer coordinates come from the stage's pointer events in `MainCanvas.svelte` / `tools/pointer.ts`.

**Status bar.** `{#snippet statusbar()}` in `ui/src/modes/image/ImageEditor.svelte` shows `width × height · sRGB` today and no pointer position.

**Units.** Keep the conversion and the step choice (1 / 2 / 5 × 10ⁿ) in a rune-free TypeScript module, so a node-side Playwright spec can test AC-03, AC-12 and AC-21 directly (AGENTS.md Gotchas: node-side specs can import plain TS, not runes).

**Resolution.** `DocumentMetadata::dpi` (`crates/pincel-image-core/src/document/mod.rs`) has no wasm accessor yet; this spec adds one read-only getter in `pincel-image-wasm` (e.g. `document_dpi(handle) -> f32`; a Rust test sets the metadata to 300 and reads it back for AC-23) and its wrapper in `ui/src/modes/image/core/wasm.ts`, read on tab switch and on every `editor.revision` bump. Other specs reuse it rather than adding a second one.

**Menus and keys.** View menu in `ImageEditor.svelte` (`id: 'view'`); `MenuSubmenu` with `checked` items as in `languageMenu()` (`ui/src/lib/i18n/locale.ts`). `togglePixelGrid` already exists in `ui/src/lib/shell/shortcuts.ts` (Ctrl+Shift+', alternate Ctrl+"); Image mode binds it. Ctrl+R / Ctrl+Shift+R are the desktop-only rotations there and browser reloads on the web; the new `toggleRulers` action follows the same `DESKTOP ? [...] : []` pattern with `{ key: 'r', code: 'KeyR', mod: true, alt: true }` (Ctrl+Alt+R is AltGr+R on Windows, hence the physical key).

**Persistence.** One IndexedDB pref, e.g. `imageView` = `{ showRulers, showPixelGrid, units }`, via `getPref` / `setPref` (`ui/src/lib/idb/prefs.ts`), parsed defensively as `parseGridPref` does in `ui/src/modes/pixel/grid.ts`.

**e2e.** Proposed `ui/e2e/image-view-aids.spec.ts`; zoom via the `+` key (×1.25 per press) and the zoom readout as in `ui/e2e/viewport.spec.ts`; `storedPref` and `waitForEditor` from `ui/e2e/helpers.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Zoom exactly 400 % | Pixel grid drawn |
| Zoom drops from 477 % to 381 % with the pixel grid on | Grid disappears; the menu entry stays checked |
| Minimum zoom (2 %) on a 20000 px wide image | Labels thin out to a large step (e.g. 2000 px) and still never overlap |
| Maximum zoom (6400 %) in Inches at 96 DPI | Step at least 0.01 in; neighbouring labels stay distinct after rounding to two decimals |
| Pointer over a ruler or the ruler corner | Treated as outside the canvas area: no markers, no status-bar position |
| Pointer left of / above the image | Negative position in the status bar and the markers, e.g. `-3, 12 px` |
| Toggle UI (Tab) hides rail, options bar and panels | Rulers and pixel grid stay as they are — they belong to the canvas view |
| Ctrl+Shift+' while typing in a live text (`image-live-text`) | The pixel grid toggles as on the canvas (`image-live-text` AC-23); no character is typed, the text stays open and unchanged |
| Phone width (< 640 px) | Rulers shown when on; the image fits into the remaining area |
| Unit Inches with a 96 DPI document, pointer on pixel 1 | Status bar shows `0.01 in` for x (German: `0,01 Zoll`) |
| Touch pan / pinch with two fingers | Position and markers follow the first contact and hide when it lifts; a contact still down does not take over, the next gesture's first contact is followed again |
| IndexedDB blocked (private window) | Toggles work for the session; the next launch shows the defaults |
| Pixel tab and Image tab open, Ctrl+Shift+' pressed in the Image tab | Only the Image pixel grid toggles; Pixel mode's pixel grid keeps its state |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Pixel grid threshold 400 % (4 CSS px per image px), not Pixel mode's 8 | The owner brief names 400 %; photo and screenshot work rarely zooms to 800 %, and one-device-pixel lines still leave three of four pixel rows visible | Low |
| Rulers and pixel grid off, unit Pixels on a fresh profile (AC-13) | paint.net's defaults; §17.3's "on by default" pixel grid is a Pixel-mode mapping (Piskel #636) | Low |
| On / off states and unit are per user for Image mode, not per document | paint.net stores them as app settings; they are view state, and a per-document store would need a keyed registry for cosmetic data | Low |
| Ruler origin is the image's top-left corner, labels in the chosen unit | paint.net behaves so; guides (which could move an origin) are out of scope | Low |
| Image size in the status bar also follows the unit | paint.net shows sizes in the chosen unit; one unit per view avoids mixed readouts | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/image-view-aids.spec.ts` "rulers: toggle, labels at their coordinates and Fit" | built | c2d5349 |
| AC-02 | `ui/e2e/image-view-aids.spec.ts` "rulers: toggle, labels at their coordinates and Fit"; `ui/e2e/image-view-aids.spec.ts` "units switch the rulers and the status bar, in either language"; `ui/e2e/view-units.spec.ts` "AC-02: ticks sit where their image coordinate lies" | built | c2d5349 |
| AC-03 | `ui/e2e/image-view-aids.spec.ts` "ruler labels never overlap from the minimum to the maximum zoom, in German too"; `ui/e2e/image-view-aids.spec.ts` "rulers: toggle, labels at their coordinates and Fit"; `ui/e2e/view-units.spec.ts` "AC-03: labels step by 1, 2 or 5 × 10ⁿ…" | built | c2d5349 |
| AC-04 | `ui/e2e/image-view-aids-pointer.spec.ts` "a hovering mouse: …", "a pressed mouse follows the drag…", "a pen: hover shows…", "touch › a touch shows while down…", "a zoom by key without a pointer move…", "a middle-button pan…", "a tab switch under a resting mouse…", "desktop › Ctrl+Alt+R under a resting mouse…" | built | bb1c248, 829dfa5 |
| AC-05 | `ui/e2e/image-view-aids.spec.ts` "rulers: toggle, labels at their coordinates and Fit", "the rulers stay at phone width and the image fits beside them (edge case)", "View ▸ Toggle UI keeps the rulers and the pixel grid (edge case)" | built | c2d5349, 3f90489f |
| AC-06 | `ui/e2e/image-view-aids.spec.ts` "pixel grid: View ▸ Pixel Grid, drawn from exactly 400 % inside the image only" | built | c2d5349 |
| AC-07 | `ui/e2e/image-view-aids.spec.ts` "pixel grid: View ▸ Pixel Grid, drawn from exactly 400 % inside the image only" (beside the image on all four sides) | built | c2d5349, 3389178b |
| AC-08 | `ui/e2e/image-view-aids.spec.ts` "pixel grid: View ▸ Pixel Grid, drawn from exactly 400 % inside the image only" | built | c2d5349 |
| AC-09 | `ui/e2e/image-view-aids.spec.ts` "pixel grid: its backing store follows a device-pixel-ratio change without a resize" (backing store only; the look stays manual) | open | 17550dc |
| AC-10 | `ui/e2e/image-view-aids-pointer.spec.ts` "a hovering mouse: position and markers inside, beside and past the image, none outside", "rulers off on a fresh profile…", "touch › rulers off: a touch shows the position while down…", "a zoom by key without a pointer move…", "a middle-button pan…", "a tab switch under a resting mouse…", "desktop › Ctrl+Alt+R under a resting mouse…" | built | bb1c248, 829dfa5, c0c5451 |
| AC-11 | `ui/e2e/image-view-aids.spec.ts` "units switch the rulers and the status bar, in either language" | built | c2d5349 |
| AC-12 | `ui/e2e/view-units.spec.ts` "AC-12: Pixels are whole…"; `ui/e2e/image-view-aids.spec.ts` "units switch the rulers and the status bar, in either language" | built | c2d5349 |
| AC-13 | `ui/e2e/image-view-aids.spec.ts` "rulers: toggle, labels at their coordinates and Fit" | built | c2d5349 |
| AC-14 | `ui/e2e/image-view-aids-state.spec.ts` "view state persists per user, applies to every Image tab and is no edit"; `ui/e2e/image-view-aids-state.spec.ts` "a toggle made before the stored state loaded wins and is stored with it" (27b5ab6a); `ui/e2e/image-view-aids-state.spec.ts` "Pixel mode keeps its grids when an Image tab toggles its own"; `ui/e2e/image-view-aids-state.spec.ts` "opened and saved files › a document saved with every aid on is byte-identical to one saved with all off" | built | c2d5349 |
| AC-15 | `ui/e2e/image-view-aids-state.spec.ts` "view state persists per user, applies to every Image tab and is no edit" | built | c2d5349 |
| AC-16 | `ui/e2e/image-view-aids.spec.ts` "pixel grid: View ▸ Pixel Grid, drawn from exactly 400 % inside the image only", "Ctrl+Shift+' in a live text toggles the pixel grid and leaves the text alone" | built | c2d5349, 3389178b |
| AC-17 | `ui/e2e/image-view-aids.spec.ts` "web: Ctrl+Alt+R has no default…", "desktop › Ctrl+Alt+R toggles the rulers by the physical R key and rotates nothing" | built | c2d5349 |
| AC-18 | `ui/e2e/image-view-aids.spec.ts` "web: Ctrl+Alt+R has no default and the shortcut list is clean", "desktop › Ctrl+Alt+R toggles the rulers by the physical R key and rotates nothing" | built | c2d5349, b027d5f |
| AC-19 | `ui/e2e/image-view-aids-state.spec.ts` "a tab switch moves the rulers and the pixel grid to that tab’s view", "another resolution › the rulers and the status bar take the resolution again on every tab switch and document change", "view state persists per user, applies to every Image tab and is no edit"; `ui/e2e/image-view-aids-pointer.spec.ts` "a tab switch under a resting mouse…" | built | c2d5349, 829dfa5 |
| AC-20 | `ui/e2e/view-units.spec.ts` "AC-20: a missing, malformed or unknown pref falls back to the defaults"; `ui/e2e/image-view-aids-state.spec.ts` "a malformed pref or a failing store keeps the editor working" | built | c2d5349 |
| AC-21 | `ui/e2e/view-units.spec.ts` "AC-21: a broken resolution counts as 96 DPI" | built | c2d5349 |
| AC-22 |  | dropped |  |
| AC-23 | `crates/pincel-image-wasm/src/lib.rs` `document_dpi_reports_the_stored_resolution`; `ui/e2e/image-view-aids-state.spec.ts` "opened and saved files › an opened PNG counts 96 DPI whatever resolution the file declares", "another resolution › the rulers and the status bar take the resolution again on every tab switch and document change"; `ui/e2e/image-view-aids.spec.ts` "units switch the rulers and the status bar, in either language" (96 DPI) | built | c2d5349 |
| AC-24 | `ui/e2e/image-view-aids-pointer.spec.ts` "a hovering mouse: …", "a pressed mouse follows the drag…", "a pen: hover shows…", "touch › a touch shows while down…", "touch › lifting the first of two contacts hides position and markers", "touch › rulers off: a touch shows…", "a middle-button pan…", "rulers off on a fresh profile…" | built | bb1c248, 829dfa5, c0c5451, 96dcc028 |
| AC-25 | `ui/e2e/image-view-aids-state.spec.ts` "a malformed pref or a failing store keeps the editor working", "without IndexedDB › the defaults apply and the editor works" | built | c2d5349 |

## Decisions

- 2026-11-05: Image mode's pixel grid reuses the `togglePixelGrid` action (Ctrl+Shift+') — §9.4: the same key means the same thing in both modes; Ctrl+' (Pixel mode's custom grid) stays unbound in Image mode because Image mode has no custom grid.
- 2026-11-05: Units are chosen in View ▸ Units only — the command palette lists menu entries, so no second control is needed.
- 2026-11-05: All three aids are view state drawn over the canvas: no core command, no undo entry, nothing in any file, no MCP tool — the document stays in Rust and the canvas is render-only (AGENTS.md §9, Decision Log 2026-11-05 "MCP tool parity").
- 2026-11-05 (owner's choice): Rulers toggle with Ctrl+Alt+R (⌥⌘R on macOS), matched by the physical R key, in the desktop app only; the web build has no default chord (bindable in the keymap) — because paint.net's Ctrl+R is Pincel's desktop Rotate 90° CW and a browser reload, ⌘⌥R is Safari's reload from origin, and desktop-only chords already have a precedent in the rotations.
- 2026-11-05 (owner's choice): This spec adds a read-only wasm getter for `DocumentMetadata::dpi`, and `image-resize-autocrop` reuses it — because one getter keeps this spec independent of the resize work, and units follow the resolution as soon as another spec edits or loads it.
- 2026-11-05 (clarification): Ruler labels start just past their tick (0–4 CSS px) and the tick marks the coordinate — paint.net's layout, and a test can measure tick and label separately.
- 2026-11-05 (clarification): The pixel grid is drawn only inside the image's bounds — lines beside the image would mark no pixels.
- 2026-11-05 (clarification): Unit suffixes and the decimal separator follow the UI language (`Zoll` and a comma in German) — §9.5 translates every user-visible string; ruler labels stay bare numbers to save width.
- 2026-11-05 (clarification): AC-22 dropped — with document tabs no Image editor exists without a document.
- 2026-11-05 (clarification): Touch has no hover, so position and markers show only while a contact is down (AC-24) — the only moment a touch pointer has a position.
- 2026-11-05 (clarification): A failed pref write is silent and the change stays for the session (AC-25) — the other view prefs (`pixelGrid`, `keymap`) behave so, and a cosmetic setting is not worth an error.
- 2026-11-06 (spec-build): The status bar shows the image size with the unit suffix in every unit, `40 × 30 px · sRGB` in Pixels (AC-12); the e2e specs that read the size were updated to the new text.
- 2026-11-06 (spec-build): Ruler labels lie at least 60 CSS px apart (not just 40), so the longest label (`-1234.56`) never reaches the next one (AC-03).
- 2026-11-06 (spec-build): The rulers live in a wrapper around the canvas (`ImageViewport.svelte`), which also tracks the pointer; the canvas itself gained only the pixel-grid layer below the marching ants.
- 2026-11-06 (spec-build): AC-09 (manual) is not checked; it needs a human look at the pixel grid on real displays.
- 2026-11-06 (rework after `/spec-check`): A pressed mouse that left the canvas area kept the pointer position and the markers — the tool's pointer capture keeps its moves coming and holds back `pointerleave`. Fixed in `ImageViewport.svelte`: a tracked move outside the area's rectangle hides both (AC-24, regression `image-view-aids-pointer.spec.ts`).
- 2026-11-06 (rework): AC-23's "taken again on every tab switch and every document change" is proved in e2e without a product seam: the spec wraps the image core's `document_dpi` export when the page instantiates the wasm module (test-only `addInitScript`) and reports a chosen resolution per document handle. A tab switch is itself a document change (`activateTab` bumps `editor.revision`), so a test cannot tell the two triggers apart; it proves both by reading after a switch and after a stroke. The clause stays as written.
- 2026-11-06 (rework): Exactly 400 % is reached through Fit in a window sized so the canvas area is 4 × the image height + 32 px (Fit's padding); 399 % one pixel less. `+` (×1.25) never lands on 400 %.
- 2026-11-06 (rework): Label overlap (AC-03) depends on rendered text width, so the e2e zoom sweep (2 % → 6400 %, German, cm / in / px, a 20000 px image for the widest labels) proves it; the Node test only pins the 40 px minimum and the 1 / 2 / 5 × 10ⁿ steps.
- 2026-11-06 (rework, red runs): each new test failed against a temporary break and passed after reverting it — label text doubled (AC-02), `RULER_MIN_SPACING` 30 and a 30 px label font (AC-03), no re-fit on resize (AC-05), a no-op Pixel Grid menu entry (AC-06), threshold 4.01 and 3.98 (AC-07, AC-08), grid lines across the whole canvas (AC-07), status clamped to the image (AC-10), Pixels always checked and inch ticks at 1.1 × dpi (AC-11), an Image toggle writing the `pixelGrid` pref and a save whose bytes depend on the grid (AC-14), physical-key matching dropped (AC-17), the grid threshold ignored (AC-19), the resolution read once / not on a revision / fixed at 300 (AC-19, AC-23), touch lift, last contact and pen leave not hiding (AC-24), the stored pref ignored on load and the read unguarded (AC-25).
- 2026-11-06 (rework after the second `/spec-check`): the canvas area kept the pointer as an image coordinate taken at event time, so a zoom by key, Fit, a pan (the area's capture handler runs before the canvas moves the view), a pinch, a tab switch or the rulers toggling under a resting pointer left the status bar and the markers stale (AC-04, AC-10, AC-19, AC-24). The area now keeps the pointer in its own screen space and re-places it when it moves or resizes; the status bar maps it through the view current at render time. A pinch therefore shows the pixel under the first contact through the moving view, not the pixel it first touched — the touch test asserted the latter and was corrected.
- 2026-11-06 (rework): the pixel grid re-measures on a device-pixel-ratio change (a `resolution` media query, re-armed per ratio), not only on a CSS resize (AC-09). CDP's ratio emulation fires no media-query `change` event, so the e2e test records the page's resolution queries and fires `change` on those that stopped matching, as a real monitor move does; it checks the backing store's ratio, while crispness stays a manual check and AC-09 stays open.
- 2026-11-06 (rework, red runs): the pointer-in-image-space code failed the zoom-by-key, middle-pan, tab-switch and pinch tests; dropping the re-placement on resize failed the Ctrl+Alt+R test; a status position shown only with the rulers on failed both rulers-off tests; dropping the ratio listener failed the device-pixel-ratio test; Pixel Grid bound to Ctrl+Shift+; instead of Ctrl+Shift+' failed both AC-18 tests. Each passed after the revert.
- 2026-11-06 (rework after the third `/spec-check`; decided by the lead session under the owner's ask-at-the-end instruction; listed for review): the edge case "Ctrl+Shift+' while typing in the Text tool's entry box → no toggle; the key goes to the text field" predates the live text. `image-live-text` AC-23 runs every Ctrl / ⌘ / Alt chord the text does not own as on the canvas, so the edge case now follows it: Ctrl+Shift+' types no character and toggles the pixel grid, which is no document command and so commits nothing. Test "Ctrl+Shift+' in a live text toggles the pixel grid and leaves the text alone" was green at once and red against a temporary break (the toggle skipped while focus is in the live text), reverted afterwards.
- 2026-11-06 (rework, fix): lifting the first of two touch contacts left the position and the markers frozen: the canvas area followed only the first contact but hid them only once every contact had lifted. Chosen behaviour: they hide when the followed (first) contact lifts; a contact still down never takes over, and the first contact of the next gesture is followed again — the remaining finger belongs to a pan or pinch that is ending, and following it would make the position jump. The edge case "Touch pan / pinch with two fingers" says so now. Regression test "touch › lifting the first of two contacts hides position and markers" was red before the fix; red after it against a temporary break that let the remaining contact take over, reverted. CDP note for such tests: a `touchMove` that omits a point does not release it; a `touchEnd` that lists points releases just those.
- 2026-11-06 (rework, fix): a toggle made before the stored `imageView` pref was read was overwritten by the stored value, while its own write had stored the defaults for the other fields. Now a field changed while the read is pending keeps its value, the other fields come from the store, and the merged state is stored again. Regression test "a toggle made before the stored state loaded wins and is stored with it" (holds the editor's first `imageView` read back in the page) was red before the fix; red after it against a temporary break that skipped storing the merge, reverted.
- 2026-11-06 (rework, red runs): AC-07's "nothing beside the image" compared only the regions right of and below the image; it now compares left of and above it too (Fit centres the narrow test image, so all four lie in the canvas area). Red against a temporary break that drew the grid three cells left of and above the image (the left region differed), reverted. The edge-case tests "View ▸ Toggle UI keeps the rulers and the pixel grid" and "the rulers stay at phone width and the image fits beside them" were green at once and red against temporary breaks — rulers hidden while the UI is hidden or below 640 px (both tests), the grid not drawn while the UI is hidden — reverted afterwards.
