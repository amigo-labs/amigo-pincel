---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §13.4, pincel.md §9.2, pincel.md §15 (2026-10-11 one effect wire format), pincel.md §15 (2026-10-12 Image-mode effects honour the selection)]
---

# Outline effect

## Context

Game sprites almost always need an outline. Aseprite has Edit ▸ FX ▸ Outline with these options, and it is a standard pixel-art workflow. Pincel's effects cover adjustments, blur, distort, noise and sharpen, but no outline. `pincel.md` §13.4 lists "Outline (configurable thickness, inside/outside/both)" among the filters.

Effects ▸ **Outline…** draws a hard outline in a chosen colour around the non-transparent pixels of the active layer, within the selection if there is one. Options: place (outside or inside), shape (circle = 4-neighbour, square = 8-neighbour), thickness 1–8 px, colour (foreground by default). It has a live preview like every effect and is one undo step.

Effects are stateless and shared by both cores through `pincel-effects`; both modes build their Effects menus and dialogs from one catalog and one positional wire format (Decision Log 2026-10-11). Pixel mode ships first; Image mode gets the same entry from the shared catalog.

## Scope

### In scope

- The Outline effect in `pincel-effects`, with the options place, shape, thickness and colour
- A colour parameter kind for the effect catalog and the effect dialog (native colour input plus hex text)
- Effects ▸ Outline… in Pixel mode and in Image mode, with live preview and one undo step
- en + de strings

### Out of scope

