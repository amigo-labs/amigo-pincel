---
status: todo
created: 2026-10-28
size: L
mode: Pixel
design: [pincel.md §5.2, pincel.md §6.1, pincel.md §15, pincel.md §17.3]
---

# Arbitrary rotation with RotSprite

## Context

Pixel mode can only rotate by 90° and 180° and flip (`Orientation`, `TransformCel`, `TransformCanvas`). Pixel artists regularly need other angles — a tilted sword, a swinging arm — and a plain nearest-neighbour rotation by an arbitrary angle leaves jaggy, broken lines. **RotSprite** is the pixel-art standard for this: it keeps lines clean and adds no new colors.

§17.3 lists RotSprite under request #1232 ("Configurable resampling: Nearest, Scale2x/3x/4x, hqx, RotSprite"). Rotation by an arbitrary angle is not in §5.2 yet.

This spec adds Image ▸ Rotate… for any angle with two methods: **Nearest** (fast, jaggy) and **RotSprite**. With a selection it rotates the selection content.

This is part C of the legacy plan `08-pixel-tools.md`; `dithered-gradient` and `shading-ink` are the other parts.

## Scope

### In scope

- Rotation by any angle from −180° to 180° with the methods Nearest and RotSprite
- Rotating the selection content when a selection exists
- Growing the target bounds so the rotated result is never cropped
- A Rotate dialog (angle as number plus slider, method) with a live preview, reached from Image ▸ Rotate…
- An input size cap with a notice
- One undo entry per rotation
- i18n keys for every new label, in `en.ts` and `de.ts`

### Out of scope

- A "Rotate (pixel art)" command in Image mode — because the legacy plan only mentions it as a possible later use; where the algorithm lives is still open (see Open questions)
- Other resampling methods from §17.3 #1232 (Scale2x/3x/4x as a resize, hqx) — because the legacy plan only adds rotation
- Interactive on-canvas rotation handles — because the legacy plan specifies a dialog with an angle field

## Acceptance criteria

- [ ] AC-01 (always): Rotating by 0° leaves every pixel and the bounds of the target unchanged, with either method.
- [ ] AC-02 (option): Where the method is RotSprite, rotating by 90° gives exactly the same pixels and bounds as the existing 90° clockwise rotation.
- [ ] AC-03 (always): Every pixel of a rotation result, with either method, has a color (alpha included) that occurs in the source or is fully transparent; no new colors are created.
- [ ] AC-04 (always): The rotated result is never cropped: the target's bounds grow to hold the whole rotated content.
- [ ] AC-05 (always): One rotation is one undo entry; undo restores the target's pixels and bounds exactly.
- [ ] AC-06 (state): While a selection exists, the rotation turns only the selection content, and pixels outside the selection that the rotated content does not cover keep their values.
- [ ] AC-07 (event): When the user chooses Image ▸ Rotate…, a dialog opens with an angle control (number field plus slider, −180° to 180°) and a method choice (Nearest / RotSprite).
- [ ] AC-08 (state): While the Rotate dialog is open, the canvas shows a live preview of the current angle and method, and the document stays unchanged until Apply; Cancel closes the dialog without a change or an undo entry.
- [ ] AC-09 (error): If the rotation input is wider or taller than 1024 pixels, then a notice explains the limit and nothing is rotated.
- [ ] AC-10 (error): If the target layer is locked or there is no rotatable image content, then nothing changes and no undo entry is added.
- [ ] AC-11 (error): If an angle outside −180° to 180° is typed into the angle field, then it becomes the nearest bound.
- [ ] AC-12 (always, manual: visual quality judgement): A RotSprite rotation of a typical sprite (outlined shapes, 1-px lines) at 30° and 45° shows fewer broken or doubled line pixels than a Nearest rotation of the same sprite.

## Technical notes

> Paths and line numbers are hints from 2026-10-28 and drift — grep for the names.

**Current state**

