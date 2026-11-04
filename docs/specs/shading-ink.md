---
status: implemented
created: 2026-10-28
size: M
mode: Pixel
design: [pincel.md §3.7, pincel.md §5.2]
depends_on: [pixel-pencil]
---

# Shading ink

## Context

Aseprite's **shading ink** lets a pixel artist darken or lighten existing pixels along a palette ramp instead of picking each shade by hand. Pincel's Pixel-mode Pencil can only paint the foreground (or, with the right button, the background) color.

This spec adds an **ink** option to the Pencil: with the shading ink, each pixel the stroke touches steps **one entry along a palette ramp** — the left button toward the ramp's end (lighter, if the ramp is ordered dark → light), the right button toward its start. Colors not in the ramp stay untouched. The ramp is a contiguous range of palette entries selected in the Palette panel, or the whole palette when no range is selected.

It extends the Pencil stroke from the `pixel-pencil` spec (one command per stroke with the pixels' values from before the stroke). The Decision Log (2026-10-29) records the Ink option and the ramp selection; §5.2 gets the Ink row and §3.7 the range selection.

This is part B of the legacy plan `08-pixel-tools.md`; `dithered-gradient` and `rotsprite` are the other parts.

## Scope

### In scope

- An Ink option for the Pixel-mode Pencil: Simple (today's behavior) or Shading
- Shading: left button steps one ramp entry toward the end, right button one toward the start, colors outside the ramp untouched, each pixel at most once per stroke
- Selecting a contiguous range of palette entries in the Palette panel with Shift+click as the ramp; the whole palette when no range is selected
- A hint in the Pencil options while Shading is chosen and no range is selected
- i18n keys for every new label, in `en.ts` and `de.ts`

### Out of scope

- An ink option for the Eraser — because the Eraser only clears pixels and has no color to step
- Other Aseprite inks (lock alpha, colorize, blur, …) — because the legacy plan only adds Simple and Shading
- Storing the ramp range in the document — because it is transient UI state, not document content
- Indexed-sprite shading (stepping palette indices) — because it comes with indexed painting in Phase 2 (§3.7), CLAUDE.md §9 forbids mixing Phase 2 work in, and paint commands reject indexed cels today; v1 matches exact RGBA on RGBA sprites only

## Acceptance criteria

- [x] AC-01 (option): Where the Pencil ink is Shading, a left-button stroke turns every touched pixel whose color before the stroke is ramp entry i (not the last) into ramp entry i + 1.
- [x] AC-02 (option): Where the Pencil ink is Shading, a right-button stroke turns every touched pixel whose color before the stroke is ramp entry i (not the first) into ramp entry i − 1.
- [x] AC-03 (option): Where the Pencil ink is Shading, a touched pixel whose color before the stroke exactly matches no ramp entry keeps its value.
- [x] AC-04 (option): Where the Pencil ink is Shading, a pixel at the last ramp entry stays there under a left-button stroke and a pixel at the first entry stays there under a right-button stroke.
- [x] AC-05 (always): Within one shading stroke each pixel changes at most once, judged against its value from before the stroke, however often the stroke passes over it.
- [x] AC-06 (option): Where the Pencil size is greater than 1, the shading ink applies to every pixel the brush stamp covers.
- [x] AC-07 (event): When the user Shift+clicks a palette entry while another entry is selected, the contiguous range of entries between the two (both included) becomes the shading ramp, in palette order.
- [x] AC-08 (state): While no palette range is selected, the whole palette in palette order is the shading ramp, and the Pencil options show a hint saying so while the Shading ink is chosen.
- [x] AC-09 (event): When the Pencil becomes the active tool, the options bar shows an Ink control with the choices Simple and Shading; a fresh editor session starts with Simple.
- [x] AC-10 (option): Where the Pencil ink is Simple, the Pencil paints exactly as it does without this feature.
- [x] AC-11 (always): A shading stroke is one undo entry; undo restores every touched pixel to its value from before the stroke.
- [x] AC-12 (always): The selected palette range is never written into the saved document.
- [x] AC-13 (error): If the target layer is locked or the document has no paintable image layer, then a shading stroke changes nothing and adds no undo entry.
- [x] AC-14 (event): When palette entries are added, removed or reordered, or a preset or import replaces the palette, then the selected palette range is cleared (the whole palette becomes the ramp); when an entry is only recolored, the range is kept.

## Technical notes

> Paths and line numbers are hints from 2026-10-28 and drift — grep for the names.

**Current state**

- Palette: `Palette { colors: Vec<PaletteEntry { rgba, name }> }` (`crates/pincel-core/src/document/palette.rs` ≈L8–28), `Sprite.palette` (`document/mod.rs` ≈L77); commands in `command/palette_ops.rs`.
- Palette UI: `ui/src/modes/pixel/components/PalettePanel.svelte` — a plain click sets the local `selected` index and picks the foreground (≈L265), right-click / context menu picks the background (≈L269), swatches are draggable to reorder. No Shift+click handling yet.
- Colors state: `colorsInit()` in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L107) — `color`, `alpha`, `bgColor`, `bgAlpha`.
- Right button with the Pencil paints the background color today (`BG_TOOLS` in `ui/src/modes/pixel/pointer.ts` ≈L21).
- Options bar: `{#snippet options()}` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L786); tool state in `toolInit()` (≈L83).