- Anti-aliased or soft outlines — because the effect is a hard pixel-art outline; a soft "stroke" layer effect for Image mode belongs to the non-destructive layer-effects backlog item.
- A hue-shifted outline colour (the `mcp.md` vision) — because it is a possible later option on top of a plain colour.
- A dedicated colour picker in the dialog — because `color-picker.md` builds it; until then the native colour input plus hex text is used.

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses Effects ▸ Outline… in Pixel mode or in Image mode, then the effect dialog opens with Place (Outside, Inside; default Outside), Shape (Circle, Square; default Square), Thickness (1–8 px; default 1) and Color (default: the current foreground colour).
- [ ] AC-02 (always): A pixel counts as part of the sprite ("opaque") when its alpha is above 0; pixels beyond the layer's edge count as transparent.
- [ ] AC-03 (option): Where Place is Outside and Shape is Square with thickness t, every transparent pixel whose Chebyshev distance to an opaque pixel is at most t becomes the outline colour, and no other pixel changes (a single opaque pixel with t = 1 gets its 8 neighbours coloured).
- [ ] AC-04 (option): Where Place is Outside and Shape is Circle with thickness t, every transparent pixel whose Manhattan distance to an opaque pixel is at most t becomes the outline colour, and no other pixel changes (a single opaque pixel with t = 1 gets its 4 edge neighbours coloured).
- [ ] AC-05 (option): Where Place is Inside with thickness t, every opaque pixel that has a transparent pixel within distance t (by the chosen shape's metric) becomes the outline colour, and opaque pixels farther inside and all transparent pixels stay unchanged.
- [ ] AC-06 (always): An outline pixel that is fully selected (or any outline pixel when there is no selection) takes the chosen colour exactly, including its alpha.
- [ ] AC-07 (state): While a selection exists, only pixels inside the selection change, and opaque pixels outside the selection still produce outline pixels inside it; in Image mode a feathered selection blends the result by coverage, as every Image-mode effect does.
- [ ] AC-08 (state): While the Outline dialog is open, the canvas shows the result live for the current options; when the user cancels, the layer is unchanged.
- [ ] AC-09 (event): When the user applies the Outline, then one undo restores the layer exactly as before, and redo applies the same outline again.
- [ ] AC-10 (error): If the Pixel-mode document is indexed, then the Outline is refused with the same message as every other effect and the document is unchanged.
- [ ] AC-11 (error): If the Color field's hex text is not `#RRGGBB` or `#RRGGBBAA`, then the dialog marks the field invalid and does not use that value.
- [ ] AC-12 (always): A thickness outside 1–8 arriving through the wire format is clamped to 1–8, like every other effect parameter, and a wrong number of parameters is an error that changes nothing.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names.

**Effects crate.** `crates/pincel-effects/src/`: `lib.rs`, `spec.rs` (`EffectSpec`, `from_params`; constructors clamp params), `effect.rs`, `image.rs` (`EffectImage`), `kernel.rs`, `error.rs` (`EffectError`: unknown effect, wrong parameter count, buffer length). Groups `adjust/`, `blur/`, `distort/`, `noise/`, `sharpen/`. The outline can live in `distort/` or a new `outline/`. For t ≤ 8, t passes of 3×3 dilation (cross for circle, full for square) are enough; a distance transform also works.

**Colour on the wire.** The wire format is a positional `f64` vector (Decision Log 2026-10-11). Proposed: the colour travels as packed RGBA (u32) in one slot; extend `EffectSpec` accordingly.

**UI catalog.** `ui/src/lib/effects/catalog.ts`: `ParamDef` is `number | bool | choice` (≈L14–25) — no `color` kind yet; `menu.ts` and `labels.ts` beside it. `ui/src/lib/components/EffectDialog.svelte` renders params and the live preview; `ui/src/lib/components/ColorPair.svelte` has swatch styling to reuse. The catalog is guarded by `every_effect_name_is_accepted` (Rust) and `assertCatalogCoversAllEffects` (TS).

**Pixel mode.** `applyEffectParams` / `cancelEffect` in `ui/src/modes/pixel/image-ops.ts` (≈L46), wired in `PixelEditor.svelte`; wasm `run_effect` in `crates/pincel-wasm/src/effects.rs` (≈L45) works on the active cel's buffer, refuses non-RGBA documents ("effects require an RGBA document", ≈L56) and applies `sprite.selection` / `selection_mask`. Note: the effect only rewrites the cel's own rectangle (see Open questions).

**Image mode.** `applyEffect` / `previewEffect` in `ui/src/modes/image/core/controller.ts` (≈L353–369); wasm `apply_effect` / `preview_effect` and `layer_effect_image` in `crates/pincel-image-wasm/src/lib.rs` (≈L1439–1495); selection through `masked_region` (Decision Log 2026-10-12).

**e2e.** Proposed: Pixel mode, 5×5 sprite with one pixel in the centre, Outline with defaults, then Eyedropper read-back at one neighbour (outline colour) and one non-neighbour (transparent), reading the foreground swatch from the DOM; Eyedropper pattern in `ui/e2e/pencil.spec.ts`. Effects pattern: `ui/e2e/effects.spec.ts`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Opaque pixel on the layer's edge, Inside | It is recoloured — the edge counts as transparent |
| Opaque pixel on the layer's edge, Outside | Outline pixels are written only inside the layer; nothing wraps around |
| Layer with no opaque pixel | Nothing changes |
| Fully opaque layer, Outside | Nothing changes — no transparent pixel to colour |
| Fully opaque layer, Inside, t = 1 | Only the border ring of the layer is recoloured |
| Thickness 3, Square, single opaque pixel | A 7×7 block around it is coloured, the centre keeps its colour |
| Selection that contains no opaque pixel but borders one | Outline pixels appear inside the selection next to the opaque pixel |
| Semi-transparent pixel (alpha 1) | Counts as opaque: Outside does not cover it, Inside may recolour it |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Defaults Shape = Square and Thickness = 1 (AC-01) | The legacy plan's e2e test says Outline "with defaults colors the 8 neighbours" of a single pixel | Low |
| Invalid hex text is rejected in the dialog (AC-11) | The colour param accepts `#RRGGBB[AA]` only; any other value has no colour to apply | Low |

## Open questions

- The Decision Log line is not written yet: a `color` parameter kind in the effect catalog, carried as packed RGBA in the positional wire format (bump or extend the 2026-10-11 format).
- §13.4 lists Outline under Phase 2 filters with "inside/outside/both"; the legacy plan has only Outside and Inside. Is a "Both" placement in scope, and is Outline cleared to ship despite its Phase 2 listing (AGENTS.md §9 forbids mixing phases)?
- In Pixel mode an effect only rewrites the active cel's rectangle. When a cel is smaller than the canvas (trimmed cels from `.aseprite` files), should an Outside outline grow the cel to the canvas, or stay clipped at the cel's edge?
- Should the effect get an alpha threshold parameter for what counts as opaque? The legacy plan mentions it as optional.

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

## Decisions

- 2026-10-28 (legacy plan): Opaque means alpha > 0 — Aseprite treats any non-zero alpha as part of the sprite.
- 2026-10-28 (legacy plan): Indexed Pixel documents are handled like every other effect (refused today) — the plan says to follow the existing effects' treatment of indexed sprites, and `pincel-wasm` refuses non-RGBA documents for all effects.
- 2026-10-28 (legacy plan): Until the own colour picker exists, the Color parameter uses the native colour input plus hex text — no new dependency and no wait on `14-color-picker.md`.
