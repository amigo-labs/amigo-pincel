---
status: todo
created: 2026-10-28
size: L
mode: both
design: [pincel.md §9.2, pincel.md §1.3, pincel.md §15]
---

# Own color picker: HSV, alpha, hex, recent colors

## Context

Clicking the foreground or background swatch today opens the browser's or OS's native color picker (`<input type="color">`). It looks different on every platform, sometimes opens as a separate modal window, and **cannot set alpha**. In Image mode color alpha can only be faked with the tool's opacity slider. paint.net, Aseprite, Photopea and Pixelorama all ship their own HSV picker with alpha.

Pincel gets its own picker, the same in both modes and on every OS, opened in a popover from each swatch: a saturation/value area, a hue slider, an alpha slider, a hex field that also takes `#RRGGBBAA`, R / G / B / A number fields and a row of recent colors. It replaces the native color input.

**Revision 2026-11-05.** The owner wants to see a palette in Image mode without opening the picker, with the recently used colors in it (owner: "I also want to see a color palette in Image mode, the recently used colors"). Image mode's Colors panel shows a row of recent colors and a fixed default palette below it, as paint.net's Colors window does. A color counts as used when a tool paints with it, not when it is picked; this rule holds for the picker's recent row in both modes too, so both rows show the same list.

## Scope

### In scope

- A popover color picker opened from the foreground and from the background swatch, in both modes
- Saturation/value area, hue slider, alpha slider, hex field, R / G / B / A fields, recent colors row
- In Image mode's Colors panel, always visible: the recent colors row and a default palette of 48 colors; a left click sets the foreground color, a right click the background color
- Mouse, touch, pen and keyboard input; live preview while dragging, one commit per interaction
- Foreground and background alpha in both modes (Image mode gains color alpha)
- Recent colors (12, per mode) and the last foreground / background color with alpha persisted per mode
- The Eyedropper sets hex and alpha in both modes
- en + de strings; §9.2 Colors paragraph updated

### Out of scope

- Color harmonies, a color wheel and named swatches — because the goal is parity with the baseline pickers (HSV + alpha + hex + recent); these are extras beyond it.
- Wide-gamut or HDR colors — because they are a Phase-1 non-goal (§1.3).
- Changes to the Pixel palette panel — because the palette stays as it is; recent colors don't replace it.
- An always-visible recent colors row in Pixel mode — because the owner wants it in Image mode only; Pixel mode has its palette panel, and its recent colors stay in the picker
- Editing, saving or loading the Image-mode palette — because `image-palette-gradient` owns it (load, save, reset, add color, kept in IndexedDB); this spec only shows the default palette
- Sampling with the picker open — because the popover stays modal; a non-modal popover over the canvas needs pointer routing and focus rules for little gain, and the picker shows the sampled color the next time it opens (AC-22).

## Acceptance criteria

