---
id: todo-outline-effect
title: Outline effect for sprites (shared effect, both modes)
status: todo
size: S–M (one session)
mode: Pixel first, Image too (shared pincel-effects)
depends_on: []
created: 2026-10-28
---

# Outline effect for sprites

## Goal

Effects ▸ **Outline…** draws a hard outline in a chosen color around the non-transparent pixels of the active layer, within the selection if there is one. Options:
- **Place:** outside (default) or inside.
- **Shape:** circle / 4-neighbour (no diagonals) or square / 8-neighbour.
- **Thickness:** 1–8 px.
- **Color:** foreground by default.

It has a live preview like every effect, and it is one undo step.

## Why

Sprites for games almost always need an outline. Aseprite has Edit ▸ FX ▸ Outline with exactly these options, and it is a standard pixel-art workflow. Pincel's effects list has adjustments, blur, distort, noise and sharpen, but no outline.

## Current state (read these first)

**Effects crate.** `crates/pincel-effects/src/`
- `lib.rs`, `spec.rs` (`EffectSpec`), `effect.rs`, `image.rs` (`EffectImage`), `kernel.rs`, `error.rs`.
- Groups: `adjust/`, `blur/`, `distort/`, `noise/`, `sharpen/`.
- Effects are stateless and shared by both cores (CLAUDE.md §1). The wire format is positional (Decision Log 2026-10-11).

**UI catalog.**
- `ui/src/lib/effects/catalog.ts`: a `ParamDef` is `number | bool | choice` (≈L16–25). **There is no `color` kind yet.**
- `menu.ts` and `labels.ts` sit next to it.
- `ui/src/lib/components/EffectDialog.svelte` renders the params and the live preview.

**Pixel-mode application.** `applyEffectParams` / `cancelEffect` in `ui/src/modes/pixel/PixelEditor.svelte`, wasm side `crates/pincel-wasm/src/effects.rs`. Pixel mode has an Effects menu.

**Image mode.** `applyEffect` / `previewEffect` in `ui/src/modes/image/core/controller.ts`, wasm `apply_effect` / `preview_effect` in `crates/pincel-image-wasm/src/lib.rs`, with the selection mask through `layer_effect_image`.

## Design

**Algorithm.** In `crates/pincel-effects/src/distort/` or a new `outline/`.
- Input: the layer's RGBA and the selection mask.
- `opaque(p)` means `alpha(p) > 0`. Aseprite treats any non-zero alpha as part of the sprite. Optionally add a threshold param.
- **Outside, thickness t:** a pixel `p` with `!opaque(p)` becomes the outline color when some opaque pixel lies within distance t, using the Chebyshev metric (square) or Manhattan (circle). Use a distance transform or t dilation passes; for t ≤ 8, t passes of 3×3 dilation are fine.
- **Inside:** an opaque pixel that has a transparent neighbour (within t) is recolored.
- Only pixels inside the selection mask change. The canvas edge counts as transparent.
- **Indexed sprites** (Pixel mode): write the palette index of the chosen color. If the color isn't in the palette, use the nearest entry and say so in the dialog, or refuse. Check how other effects treat indexed sprites in `crates/pincel-wasm/src/effects.rs` and follow that.

**Color parameter.**
- Add a `color` `ParamDef` kind (`#RRGGBB[AA]`, default "foreground") to the catalog and `EffectDialog.svelte`. The swatch can reuse `ColorPair` styling, or the picker from `14-color-picker.md` once it exists; until then, use the native color input plus hex.
- Carry it in the positional wire format as packed RGBA (u32). **Bump or extend the spec** following the 2026-10-11 decision, and add a Decision Log line.

## Tasks (one session)

1. **`docs(spec)`.** Add Outline to the effects list in §5.4 / the Pixel effects menu, plus a Decision Log line for the color param kind.
2. **`feat(pincel-effects)`.** The outline effect.
   - Tests:
     - `outline_outside_square_1px_surrounds_single_pixel_with_8`
     - `outline_outside_circle_1px_gives_4`
     - `outline_inside_recolors_border_only`
     - `outline_respects_selection_mask`
     - `outline_thickness_3_square_grows_ring`
     - `outline_image_edge_counts_as_transparent`
3. **`feat(ui)`** (+ the wasm bindings if the catalog needs them).
   - The color param kind, the catalog entry, i18n, and both modes' menus.
   - e2e: on a 5×5 sprite with one pixel in the centre, Outline with defaults colors the 8 neighbours (Pixel mode, Eyedropper read-back at one neighbour and one non-neighbour).

## Out of scope

- Anti-aliased or soft outlines (Image mode could later add a "stroke" layer effect; see the backlog: non-destructive layer effects).
- A hue-shifted outline color (the `mcp.md` vision). That could be an option later.
