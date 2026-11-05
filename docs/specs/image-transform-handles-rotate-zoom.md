---
status: todo
created: 2026-11-05
size: M
mode: Image
design: [pincel.md §5.4, pincel.md §6.2, pincel.md §15 (2026-10-11 One effect wire format), pincel.md §15 (2026-11-04 effect panel), pincel.md §15 (2026-11-05 MCP tool parity)]
---

# Image Layer ▸ Rotate / Zoom

## Context

paint.net's Layers ▸ Rotate / Zoom turns the active layer by any angle, pans it and zooms it, with a live preview, as one history step. Pincel's Image mode can only flip a layer and turn it by 90° / 180° (`TransformLayer`, `RotateLayer90`); the Photo ▸ Straighten effect rotates by at most ±45° around the center and cannot pan or zoom freely. The owner wants paint.net parity for Image mode.

This spec adds Layer ▸ Rotate / Zoom… with angle, pan, zoom and a resampling quality, previewed live and applied as one undo step. It is the first half of the `image-transform-handles` topic: it builds the arbitrary rotate-and-scale resampler that the on-canvas handles of `image-transform-handles` then reuse, and it needs no live-edit architecture because it runs through the existing effect preview / apply path (Decision Log 2026-10-11 "One effect wire format"), which already gives a preview that leaves the document alone, selection confinement, one undo step, a history name and the MCP `effect.apply` tool.

## Scope

### In scope

- Layer ▸ Rotate / Zoom… in Image mode, with Angle, Pan (X / Y), Zoom and Quality (Nearest / Bilinear / Bicubic)
- A live preview on the canvas while the dialog is open; Apply / Cancel
- Confinement to the selection, like every effect
- One undo step per Apply
- en + de strings for every new label and the history step

### Out of scope