- [ ] AC-01 (event): When the user clicks the foreground swatch, then the color picker opens as a popover next to it and edits the foreground color; clicking the background swatch opens it for the background color — in Pixel and in Image mode.
- [ ] AC-02 (always): The picker offers a saturation/value area, a hue slider, an alpha slider, a hex field, R, G, B and A number fields (0–255) and a recent-colors row, each with an accessible name.
- [ ] AC-03 (always): Clicking a swatch never opens the browser's or OS's native color dialog; the picker is the same in both modes.
- [ ] AC-04 (event): When the user types `#11223380` into the hex field and presses Enter, then the edited color becomes R 17, G 34, B 51, A 128, and the R / G / B / A fields show those values.
- [ ] AC-05 (always): The hex field accepts `RRGGBB`, `#RRGGBB`, `#RRGGBBAA` and `#RGB` (case-insensitive); `#abc` sets the same RGB as `#aabbcc`.
- [ ] AC-06 (error): If the user commits hex text that is none of the accepted forms (e.g. `#12345`, `zzz`, empty), then the edited color stays unchanged.
- [ ] AC-07 (error): If the user enters a value below 0 or above 255 in an R / G / B / A field, then the value is clamped to 0–255 before it is applied.
- [ ] AC-08 (event): When the user drags in the saturation/value area or on the hue or alpha slider, then the swatch and the color the tools paint with update during the drag; picking a color does not change the recent-colors row (AC-24).
- [ ] AC-09 (always): The saturation/value area and the sliders can be dragged with mouse, touch and pen.
- [ ] AC-10 (event): When the saturation/value area or a slider has keyboard focus and the user presses an arrow key, then the value moves by one step in that direction; with Shift held, by a larger step.
- [ ] AC-11 (always): Moving the color through black (value 0) or grey (saturation 0) and back does not reset the hue: the hue slider keeps its position.
- [ ] AC-12 (always): The recent-colors row holds at most the last 12 colors painted with (AC-24), most recent first, without duplicates; painting with a color already in the row moves it to the front. Two colors are duplicates when their R, G, B and A are equal.
- [ ] AC-13 (event): When the user clicks a color in the recent-colors row, then the edited color (foreground or background) becomes that color, including its alpha.
- [ ] AC-14 (state): While in one mode, the recent-colors row shows only colors painted with in that mode: a color painted with in Pixel mode does not appear in Image mode's row.
- [ ] AC-15 (event): When the app is reloaded, or the desktop app is quit and started again, then each mode's recent colors (in order) and its last foreground and background color, including alpha, are restored, in the picker and in Image mode's Colors panel.
- [ ] AC-16 (event): When the user paints in Pixel mode with foreground `#11223380` on a transparent layer and samples that pixel with the Eyedropper, then the foreground becomes `#11223380` (A field shows 128).
- [ ] AC-17 (event): When the user paints in Image mode on a transparent layer with foreground alpha 128 and tool opacity 100 %, then the Eyedropper sampling that layer yields alpha 128; with tool opacity 50 % it yields alpha 64 (±1).
- [ ] AC-18 (event): When the user presses Escape, the popover closes and uncommitted field text is discarded; when the user clicks outside, valid uncommitted field text is committed first, then the popover closes.
- [ ] AC-19 (always): The swap and reset buttons next to the swatches keep working: swap exchanges foreground and background including their alpha; reset sets opaque black foreground and opaque white background.
- [ ] AC-20 (always): The popover lies fully inside the window, also at a 360 px wide viewport and when the swatch sits near a window edge.
- [ ] AC-21 (option): Where the UI language is German, the picker's labels (hue, saturation, value, alpha, recent colors, hex) are German.
- [ ] AC-22 (event): When the Eyedropper changes the color while the picker is closed, then the next time the picker opens it shows the sampled color and alpha.
- [ ] AC-23 (event): When the user commits a hex with 3 or 6 digits, then the alpha stays unchanged; when it has 8 digits, the alpha is set from the last pair.
- [ ] AC-24 (event): When a tool commits a change that alters at least one pixel, then every color the change painted with is added to the mode's recent colors (AC-12), the foreground color first when both were used: the color a Brush, Pencil, Line, shape, Paint Bucket, Text or Gradient stroke actually applied, which is the foreground or, for a right-button gesture or a shape's fill, the background color as the tool uses it today. In an indexed Pixel document the entry's RGBA color is recorded. Erasers (to transparency or to the background), the selection tools, the Eyedropper, effects and adjustments add nothing, because they do not paint with a chosen color; a change that alters no pixel (locked or hidden layer, stroke outside the canvas or selection, refused tool) adds nothing.
- [ ] AC-25 (always): In Image mode the Colors panel always shows, below the foreground and background swatches, the recent-colors row of AC-12 and the Image-mode palette — the default palette of 48 colors (AC-27) until the user loads or edits one (`image-palette-gradient`); both are visible without opening the picker, also while the picker is open.
- [ ] AC-26 (event): When the user left-clicks a color in Image mode's recent-colors row or default palette, then the foreground color becomes that color, including its alpha; when the user right-clicks it, or long-presses it with touch or pen (the platform's context-menu gesture, as the tool rail uses it; touch and pen have no right button), then the background color does, and no context menu opens.
- [ ] AC-27 (always): The default palette holds, in this order, 12 greys with each channel equal to round(i × 255 / 11) for i = 0 … 11, then for each hue 0°, 30°, …, 330° the fully saturated color at value 100 %, then the same 12 hues at value 70 %, then at value 40 % (HSV); every channel is rounded to the nearest integer with halves rounded up (178.5 → 179), and every color is opaque.
- [ ] AC-28 (option): Where the UI language is German, the recent-colors row and the default palette have German accessible names, and each color's accessible name gives its hex value.
- [ ] AC-29 (state): While the recent-colors row is empty (no color painted with yet in that mode), Image mode's Colors panel shows the row's label and an empty area as high as one row of swatches, so the default palette does not jump when the first color arrives.
- [ ] AC-30 (error): If a stored recent-colors list or a stored foreground / background color cannot be read (missing, not a list of colors, malformed entries), then the mode starts with an empty recent-colors row, or with opaque black foreground and opaque white background, keeping every entry that is readable; no error is shown.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**`ui/src/lib/components/ColorPair.svelte`** (shared by both modes):
- Props: `fg`, `bg` (`#rrggbb`), `onFg`, `onBg`, `onSwap`, `onReset`, optional `alpha` / `onAlpha`; the alpha slider only shows when both are passed.
- The swatches are two overlapping `<input type="color">` (≈L39, ≈L47) — these get replaced.
- The hex field edits the fg only, as `#RRGGBB`. No background-alpha UI.

