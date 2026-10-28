---
id: todo-pixel-tools
title: Pixel tools — dithered gradient, shading ink, RotSprite rotation
status: todo
size: XL (three independent M/L tasks; do them in separate sessions)
mode: Pixel
depends_on: [todo-pixel-pencil (shading ink only)]
created: 2026-10-28
---

# Pixel tools: dithered gradient, shading ink, RotSprite rotation

Three independent features. Each one is its own session and its own PR-sized change. Order: **A → B → C**. B needs `01-pixel-pencil.md`.

## Shared context (read first)

**Tools.** `ui/src/modes/pixel/tools.ts`
- `type Tool = 'pencil' | 'eraser' | … | 'slice'` (≈L10–30).
- Rail groups (≈L90–130), e.g. `{ id: 'fill', tools: [{ id: 'bucket', … }] }` (≈L108).
- Shape helpers `isShapeTool` / `isDragShapeTool` (≈L39–57).

**Options bar.** `{#snippet options()}` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L786–850). Tool state lives in `toolInit()` (`ui/src/modes/pixel/stores/editor.svelte.ts` ≈L83).

**Palette.**
- `Palette { colors: Vec<PaletteEntry { rgba, name } > }` (`crates/pincel-core/src/document/palette.rs` ≈L8–28), as `Sprite.palette` (`document/mod.rs` ≈L77).
- Commands in `command/palette_ops.rs`.
- UI: `ui/src/modes/pixel/components/PalettePanel.svelte`.

**Fill.** `FillRegion` (`crates/pincel-core/src/command/fill_region.rs` ≈L29): 4-connected BFS, tolerance 0, **rejects indexed and grayscale**.

**Selection.** `Sprite.selection` / `selection_mask` (`document/mod.rs` ≈L93–97). Every pixel tool should respect the mask, as `ClearRegion::masked()` does.

**Spec.** None of these is in §5.2 or §13 yet. §17.3 lists RotSprite under #1232 ("Configurable resampling: Nearest, Scale2x/3x/4x, hqx, RotSprite").

---

## A. Dithered gradient tool (M)

### Goal

A **Gradient** tool in Pixel mode. Drag a line, and the area (the selection, or the whole cel) fills with a **dithered** ramp from foreground to background:
- Linear or Radial;
- dither matrix: **Bayer 2×2 / 4×4 / 8×8**, or none (hard bands).

It creates no new colors, so it stays palette-clean, which is the pixel-art reason for dithering.

### Design

**Ramp math.** Image mode already has it: `GradientKind`, `parameter(px, py, &Ramp)` and `sample()` in `crates/pincel-image-core/src/tools/gradient.rs` (≈L23–160).
- The two cores stay separate on purpose (spec Decision Log 2026-09-06), so **don't import pincel-image-core from pincel-core**.
- Move the pure parameter math, `t(px, py)` for linear and radial, into **`crates/pincel-effects`**, which both cores already use, and call it from both.
- Use a `refactor(pincel-effects)` commit before the feature commit.

**Dither rule.** Per pixel, `t ∈ [0, 1]` and threshold `m = bayer[n][y mod n][x mod n]`, normalized to (0, 1). The pixel is fg if `t < m`, else bg.

**Multi-color ramp, optional.** When the palette panel has a range selected (see B), step through the range instead of fg → bg: `index = floor(t × (k − 1))`, then dither between adjacent entries.

**Core.** `DrawDitherGradient { layer, frame, from, to, kind, matrix, colors }` in `crates/pincel-core/src/command/`, plus an `AnyCommand` variant.
- It respects the selection mask, writes into the cel (growing it like `FillRegion` does) and reverts by stored prior pixels.
- Indexed sprites: write **palette indices** (fg / bg index). This is the main advantage over Image mode's gradient.

**UI.**
- A Gradient tool in the `fill` rail group next to Bucket. Key `G` cycles the group (see `TOOL_GROUP_KEYS` in `ui/src/lib/shell/shortcuts.ts`).
- Options: kind and matrix.
- A drag preview line in the overlay.
- i18n.

### Tests

- `bayer4_ramp_has_monotonic_density`
- `gradient_respects_selection_mask`
- `gradient_on_indexed_writes_indices`
- `gradient_revert_restores_cel`
- e2e: drag the gradient on a 16×1 sprite, and the left end is fg while the right end is bg (Eyedropper read-back).

