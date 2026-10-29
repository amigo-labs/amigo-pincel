---
status: todo
created: 2026-10-28
size: L
mode: both
design: [pincel.md §9.2, pincel.md §1.3]
---

# Own color picker: HSV, alpha, hex, recent colors

## Context

Clicking the foreground or background swatch today opens the browser's or OS's native color picker (`<input type="color">`). It looks different on every platform, sometimes opens as a separate modal window, and **cannot set alpha**. In Image mode color alpha can only be faked with the tool's opacity slider. paint.net, Aseprite, Photopea and Pixelorama all ship their own HSV picker with alpha.

Pincel gets its own picker, the same in both modes and on every OS, opened in a popover from each swatch: a saturation/value area, a hue slider, an alpha slider, a hex field that also takes `#RRGGBBAA`, R / G / B / A number fields and a row of recent colors. It replaces the native color input.

## Scope

### In scope

- A popover color picker opened from the foreground and from the background swatch, in both modes
- Saturation/value area, hue slider, alpha slider, hex field, R / G / B / A fields, recent colors row
- Mouse, touch, pen and keyboard input; live preview while dragging, one commit per interaction
- Foreground and background alpha in both modes (Image mode gains color alpha)
- Recent colors (12, per mode) and the last foreground / background color with alpha persisted per mode
- The Eyedropper sets hex and alpha in both modes
- en + de strings; §9.2 Colors paragraph updated

### Out of scope

- Color harmonies, a color wheel and named swatches — because the goal is parity with the baseline pickers (HSV + alpha + hex + recent); these are extras beyond it.
- Wide-gamut or HDR colors — because they are a Phase-1 non-goal (§1.3).
- Changes to the Pixel palette panel — because the palette stays as it is; recent colors don't replace it.

## Acceptance criteria

- [ ] AC-01 (event): When the user clicks the foreground swatch, then the color picker opens as a popover next to it and edits the foreground color; clicking the background swatch opens it for the background color — in Pixel and in Image mode.
- [ ] AC-02 (always): The picker offers a saturation/value area, a hue slider, an alpha slider, a hex field, R, G, B and A number fields (0–255) and a recent-colors row, each with an accessible name.
- [ ] AC-03 (always): Clicking a swatch never opens the browser's or OS's native color dialog; the picker is the same in both modes.
- [ ] AC-04 (event): When the user types `#11223380` into the hex field and presses Enter, then the edited color becomes R 17, G 34, B 51, A 128, and the R / G / B / A fields show those values.
- [ ] AC-05 (always): The hex field accepts `RRGGBB`, `#RRGGBB`, `#RRGGBBAA` and `#RGB` (case-insensitive); `#abc` sets the same RGB as `#aabbcc`.
- [ ] AC-06 (error): If the user commits hex text that is none of the accepted forms (e.g. `#12345`, `zzz`, empty), then the edited color stays unchanged.
- [ ] AC-07 (error): If the user enters a value below 0 or above 255 in an R / G / B / A field, then the value is clamped to 0–255 before it is applied.
- [ ] AC-08 (event): When the user drags in the saturation/value area or on the hue or alpha slider, then the swatch and the color the tools paint with update during the drag, and on release the color is committed once: the recent-colors row gains exactly one entry for the whole drag.
- [ ] AC-09 (always): The saturation/value area and the sliders can be dragged with mouse, touch and pen.
- [ ] AC-10 (event): When the saturation/value area or a slider has keyboard focus and the user presses an arrow key, then the value moves by one step in that direction; with Shift held, by a larger step.
- [ ] AC-11 (always): Moving the color through black (value 0) or grey (saturation 0) and back does not reset the hue: the hue slider keeps its position.
- [ ] AC-12 (always): The recent-colors row holds at most the last 12 committed colors, most recent first, without duplicates; committing a color already in the row moves it to the front.
- [ ] AC-13 (event): When the user clicks a color in the recent-colors row, then the edited color (foreground or background) becomes that color, including its alpha.
- [ ] AC-14 (state): While in one mode, the recent-colors row shows only colors committed in that mode: a color committed in Pixel mode does not appear in Image mode's row.
- [ ] AC-15 (event): When the app is reloaded, then each mode's recent colors and its last foreground and background color, including alpha, are restored.
- [ ] AC-16 (event): When the user paints in Pixel mode with foreground `#11223380` on a transparent layer and samples that pixel with the Eyedropper, then the foreground becomes `#11223380` (A field shows 128).
- [ ] AC-17 (event): When the user paints in Image mode on a transparent layer with foreground alpha 128 and tool opacity 100 %, then the Eyedropper sampling that layer yields alpha 128; with tool opacity 50 % it yields alpha 64 (±1).
- [ ] AC-18 (event): When the user presses Escape or clicks outside the popover, then the popover closes and the last committed color stays.
- [ ] AC-19 (always): The swap and reset buttons next to the swatches keep working: swap exchanges foreground and background including their alpha; reset sets opaque black foreground and opaque white background.
- [ ] AC-20 (always): The popover lies fully inside the window, also at a 360 px wide viewport and when the swatch sits near a window edge.
- [ ] AC-21 (option): Where the UI language is German, the picker's labels (hue, saturation, value, alpha, recent colors, hex) are German.

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
- `colorWithOpacity(hex)` (`ui/src/modes/image/core/controller.ts` ≈L63) bakes opacity into the RGBA it sends — plan recommendation: effective alpha = color alpha × tool opacity, computed there.
- `sampleColor` (`controller.ts` ≈L798) drops alpha today (`rgbToHex(r, g, b)`) — must keep it for AC-17. Its `tool.eyedropperSample` decides layer vs. image sampling; AC-17's test must sample the painted transparent layer, not a composite over an opaque background.
- `ColorsPanel.svelte` is at `ui/src/modes/image/components/panels/ColorsPanel.svelte`; it passes no alpha props. Image colors are not persisted today.