**`ui/src/lib/color.ts`**: `packColor(hex, alpha)` → `0xRRGGBBAA` (≈L7), `unpackColor(rgba)` → `#RRGGBB` dropping alpha (≈L17). No HSV helpers. Image mode has its own `hexToRgb` / `rgbToHex` (used in `ui/src/modes/image/core/controller.ts`) — consolidate rather than add a third copy. Plan design: `hexToRgba`, `rgbaToHex(withAlpha)`, `rgbToHsv`, `hsvToRgb`, `parseColor(text)`; keep hue and saturation in picker state when value or saturation is 0 (AC-11).

**Pixel mode:**
- `colors = { color, alpha, bgColor, bgAlpha }` via `colorsInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L107, `$state` ≈L192). The core `Rgba` already takes alpha through `packColor`.
- The Eyedropper writes color and alpha (`ui/src/modes/pixel/pointer.ts` ≈L139).
- Palette left-click sets fg, right-click bg (`PixelEditor.svelte`).
- Prefs `lastColor` / `lastBgColor` (`PixelEditor.svelte` ≈L162–163) persist hex only, not alpha. Either store alpha alongside or switch to `#RRGGBBAA`; old `#RRGGBB` values must still load.

**Image mode:**
- `tool.foreground` / `tool.background` are `#rrggbb` with no alpha (`ui/src/modes/image/stores/editor.svelte.ts` ≈L194–196); opacity is the separate `tool.opacity` (1–100 %).
- `colorWithOpacity(hex)` (`ui/src/modes/image/core/controller.ts` ≈L63) bakes opacity into the RGBA it sends — decided (2026-10-29): painted alpha = round(color alpha × `tool.opacity` / 100), rounded to the nearest integer, computed there.
- `sampleColor` (`controller.ts` ≈L798) drops alpha today (`rgbToHex(r, g, b)`) — must keep it for AC-17. Its `tool.eyedropperSample` decides layer vs. image sampling; AC-17's test must sample the painted transparent layer, not a composite over an opaque background.
- `ColorsPanel.svelte` is at `ui/src/modes/image/components/panels/ColorsPanel.svelte`; it passes no alpha props. Image colors are not persisted today.

**Popover.** No generic popover exists. Pattern: the ⋯ menu of `ui/src/modes/pixel/components/PalettePanel.svelte` — fixed position from `getBoundingClientRect()` clamped to the window, a transparent backdrop button that closes it, Escape closes it.

**Picker body.** Plan design: `ui/src/lib/components/color/ColorPicker.svelte`, mode-agnostic, props `value: { hex; alpha }`, `recent`, `onChange`, `onCommit`. CSS-only: the SV area is a div with the hue as background plus two linear gradients; hue and alpha tracks are gradients, alpha over a checkerboard. No `<canvas>`, so nothing can ever be read back (AGENTS.md §9). Pointer capture for drags.

**Recent colors.** IndexedDB prefs `recentColors:pixel` / `recentColors:image` (`ui/src/lib/idb/prefs.ts` `getPref` / `setPref`), never localStorage. Recent colors update when a paint change is committed (AC-24); the foreground / background prefs update when the picker commits. Colors are not document state — no undo entries.

**i18n.** `color.*` keys in `ui/src/lib/i18n/en.ts`; plan names `color.picker`, `color.recent`, `color.hue`, `color.saturation`, `color.value`.

