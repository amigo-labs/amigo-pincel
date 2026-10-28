---
id: todo-color-picker
title: Own color picker — HSV, alpha, hex, recent colors (both modes)
status: todo
size: L (two M sessions)
mode: Pixel + Image
depends_on: []
created: 2026-10-28
---

# Own color picker (both modes)

## Goal

Clicking the foreground or background swatch opens **Pincel's own picker** in a popover. It is the same in both modes and on every OS. It has:
- a saturation/value square and a hue slider;
- an **alpha** slider;
- a hex field that also accepts `#RRGGBBAA`;
- R, G, B and A number fields;
- a row of **recent colors**.

It replaces the native `<input type="color">`.

## Why

Today the swatches open the browser's or OS's own picker:
- It looks different everywhere.
- It **cannot set alpha**.
- On some platforms it opens as a separate modal window.

paint.net, Aseprite, Photopea and Pixelorama all have their own HSV picker with alpha. In Image mode, color alpha can only be faked with the tool's opacity slider.

## Current state (read these first)

**`ui/src/lib/components/ColorPair.svelte`**, shared by both modes:
- Props (≈L9–21): `fg`, `bg` (`#rrggbb`), `onFg`, `onBg`, `onSwap`, `onReset`, and optional `alpha` / `onAlpha`. The alpha slider only shows when both are passed (≈L92–106).
- The swatches are two overlapping `<input type="color">` (≈L38–53). These are what gets replaced.
- The hex field edits the **fg only**, and only as `#RRGGBB` (≈L30–33).
- There is no background-alpha UI.

**`ui/src/lib/color.ts`**:
- `packColor(hex, alpha)` → `0xRRGGBBAA`.
- `unpackColor(rgba)` → `#RRGGBB`, dropping alpha.
- There are no HSV helpers.

**Pixel mode**:
- `colors = { color, alpha, bgColor, bgAlpha }` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L107). The core (`Rgba`) already takes alpha through `packColor`.
- The eyedropper writes both color and alpha (`ui/src/modes/pixel/pointer.ts` ≈L139–158).
- Palette left-click sets fg and right-click sets bg (`PixelEditor.svelte` ≈L594–621).
- Prefs `lastColor` / `lastBgColor` persist hex only, **not alpha** (≈L162–163, L466–540).

**Image mode**:
- `tool.foreground` / `tool.background` are hex strings with **no alpha** (`ui/src/modes/image/stores/editor.svelte.ts` ≈L194–196).
- Opacity is a separate tool setting, `tool.opacity` (1–100 %).
- `colorWithOpacity(hex)` in `ui/src/modes/image/core/controller.ts` (≈L62) bakes opacity into the RGBA it sends.
- `ColorsPanel.svelte` passes no alpha props.
- Image colors are not persisted.

**Popover pattern.** No generic popover exists. Copy the PalettePanel ⋯ menu (`ui/src/modes/pixel/components/PalettePanel.svelte` ≈L61–68 and L204–243):
- fixed position from `getBoundingClientRect()`, clamped to the window;
- a transparent backdrop button that closes it;
- Escape closes it.

**i18n keys** `color.*` are in `ui/src/lib/i18n/en.ts` (≈L106–114).

## Design

**Components.**
- `ui/src/lib/components/color/ColorPicker.svelte`: the popover body, with no knowledge of modes.
  - Props: `value: { hex: string; alpha: number }`, `recent: string[]`, `onChange`, `onCommit`.
- `ColorPair.svelte` opens it from each swatch and keeps the swap and reset buttons.
- The picker is **CSS-only**:
  - The SV square is a div with the hue as its background color plus two linear gradients (white → transparent horizontally, transparent → black vertically).
  - The hue and alpha sliders are gradient tracks; the alpha track sits over a checkerboard.
  - No `<canvas>` is involved, so nothing could ever be read back (CLAUDE.md §9).
- Input works with pointer, touch and pen (pointer capture), and with the keyboard: arrow keys move the SV handle and the sliders, Shift makes the steps bigger.
- Live preview while dragging. **One commit when the drag ends.** Colors aren't document state, so there is no undo concern, but recent colors and the IndexedDB prefs update on commit only.

**Color math.** New `ui/src/lib/color.ts` helpers:
- `hexToRgba`, `rgbaToHex(withAlpha)`, `rgbToHsv`, `hsvToRgb`;
- `parseColor(text)`, which accepts `RRGGBB`, `#RRGGBB`, `#RRGGBBAA` and `#RGB`.

Keep hue and saturation in picker state when value is 0 or saturation is 0, so dragging through black or grey doesn't reset the hue.

**Recent colors.**
- The last 12 committed colors, most recent first, de-duplicated.
- Persisted per mode in the IndexedDB prefs (`recentColors:pixel` / `recentColors:image`), never in localStorage.
- Pixel mode also keeps its palette panel, so recent colors don't replace it.

**Alpha.**
- **Pixel mode** already carries alpha. Expose `bgAlpha` in the picker too, and persist alpha alongside the hex prefs, or switch the prefs to `#RRGGBBAA`.
- **Image mode** gains a color alpha. Recommendation: effective alpha = color alpha × tool opacity, computed in `colorWithOpacity`. paint.net has the same split: alpha on the color, opacity on the brush. Document this in a Decision Log entry.
  - Persist Image colors as well, under new keys.

**Eyedropper.** It sets the hex and the alpha in both modes. The picker must reflect that while open.

## Tasks (one M session each)

1. **`docs(spec)` + color math + picker component.**
   - Decision Log entry: own picker, CSS-only, recent colors, and the Image-mode alpha × opacity rule.
   - Update the §9.2 Colors paragraph.
   - Color helpers.
   - `ColorPicker.svelte`.
   - Wire it into `ColorPair.svelte` with popover behaviour.
   - Pixel mode fully wired: fg and bg alpha, prefs with alpha, recent colors.
   - i18n keys `color.picker`, `color.recent`, `color.hue`, `color.saturation`, `color.value`.
2. **Image mode.**
   - Color alpha in the store and in `colorWithOpacity`.
   - Prefs and recent colors.
   - e2e `ui/e2e/color-picker.spec.ts`, Pixel and Image:
     - open the picker from the fg swatch, type `#11223380`, commit;
     - paint;
     - check with the Eyedropper that the alpha came through (the Pixel test pattern in `ui/e2e/pencil.spec.ts`);
     - the recent row shows the color;
     - reload and the color persists.

## Out of scope

- Color harmonies, a color wheel and named swatches.
- Wide-gamut or HDR colors (non-goal, §1.3).
- The palette panel itself.