---

## B. Shading ink (M, needs `01-pixel-pencil.md`)

### Goal

An **ink** option for the Pencil, as in Aseprite: instead of painting the foreground color, each touched pixel steps **one entry along a palette ramp**.
- Left button: toward the ramp's end (lighter, if the ramp is ordered dark → light).
- Right button: toward its start.
- Colors not in the ramp stay untouched.

### Design

**Ramp selection.** In `PalettePanel.svelte`, Shift+click selects a **contiguous range** of entries; the selected range is the ramp.
- Store the range in the Pixel colors state, not in the document; it's transient UI state.
- Without a range, the whole palette is the ramp.

**Core.** `PencilStroke` (from 01) gets `ink: Ink::Simple | Ink::Shading { ramp: Vec<Rgba>, step: i8 }`.
- Each pixel is shaded **once per stroke**, judged against its value from **before** the stroke (the `prior` map). Going over it again within the same stroke doesn't keep shifting it. This matches Aseprite.
- Indexed sprites step indices. RGBA sprites match the exact RGBA against the ramp.

**UI.** An Ink select in the Pencil options (Simple / Shading), plus a hint when no range is selected. i18n.

### Tests

- `shading_steps_one_entry_toward_end`
- `shading_right_button_steps_toward_start`
- `shading_ignores_colors_outside_ramp`
- `shading_same_pixel_twice_in_one_stroke_steps_once`
- `shading_clamps_at_ramp_end`
- e2e: a 3-color ramp, one stroke over a pixel of entry 0 makes it entry 1 (Eyedropper read-back).

---

## C. Arbitrary rotation with RotSprite (M–L)

### Goal

Image ▸ Rotate… (and, with a selection, it rotates the selection content) by **any angle**, with two methods:
- **Nearest**: fast, jaggy.
- **RotSprite**: the pixel-art standard, which keeps lines clean.

Today only 90°, 180° and flips exist: `Orientation` in `crates/pincel-core/src/command/transform_support.rs` (≈L10), `TransformCel` and `TransformCanvas`, wasm `crates/pincel-wasm/src/transforms.rs`, UI `ui/src/modes/pixel/image-ops.ts` (≈L87–97).

### Design

**Algorithm (RotSprite).**
1. Upscale ×8 with three passes of **Scale2x (EPX)**.
2. Rotate with nearest-neighbour sampling around the centre, into an ×8 target large enough for the rotated bounds.
3. Downscale ×8 by taking the sample at each 8×8 block's centre. Some variants pick the block's most common color; choose one, document it, and test it.
4. Alpha handling: treat fully transparent as a color, so edges don't bleed.

**Where.**
- The pure functions `scale2x(raster)` and `rotsprite(raster, angle_deg)` / `rotate_nearest(...)` go into `Raster` (`command/transform_support.rs`) or a new `transform/rotate.rs` in pincel-core.
- Possibly into pincel-effects if Image mode should get "Rotate (pixel art)" later. **Decide and record it.**

**Command.** `RotateCel { layer, frame, angle, method, within: Option<Rect> }`.
- It grows the cel bounds to fit the result; Aseprite cels can carry their own bounds, so nothing is cropped.
- Undo by stored prior cel. One undo step.

**UI.**
- A Rotate dialog with an angle (−180…180, number plus slider), the method, and a live preview through the existing effect-preview pattern, if Pixel mode has one; otherwise preview on Apply.
- Menu entry Image ▸ Rotate….
- i18n.

**Performance.** ×8 of a 256×256 cel is 2048² = 4 M pixels per pass, which is fine. Cap the input at 1024² and show a notice beyond that.

### Tests

- `scale2x_matches_reference` (a hand-made 3×3 → 6×6 fixture)
- `rotsprite_0_degrees_is_identity`
- `rotsprite_90_matches_orient_rotate90` (must equal the exact 90° path)
- `rotsprite_keeps_palette_no_new_colors`
- `rotate_cel_grows_bounds_and_reverts`
- proptest: rotating by 0 or 360 is the identity for any small raster.

---

## Before each part

- Run the CLAUDE.md §3.1 pre-flight.
- Make a `docs(spec)` commit first: add the tool and menu entry to §5.2, plus a Decision Log entry for the choices made in that part (A: ramp math moved to pincel-effects; B: ramp selection in the palette panel; C: downscale rule and where RotSprite lives).
