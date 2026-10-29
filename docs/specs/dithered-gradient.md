---
status: todo
created: 2026-10-28
size: M
mode: Pixel
design: [pincel.md §2.3, pincel.md §3.7, pincel.md §5.2, pincel.md §5.4, pincel.md §13.1, pincel.md §15]
---

# Dithered gradient tool

## Context

Pixel artists build gradients by dithering between palette colors instead of blending, so no new colors enter the sprite. Pixel mode has no gradient tool at all; Image mode has a blending gradient (`pincel-image-core`, §5.4) whose output is not palette-clean.

This spec adds a **Gradient** tool to Pixel mode: drag a line and the area (the selection, or the target cel — a canvas-sized cel on a layer without one — when there is none) fills with a **dithered** ramp from the foreground to the background color — linear or radial, with an ordered-dither (Bayer 2×2 / 4×4 / 8×8) matrix or hard bands. It creates no new colors.

Already decided: the two cores stay separate (Decision Log 2026-09-06, "Two cores, one shell"), so `pincel-core` must not depend on `pincel-image-core`; `pincel-effects` serves both cores. Neither the tool nor its menu entry is in §5.2 yet.

This is part A of the legacy plan `08-pixel-tools.md`; `shading-ink` and `rotsprite` are the other parts.

## Scope

### In scope

- A Gradient tool in the Pixel-mode fill tool group, next to the Bucket
- Linear and radial gradients from the foreground color at the drag start to the background color at the drag end
- Dither matrix choice: none (hard bands), Bayer 2×2, Bayer 4×4, Bayer 8×8
- Respecting the selection mask
- A preview line while dragging
- One undo entry per gradient
- i18n keys for every new label, in `en.ts` and `de.ts`

### Out of scope

- Gradient kinds beyond linear and radial (reflected, conical, diamond, which Image mode has) — because the legacy plan limits the Pixel-mode tool to linear and radial
- Blended (non-dithered) gradients in Pixel mode — because they create new colors, which is exactly what this tool avoids
- Changes to Image mode's gradient behavior — because Image mode only shares the moved parameter math, its output must stay identical
- Writing palette indices on indexed sprites — Phase 2 with indexed painting (§3.7, AGENTS.md §9)
- Dithering through a palette range — because the range selection comes with `shading-ink`; follow-up

## Acceptance criteria

- [ ] AC-01 (option): Where the kind is linear, after a drag from P0 to P1 every changed pixel whose centre projects onto the drag line at or before P0 has the foreground color, and every one that projects at or beyond P1 has the background color.
- [ ] AC-02 (option): Where the kind is radial, after a drag from P0 to P1 the pixel at P0 has the foreground color and every changed pixel at a distance of at least |P1 − P0| from P0 has the background color.
- [ ] AC-03 (always): Every pixel the gradient writes has exactly the foreground or exactly the background color; no other color is created.
- [ ] AC-04 (option): Where a Bayer n×n matrix (n = 2, 4 or 8) is chosen, a pixel at (x, y) gets the foreground color exactly when its gradient position t (0 at P0, 1 at P1, clamped) is below the n×n Bayer threshold for (x mod n, y mod n), normalized into (0, 1).
- [ ] AC-05 (option): Where a Bayer matrix is chosen, along a horizontal linear gradient the number of foreground pixels per column never increases from the start column to the end column.
- [ ] AC-06 (option): Where the matrix is none, the result is two hard bands: foreground where t < 0.5, background elsewhere.
- [ ] AC-07 (state): While a selection exists, the gradient changes only pixels inside the selection; every pixel outside keeps its value.
- [ ] AC-08 (always): One gradient drag is one undo entry; undo restores every pixel of the target to its value from before the drag, and redo reproduces the identical result.
- [ ] AC-09 (state): While the gradient drag is in progress, a preview line from the drag start to the pointer is shown, and the document stays unchanged until release.
- [ ] AC-10 (event): When the user presses `G`, the tool selection cycles through the fill group, which contains Bucket and Gradient.
- [ ] AC-11 (event): When the Gradient tool becomes active, the options bar shows a Kind control (linear / radial) and a Dither control (none / Bayer 2×2 / Bayer 4×4 / Bayer 8×8).
- [ ] AC-12 (error): If the drag starts and ends on the same point, then nothing changes and no undo entry is added.
- [ ] AC-13 (error): If the target layer is locked, the document has no paintable image layer, or the sprite is indexed or grayscale, then nothing changes and no undo entry is added.
- [ ] AC-14 (always): Image mode's gradient results stay identical after its parameter math is shared with Pixel mode.
- [ ] AC-15 (state): While no selection exists, the gradient writes every pixel of the target cel; it never grows the cel.

## Technical notes

> Paths and line numbers are hints from 2026-10-28 and drift — grep for the names.

**Current state**