- `crates/pincel-core/src/command/transform_support.rs` — `Orientation` (≈L10: flips, `Rotate90Cw`, `Rotate90Ccw`, `Rotate180`) and `Raster` (≈L85–230: `get`, `put`, `blit`, `crop`, `oriented`, `orient_region`, `resample`). Works on canvas-sized straight-alpha RGBA8 buffers.
- `TransformCel { layer, frame, op, region, prior }` in `command/transform_cel.rs` (≈L20) and `TransformCanvas` in `command/transform_canvas.rs`; undo by stored prior cel.
- wasm: `transformLayer` / `transformCanvas` in `crates/pincel-wasm/src/transforms.rs`.
- UI: `ui/src/modes/pixel/image-ops.ts` (≈L87–97: `layer:rotate_90_cw` … `canvas:rotate_90_ccw`). The Image menu (`ui/src/modes/pixel/menus.ts` ≈L168) holds the canvas transforms; the Layer menu holds the layer (cel) transforms.
- Pixel mode has a live effect-preview pattern: `previewEffectParams` in `image-ops.ts` (≈L24) draws `ed.doc.previewEffect(...)` without touching the document.

**Design (from the legacy plan)**

- RotSprite: (1) upscale ×8 with three Scale2x (EPX) passes; (2) rotate with nearest-neighbour sampling around the centre into an ×8 target large enough for the rotated bounds; (3) downscale ×8 by one rule per 8×8 block (block-centre sample, or the block's most common color — see Open questions); (4) treat fully transparent as a color so edges don't bleed.
- Pure functions `scale2x(raster)`, `rotsprite(raster, angle_deg)`, `rotate_nearest(...)` on `Raster` or in a new `transform/rotate.rs` in `pincel-core` — or in `pincel-effects` (see Open questions).
- Command `RotateCel { layer, frame, angle, method, within: Option<Rect> }`: grows the cel bounds (Aseprite cels carry their own bounds), undo by stored prior cel.
- Performance note from the plan: ×8 of a 256×256 cel is 2048² ≈ 4 M pixels per pass.
- Suggested tests: `scale2x_matches_reference` (hand-made 3×3 → 6×6 fixture), `rotsprite_0_degrees_is_identity`, `rotsprite_90_matches_orient_rotate90`, `rotsprite_keeps_palette_no_new_colors`, `rotate_cel_grows_bounds_and_reverts`, and a proptest: rotating by 0° or 360° is the identity for any small raster.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 1×1 cel rotated by 45° | The single pixel survives; no new colors (AC-03) |
| Fully transparent cel | Result stays fully transparent; no error |
| Angle 180° vs −180° | Both produce the same result |
| Rotated content would extend beyond the canvas | Bounds grow; nothing is cropped (AC-04) |
| Selection near the canvas edge rotated by 45° | Rotated content may extend outside the selection; pixels outside it that are not covered keep their values (AC-06) |
| 1025×10 cel | Notice, no rotation (AC-09) |
| Cancel after moving the slider | Document and undo history unchanged (AC-08) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Positive angles turn clockwise, so +90° matches the existing 90° clockwise rotation | The plan requires RotSprite at 90° to equal the exact 90° path and names `Rotate90Cw` | Low |
| The angle accepts whole degrees | The plan gives a −180…180 number plus slider without a step | Low |
| The 1024 cap applies per side (width or height) | The plan writes "1024²" | Low |
| Rotation stays RGBA-only like the existing transforms | `Raster` works on RGBA8 buffers and indexed painting is Phase 2 (§3.7) | Low |
| Typed angles outside −180…180 are clamped to the nearest bound | The control has that range; the plan gives no error behavior | Low |

## Open questions

- What does Image ▸ Rotate… rotate without a selection: the active layer's cel in the current frame (as `RotateCel { layer, frame }` suggests), or the whole canvas across all layers and frames (as the other Image-menu transforms do)? Should it also appear in the Layer menu?
- Which downscale rule does RotSprite use: the sample at each 8×8 block's centre, or the block's most common color? The plan asks to choose one and record it in a Decision Log entry.
- Where does the rotation code live: `pincel-core` (Pixel mode only) or `pincel-effects` (so Image mode could get "Rotate (pixel art)" later)? The plan asks to decide and record it in a Decision Log entry; the §5.2 / menu entry is not written either.
- Is the 1024 cap safe? At ×8 a 1024×1024 input becomes 8192×8192 (≈268 MB of RGBA per buffer) in wasm memory; the plan's "fine" estimate was for 256×256.

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

- RotSprite as Scale2x ×3, nearest-neighbour rotation, ×8 downscale, with fully transparent treated as a color — because it is the established pixel-art rotation and keeps edges from bleeding (legacy plan).
- Rotation grows the bounds instead of cropping — because Aseprite cels carry their own bounds, so nothing needs to be lost (legacy plan).
- The live preview reuses Pixel mode's existing effect-preview pattern — because the plan prefers it where Pixel mode has one, and it does (`previewEffectParams`).