**Existing tests that touch the swatches.** `ui/e2e/pencil.spec.ts` and `ui/e2e/shortcuts.spec.ts` drive `getByLabel('Foreground color')` with `fill()` / `toHaveValue()` on the native input; they must move to the new control (e.g. the hex field) in the same change. New e2e `ui/e2e/color-picker.spec.ts` for both modes (open from fg swatch, type `#11223380`, commit, paint, Eyedropper read-back, recent row, reload).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A stroke undone after it added a color to the recent row | The color stays in the row; the row records use, not document state |
| A brush stroke with tool opacity 50 % | The foreground color (with its own alpha) is added, not the blended result |
| Several strokes in a row with the same color | The row does not change after the first |
| Narrow sidebar (240 px) | The default palette and the recent row wrap into more rows; every swatch stays at least 16 px, because smaller targets are hard to hit and their colors hard to tell apart |
| Stored pref from before this feature (`#RRGGBB`, no alpha) | Loads as that color with alpha 255 |
| Same color painted with twice in a row | Recent row still holds it once, at the front |
| 13th distinct color painted with | The oldest drops out; the row holds 12 |
| Drag ends outside the popover or the window | Counts as release: one commit, popover stays open |
| Picker opened for the background color | Changes affect only the background; foreground unchanged |
| Alpha 0 committed | Valid color; swatch shows the checkerboard |
| Phone width (360 px) | Popover stays fully on screen (AC-20) |
| Hex typed but popover closed without Enter | Escape discards the text; an outside click commits it if valid, drops it if invalid (AC-18, AC-06) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| R / G / B / A fields are integers 0–255 | Matches `#RRGGBBAA` and the core's `Rgba` | Low |
| Arrow-key steps: 1 unit, Shift = 10 units (hue in degrees, saturation / value in percent, alpha 0–255) | The plan says "Shift makes the steps bigger" without numbers | Low |
| Reset (D) gives opaque black and white | Default colors are opaque in both modes today | Low |
| A drag that changes the color live without release (e.g. pointer cancel) commits the last previewed color | The plan specifies only "one commit when the drag ends" | Medium |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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
| AC-08 |  | open |  |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |  | open |  |
| AC-13 |      | open   |        |
| AC-14 |  | open |  |
| AC-15 |  | open |  |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |
| AC-23 |      | open   |        |
| AC-24 |  | open |  |
| AC-25 |  | open |  |
| AC-26 |  | open |  |
| AC-27 |  | open |  |
| AC-28 |  | open |  |
| AC-29 |  | open |  |
| AC-30 |  | open |  |

## Decisions

- 2026-11-05: AC-25 shows "the Image-mode palette", which is the default palette until the user replaces it; loading, saving and editing it moved to `image-palette-gradient` (owner chose that spec on 2026-11-05).
- 2026-11-05: AC-15 names the desktop app's quit and restart explicitly and covers Image mode's Colors panel — the owner asked for the last colors to survive program starts.
- 2026-11-05: Revision. Image mode's Colors panel shows the recent colors and a 48-color default palette without opening the picker; left click sets the foreground, right click (or long press) the background. Recent colors are now the colors painted with, not the colors committed in the picker, in both modes, so the picker row and the panel row show one list. Changed: AC-08, AC-12, AC-14; new: AC-24 – AC-29. Requested by the owner, who chose "recent + default palette", "when painting" (as paint.net), "left = foreground, right = background" and "Image mode only" for the always-visible row. The default palette's composition (AC-27) is our own, generated from HSV, so it is reproducible and copies no other product's palette.
- 2026-10-28 (legacy plan): The picker is CSS-only (gradients, no `<canvas>`) — nothing can be read back from it, and no new dependency is needed.
- 2026-10-28 (legacy plan): Live preview while dragging, one commit at the end — recent colors and prefs would otherwise fill up with intermediate colors. (Since the 2026-11-05 revision, picking no longer adds to the recent colors; the single commit still keeps the prefs from churning.)
- 2026-10-28 (legacy plan): Recent colors: last 12, most recent first, de-duplicated, per mode in IndexedDB — localStorage is forbidden (AGENTS.md §9) and the two modes use colors differently.
- 2026-10-28 (legacy plan): Pixel mode keeps its palette panel — recent colors complement it, they don't replace it.
- 2026-10-29: A Decision Log entry records the own CSS-only picker (HSV area, hue / alpha sliders, `#RRGGBB[AA]` hex, RGBA fields, 12 recent colors per mode in IndexedDB) replacing `<input type="color">` in both modes, with fg / bg alpha persisted per mode, and `pincel.md` §9.2 Colors gains one sentence — AGENTS.md §2.1 requires the entry before code, and one entry also covers the Image-mode alpha rule (owner accepted the recommendation).
- 2026-10-29: In Image mode the painted alpha is the color's alpha × `tool.opacity` / 100, rounded to the nearest integer, in `colorWithOpacity` — it is paint.net's model, keeps the opacity slider meaningful, and AC-17 already encodes it (owner accepted the recommendation).
- 2026-10-29: A 3- or 6-digit hex keeps the current alpha; only an 8-digit hex sets alpha (AC-23) — hex codes pasted from palettes are RGB-only, alpha has its own slider and field, and Figma behaves the same way (owner accepted the recommendation).
- 2026-10-29: The popover stays modal and reads the current fg / bg each time it opens, so the Eyedropper's color shows on the next open (AC-22); sampling with the picker open is out of scope — a non-modal popover over the canvas needs pointer routing and focus rules for little gain (owner accepted the recommendation).
- 2026-10-29: Fields commit on Enter and on blur (`change`); an outside click blurs and so commits valid text, invalid text is dropped as in AC-06, and Escape discards uncommitted text (AC-18) — this matches native input `change` semantics and keeps Escape's meaning of cancel (owner accepted the recommendation).