- Tools: `ui/src/modes/pixel/tools.ts` — `type Tool` (≈L10), rail groups (≈L90–130), e.g. `{ id: 'fill', tools: [{ id: 'bucket', … }] }` (≈L108); `isShapeTool` / `isDragShapeTool` (≈L39–57). `TOOL_GROUP_KEYS` in `ui/src/lib/shell/shortcuts.ts` maps `fill: 'g'` (≈L89).
- Options bar: `{#snippet options()}` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L786); tool state in `toolInit()` (`ui/src/modes/pixel/stores/editor.svelte.ts` ≈L83). Overlay previews in `ui/src/modes/pixel/overlay-painter.ts`.
- Palette: `Palette { colors: Vec<PaletteEntry { rgba, name }> }` (`crates/pincel-core/src/document/palette.rs` ≈L8–28), `Sprite.palette` (`document/mod.rs` ≈L77); commands in `command/palette_ops.rs`.
- Fill: `FillRegion` (`crates/pincel-core/src/command/fill_region.rs` ≈L29) — 4-connected BFS, tolerance 0, rejects indexed and grayscale cels; a seed outside the cel is a no-op (it does **not** grow the cel, contrary to the legacy plan's note).
- Selection: `Sprite.selection` / `selection_mask` (`document/mod.rs` ≈L93–98); `ClearRegion::masked()` (`command/clear_region.rs` ≈L61) is the masking pattern.
- Image-mode ramp math: `GradientKind`, `parameter(px, py, &Ramp)` (≈L130) and `sample()` (≈L153) in `crates/pincel-image-core/src/tools/gradient.rs`; a drag shorter than `f32::EPSILON` returns no command (≈L97).

**Design (from the legacy plan)**

- Move the pure parameter math `t(px, py)` for linear and radial into `crates/pincel-effects` and call it from both cores (a `refactor(pincel-effects)` commit before the feature commit).
- Dither rule per pixel: `t ∈ [0, 1]`, threshold `m = bayer[n][y mod n][x mod n]` normalized to (0, 1); foreground if `t < m`, else background.
- Core command `DrawDitherGradient { layer, frame, from, to, kind, matrix, colors }` in `crates/pincel-core/src/command/` plus an `AnyCommand` variant; respects the selection mask; reverts from stored prior pixels.
- UI: Gradient tool in the `fill` rail group, options kind and matrix, drag preview line in the overlay.
- Suggested tests: `bayer4_ramp_has_monotonic_density`, `gradient_respects_selection_mask`, `gradient_revert_restores_cel`; e2e: a gradient dragged across a 16×1 sprite leaves the left end foreground and the right end background (Eyedropper read-back as in `ui/e2e/pencil.spec.ts`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Zero-length drag (click) | No change, no undo entry (AC-12) |
| Drag that starts and ends outside the canvas | Linear: pixels are coloured by their projection, clamped to 0–1 (AC-01) |
| Selection that the drag line does not touch | Only the selection changes, coloured by the pixels' t values (AC-07) |
| Foreground equals background color | Every target pixel gets that color (AC-03) |
| Bayer 8×8 on a 4×4 sprite | Only the matching 4×4 part of the threshold pattern is used (AC-04) |
| 16×1 sprite, horizontal drag from column 0 to column 15 | Column 0 is foreground, column 15 is background (AC-01) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| "None" means a single split at t = 0.5 | The plan says "hard bands" with only two colors and gives no threshold | Low |
| The tool starts with kind linear and matrix Bayer 4×4 | The plan names linear first and tests Bayer 4×4; it gives no defaults | Low |
| A zero-length drag changes nothing | Image mode's gradient returns no command for it | Low |

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
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |

## Decisions

- The linear / radial parameter math moves into `pincel-effects` and both cores call it — because `pincel-core` must not depend on `pincel-image-core` (Decision Log 2026-09-06) and `pincel-effects` already serves both (legacy plan).
- Ordered (Bayer) dithering with a per-pixel threshold — because it is deterministic and creates no new colors (legacy plan).
- The Gradient tool lives in the fill group cycled by `G` — because it fills an area like the Bucket and `G` already selects that group (legacy plan; `TOOL_GROUP_KEYS`).
- 2026-10-29: A Decision Log entry records moving the linear / radial `t(px, py)` math from `pincel-image-core` into `pincel-effects` and the dithered Pixel Gradient tool (fg → bg, Bayer 2/4/8 or hard split, no new colors) in the `G` fill group, and `pincel.md` §5.2 gains a Gradient row — AGENTS.md §2.1, and §5.2 lists every Pixel tool (owner accepted the recommendation).
- 2026-10-29: Indexed and grayscale cels are refused like `FillRegion` does (AC-13); writing palette indices comes later with indexed painting — indexed painting is Phase 2 (§3.7) and AGENTS.md §9 forbids mixing phases, while fg / bg-only output already creates no new colors in RGBA (owner accepted the recommendation).
- 2026-10-29: Dithering through a palette range is a follow-up; this spec stays fg → bg and does not depend on `shading-ink` — it keeps the spec at size M and independent, and the ramp can later take N colors without changing the dither rule (owner accepted the recommendation).
- 2026-10-29: Without a selection the gradient covers the target cel's bounds clipped to the canvas and never grows the cel; a layer without a cel gets the usual canvas-sized cel (`ensure_paint_cel`) (AC-15) — Phase 1 never auto-grows cels, and trimmed cels only come from imported `.aseprite` files (owner accepted the recommendation).