**Design (from the legacy plan)**

- `PencilStroke` (from `pixel-pencil`) gets `ink: Ink::Simple | Ink::Shading { ramp: Vec<Rgba>, step: i8 }`. Each pixel is shaded once per stroke against its value from the `prior` map, matching Aseprite. RGBA sprites match the exact RGBA against the ramp.
- The range lives in the Pixel colors state, not the document.
- Suggested tests: `shading_steps_one_entry_toward_end`, `shading_right_button_steps_toward_start`, `shading_ignores_colors_outside_ramp`, `shading_same_pixel_twice_in_one_stroke_steps_once`, `shading_clamps_at_ramp_end`; e2e: with a 3-color ramp, one stroke over a pixel of entry 0 makes it entry 1 (Eyedropper read-back as in `ui/e2e/pencil.spec.ts`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Stroke passes over the same pixel three times | The pixel steps exactly once (AC-05) |
| Pixel at the ramp's last entry, left-button stroke | Unchanged (AC-04) |
| Transparent pixel and transparent is not in the ramp | Unchanged (AC-03) |
| Color that is close to but not exactly a ramp entry | Unchanged (AC-03) |
| Range of one single entry | Every matching pixel stays unchanged in both directions (AC-04) |
| Pixel-perfect on with the Shading ink | A removed corner pixel shows its value from before the stroke |
| Shift+click on the entry that is already selected | The range is that single entry (AC-07) |
| Range selected, then an entry is deleted, added or dragged to a new position | The range is cleared (AC-14) |
| Range selected, then one of its entries is recolored | The range is kept; the ramp uses the new color (AC-14) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The range anchor is the entry selected by the last plain click, and a plain click clears the range | The plan says "Shift+click selects a contiguous range" without naming the anchor or how to clear it | Medium — affects the Palette panel interaction |
| Shift+click does not change the foreground color | The plan only gives Shift+click the range role | Low |
| If a color occurs twice in the ramp, a pixel of that color is matched to its first occurrence | Exact RGBA matching needs a rule for duplicates; the plan gives none | Low |

## Summary

> Completed: 2026-11-03

- The Pencil's Shading ink steps each painted pixel one palette entry along a ramp chosen by Shift+click in the palette (toward the end, or the start with the right button), once per pixel per stroke, as one undo step
- Colours outside the ramp are left alone and the ends clamp; larger stamps and pixel-perfect follow the same rule
- The ink choice is offered for the Pencil only, starts Simple, and the ramp range is view state that never reaches the saved file

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `shading_steps_one_entry_toward_end` (crates/pincel-core/src/command/pencil_stroke.rs); `shading_stroke_steps_each_pixel_once_as_one_undo_entry` (crates/pincel-wasm/src/stroke.rs); "a shading stroke steps ramp colors one entry, once per pixel, as one undo step" (ui/e2e/pencil.spec.ts) | verified | ba53299 |
| AC-02 | `shading_right_button_steps_toward_start` (crates/pincel-core/src/command/pencil_stroke.rs); `shading_stroke_backwards_steps_toward_the_start` (crates/pincel-wasm/src/stroke.rs); "a shading stroke steps ramp colors one entry, …" (ui/e2e/pencil.spec.ts) | verified | ba53299 |
| AC-03 | `shading_ignores_colors_outside_ramp` (crates/pincel-core/src/command/pencil_stroke.rs) | verified | 6aed618 |
| AC-04 | `shading_clamps_at_ramp_end` (crates/pincel-core/src/command/pencil_stroke.rs) | verified | ba53299 |
| AC-05 | `shading_same_pixel_twice_in_one_stroke_steps_once`, `shading_pixel_perfect_restores_a_pruned_corner_to_its_prior_value` (crates/pincel-core/src/command/pencil_stroke.rs) | verified | 35f631c |
| AC-06 | `shading_size_3_shades_every_stamped_pixel` (crates/pincel-core/src/command/pencil_stroke.rs) | verified | 6aed618 |
| AC-07 | "a shading stroke steps ramp colors one entry, once per pixel, as one undo step", "a plain click or a structural palette edit clears the ramp range; a recolor keeps it" (ui/e2e/pencil.spec.ts) | verified | 35f631c |
| AC-08 | "the pencil offers Simple and Shading inks and a fresh session starts Simple", "a plain click or a structural palette edit clears the ramp range; a recolor keeps it" (ui/e2e/pencil.spec.ts) | verified | 9af6b69 |
| AC-09 | "the pencil offers Simple and Shading inks and a fresh session starts Simple" (ui/e2e/pencil.spec.ts) | verified | 9a7f4f0 |
| AC-10 | `simple_ink_paints_like_a_plain_stroke` (crates/pincel-core/src/command/pencil_stroke.rs); the unchanged Pencil specs in ui/e2e/pencil.spec.ts and ui/e2e/pixel-tools.spec.ts | verified | c43a845 |
| AC-11 | `shading_stroke_is_one_undo_entry_that_restores_every_pixel` (crates/pincel-core/src/command/pencil_stroke.rs); `shading_stroke_steps_each_pixel_once_as_one_undo_entry` (crates/pincel-wasm/src/stroke.rs); "a shading stroke steps ramp colors one entry, …" (ui/e2e/pencil.spec.ts) | verified | 5dd2afe |
| AC-12 | "the ramp range is not written into the saved document" (ui/e2e/pencil.spec.ts) | verified | 22f5a52 |
| AC-13 | `shading_stroke_on_locked_layer_errs_without_undo_entry`, `shading_stroke_without_image_layer_errs` (crates/pincel-wasm/src/stroke.rs) | verified | 3678aba |
| AC-14 | "a plain click or a structural palette edit clears the ramp range; a recolor keeps it" (ui/e2e/pencil.spec.ts) | verified | 48b5d53 |

## Decisions

- Each pixel is shaded at most once per stroke, judged against its value from before the stroke — because that matches Aseprite and keeps a scribble from running a pixel through the whole ramp (legacy plan).
- The ramp is a contiguous palette range chosen with Shift+click in the Palette panel, the whole palette without one — because it needs no new panel and keeps the ramp in palette order (legacy plan).
- The range is transient UI state in the Pixel colors state, not document content (legacy plan).
- The ink is an option of the Pencil stroke, not a separate tool — because it reuses the stroke's stamping, sizes and undo (legacy plan).
- 2026-11-04 (spec-build): `PencilStroke` takes the ink through `with_ink(Ink)` rather than a new `new` argument, and the wasm boundary gets `beginShadingStroke(ramp: Uint32Array, step: ±1, size, shape, pixelPerfect)` next to `beginStroke` — another positional argument on either would need a new `too_many_arguments` suppression, and the Simple ink keeps every existing caller unchanged.
- 2026-11-04 (spec-build): A pixel the shading ink leaves alone (no ramp match, or clamped at a ramp end) is not written and not counted in the stroke's dirty region; its prior value is still recorded so pixel-perfect pruning and undo stay uniform.
- 2026-11-04 (spec-build): Shift+click with no entry selected makes the clicked entry the anchor and the range that one entry, without changing the foreground — it keeps Shift+click from ever picking a color, like the Assumptions say.
- 2026-11-04 (spec-build): Besides the panel's own structural edits, the range is cleared whenever it no longer fits the palette (a new or opened document, an undone add) — an index past the palette has no meaning; undo / redo of a move keeps an in-range range, since the panel cannot tell it from a recolor.
- 2026-11-04 (spec-build): The Ink control shows for the Pencil only and is not persisted; with the Shading ink the right button steps toward the ramp's start instead of painting the background color (AC-02).
- 2026-11-04 (spec-build): The hint reads "ramp: the whole palette — Shift+click a palette entry for a range" without a range and "ramp: palette entries {from}–{to}" with one, using the 0-based indices the entry editor shows; range swatches get an accent underline.
- 2026-10-29: The Decision Log records that the Pixel Pencil gets an Ink option (Simple / Shading), that the shading ramp is a Shift+click contiguous palette range (transient UI state, the whole palette without one) and that each pixel steps once per stroke; §5.2 gets the Ink row and §3.7 the range selection — `Ink` is a new public stroke field on the npm-facing API (owner accepted the recommendation).
- 2026-10-29: Indexed-sprite shading is left for later; v1 matches exact RGBA on RGBA sprites only — CLAUDE.md §9 forbids mixing Phase 2 work in, and paint commands reject indexed cels today (owner accepted the recommendation).
- 2026-10-29: Adding, removing or reordering entries, or replacing the palette by a preset or import, clears the selected range; recoloring an entry keeps it (AC-14) — it is predictable, and after a structural change the old index span has no meaning left (owner accepted the recommendation).