**Popover.** No generic popover exists. Pattern: the ⋯ menu of `ui/src/modes/pixel/components/PalettePanel.svelte` — fixed position from `getBoundingClientRect()` clamped to the window, a transparent backdrop button that closes it, Escape closes it.

**Picker body.** Plan design: `ui/src/lib/components/color/ColorPicker.svelte`, mode-agnostic, props `value: { hex; alpha }`, `recent`, `onChange`, `onCommit`. CSS-only: the SV area is a div with the hue as background plus two linear gradients; hue and alpha tracks are gradients, alpha over a checkerboard. No `<canvas>`, so nothing can ever be read back (AGENTS.md §9). Pointer capture for drags.

**Recent colors.** IndexedDB prefs `recentColors:pixel` / `recentColors:image` (`ui/src/lib/idb/prefs.ts` `getPref` / `setPref`), never localStorage. Recent colors and prefs update on commit only. Colors are not document state — no undo entries.

**i18n.** `color.*` keys in `ui/src/lib/i18n/en.ts`; plan names `color.picker`, `color.recent`, `color.hue`, `color.saturation`, `color.value`.

**Existing tests that touch the swatches.** `ui/e2e/pencil.spec.ts` and `ui/e2e/shortcuts.spec.ts` drive `getByLabel('Foreground color')` with `fill()` / `toHaveValue()` on the native input; they must move to the new control (e.g. the hex field) in the same change. New e2e `ui/e2e/color-picker.spec.ts` for both modes (open from fg swatch, type `#11223380`, commit, paint, Eyedropper read-back, recent row, reload).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Stored pref from before this feature (`#RRGGBB`, no alpha) | Loads as that color with alpha 255 |
| Same color committed twice in a row | Recent row still holds it once, at the front |
| 13th distinct color committed | The oldest drops out; the row holds 12 |
| Drag ends outside the popover or the window | Counts as release: one commit, popover stays open |
| Picker opened for the background color | Changes affect only the background; foreground unchanged |
| Alpha 0 committed | Valid color; swatch shows the checkerboard |
| Phone width (360 px) | Popover stays fully on screen (AC-20) |
| Hex typed but popover closed without Enter | See Open questions |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| R / G / B / A fields are integers 0–255 | Matches `#RRGGBBAA` and the core's `Rgba` | Low |
| Arrow-key steps: 1 unit, Shift = 10 units (hue in degrees, saturation / value in percent, alpha 0–255) | The plan says "Shift makes the steps bigger" without numbers | Low |
| Reset (D) gives opaque black and white | Default colors are opaque in both modes today | Low |
| A drag that changes the color live without release (e.g. pointer cancel) commits the last previewed color | The plan specifies only "one commit when the drag ends" | Medium |

## Open questions

- Decision Log entry not yet written: own CSS-only picker replacing `<input type="color">`, per-mode recent colors in IndexedDB, persisted alpha — confirm and add to `pincel.md` §15 together with the §9.2 Colors paragraph.
- Image-mode alpha rule (plan recommendation, needs the Decision Log entry): is the painted alpha = color alpha × tool opacity, as AC-17 assumes, paint.net-style?
- Does entering a 6- or 3-digit hex keep the current alpha or reset it to 255?
- The plan wants the open picker to reflect the Eyedropper, but the popover's backdrop closes it on any outside click, so the Eyedropper can't be used while it is open. Should the backdrop leave the canvas usable (picker stays open while sampling), or is "shows the sampled color the next time it opens" enough?
- When the popover closes (Escape / outside click) with hex or number-field text that was typed but not committed with Enter, is that text applied or discarded?

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
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |

## Decisions

- 2026-10-28 (legacy plan): The picker is CSS-only (gradients, no `<canvas>`) — nothing can be read back from it, and no new dependency is needed.
- 2026-10-28 (legacy plan): Live preview while dragging, one commit at the end — recent colors and prefs would otherwise fill up with intermediate colors.
- 2026-10-28 (legacy plan): Recent colors: last 12, most recent first, de-duplicated, per mode in IndexedDB — localStorage is forbidden (AGENTS.md §9) and the two modes use colors differently.
- 2026-10-28 (legacy plan): Pixel mode keeps its palette panel — recent colors complement it, they don't replace it.