- The Tiling and Preserve-background options of paint.net's dialog — because the topic asks for angle, pan and zoom; both can follow as extra parameters
- Pixel mode — because Pixel mode's arbitrary rotation is the `rotsprite` spec (Layer ▸ Rotate… with Nearest / RotSprite)
- Growing the canvas to fit the rotated layer — because Image layers are canvas-sized and paint.net's Rotate / Zoom keeps the canvas too; Image ▸ Canvas Size exists
- Applying to every layer of a group — because effects act on one raster layer and this command runs as one; a group as active layer is refused (AC-08)
- On-canvas handles — because `image-transform-handles` covers them
- A dedicated MCP tool — because the command runs as an effect, which `effect.apply` already reaches (Decision Log 2026-11-05 "MCP tool parity": one name per capability)

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses Layer ▸ Rotate / Zoom… with a document open, then the effect dialog opens with Angle (−180° to 180°, step 0.1°, default 0°), Pan X / Pan Y (the shift of the layer's center from the canvas center, in percent of the canvas width / height, −100 % to 100 %, step 0.1 %, default 0 %), Zoom (0.1× to 10×, default 1×) and Quality (Nearest / Bilinear / Bicubic, default Bilinear); without a document the item is disabled.
- [ ] AC-02 (state): While the dialog is open, the canvas shows the active layer rotated, panned and zoomed with the current values, and the document and its undo history are unchanged; Cancel or Escape closes the dialog with no change and no undo step.
- [ ] AC-03 (event): When the user clicks Apply, then the active layer shows the previewed result, the history gains exactly one step named "Rotate / Zoom" (English and German), and one Undo restores every pixel of the layer.
- [ ] AC-04 (always): The layer's content turns about the canvas center by the angle, positive clockwise on screen, is scaled by the zoom about that center and then shifted by the pan (Pan X × canvas width / 100 px to the right, Pan Y × canvas height / 100 px down, sub-pixel shifts allowed); the canvas center is the point (width / 2, height / 2) between pixel edges, so on an odd size it is the center of the middle pixel; pixels that land outside the canvas are dropped and canvas pixels no source pixel reaches become transparent.
- [ ] AC-05 (always): Angle 0°, pan 0 % / 0 % and zoom 1× leave every pixel unchanged, on even and odd canvas sizes alike, with every quality, and Apply with these values adds no undo step.
- [ ] AC-06 (option): Where the quality is Nearest, every result pixel has a color (alpha included) that occurs in the source layer or is fully transparent, and an angle of 90° on a square canvas equals Layer ▸ Rotate 90° CW pixel for pixel.
- [ ] AC-07 (state): While a selection exists, only selected pixels change, feathered edges blend the result with the original by their coverage, as every effect does, and pixels outside the selection keep their values; the source of the rotation is the whole layer, not only the selected part.
- [ ] AC-08 (error): If the active layer is locked (itself or through its group) or is a group, then the item shows the locked-layer notice that effects show, and nothing changes.
- [ ] AC-09 (error): If an angle (−180° to 180°), pan (−100 % to 100 %) or zoom (0.1× to 10×) outside its range is typed, then the value becomes the nearest bound; a non-numeric entry keeps the previous value.
- [ ] AC-10 (always): The MCP tool `effect.apply` with the Rotate / Zoom effect and the same parameters produces the same pixels as Apply in the editor.
- [ ] AC-11 (always): Where the quality is Nearest, an angle of 180° equals Image ▸ Rotate 180° pixel for pixel on canvases of odd and even width and height, and a pan of exactly one pixel (100 / width %) at angle 0° and zoom 1× shifts every pixel by one to the right, with the vacated column transparent.

## Technical notes

> Paths are hints from 2026-11-05 and drift — grep for the names.

- Precedent: `photo::straighten` in `crates/pincel-effects/src/photo/straighten.rs` rotates via `distort::edge::{remap, sample}` (inverse mapping, bilinear on premultiplied pixels, `EdgeBehavior::Transparent`). Rotate / Zoom is the general affine case plus Nearest and Bicubic taps; the same resampler serves `image-transform-handles` (`pincel-image-core` already depends on `pincel-effects`).
- `Interpolation` (`crates/pincel-image-core/src/transform/mod.rs`) names the three qualities; the effect's Quality parameter mirrors it as a choice.
- An Image-only entry in `ui/src/lib/effects/catalog.ts` (`modes: IMAGE`) with two number parameters Pan X / Pan Y in percent (the catalog's `point` parameter is a position in [0, 1] of the image area and cannot express a shift to or beyond the edge; percent keeps the fixed ranges the catalog needs while the canvas size varies), opened from the Layer menu instead of the Effects menu; `preview_effect` / `apply_effect` in `crates/pincel-image-wasm/src/lib.rs` do preview, selection masking and the single `SetPixels` step.
- The history kind is the effect id, as `history-panel` specifies for effects.
- **Resampler contract for `image-transform-handles`**: a function from a source buffer of any size (not only the canvas size), read as transparent outside its pixels, through an inverse affine map with sub-pixel center and offset, into a target buffer of any size, with the three qualities. Rotate / Zoom uses it with source = target = the layer; the handles spec uses it with the lifted patch as source (its AC-11, AC-13 test that use).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Angle 180° vs −180° | Same result |
| Zoom 0.1× on a 1×1 canvas | The single pixel is sampled or the canvas becomes transparent; no error |
| Pan 100 % / 0 % at zoom 1× | The content moves a full canvas width right; the canvas becomes transparent inside the selection; Apply is one undo step |
| Odd canvas (5×5), angle 90°, Nearest | Equals Layer ▸ Rotate 90° CW (AC-06; the center is the middle pixel's center) |
| Fully transparent layer | Stays transparent; Apply adds no undo step, because no pixel changed (as in AC-05) |
| Selection near the edge, angle 45° | Only selected pixels change (AC-07) |
| Cancel after moving every control | Document and history unchanged (AC-02) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Rotate / Zoom runs as an Image-only effect of `pincel-effects` | The effect path already gives preview, selection confinement, one step, history name and MCP; a new command channel would duplicate it | Low |
| Pan is a shift in percent of the canvas size, −100 % to 100 % | The catalog needs fixed ranges independent of the canvas; ±100 % reaches every position from fully off to fully off | Low |
| Zoom range 0.1× – 10× | Covers shrinking to a thumbnail and a 10× blow-up without huge sampling cost | Low |
| Angle step 0.1° | Matches Straighten's angle control | Low |
| Positive angles turn clockwise | Straighten and `rotsprite` use the same convention | Low |

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

## Decisions

- 2026-11-05: Split from `image-transform-handles` — because together they exceed size L; this half needs no live-edit decision and builds the shared resampler, so it goes first.
- 2026-11-05: Clarification pass — Pan is a shift from the canvas center in percent of the canvas size (AC-01, AC-04, AC-09), because the catalog's point parameter is a position in [0, 1] and cannot reach beyond the edge, and fixed percent ranges fit a catalog that does not know the canvas size. The canvas center is (width / 2, height / 2), so odd sizes turn about the middle pixel's center (AC-04, AC-11). The resampler's contract for `image-transform-handles` (any source size, transparent outside, sub-pixel center) is in Technical notes and is proved by that spec's AC-11 / AC-13.
