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

This spec adds Layer ▸ Rotate… for any angle with two methods: **Nearest** (fast, jaggy) and **RotSprite**. Without a selection it rotates the active layer's cel in the current frame; with a selection it rotates the selection content. It sits in the Layer menu because the Image menu holds canvas-wide transforms and the Layer menu the cel transforms.

This is part C of the legacy plan `08-pixel-tools.md`; `dithered-gradient` and `shading-ink` are the other parts.

## Scope

### In scope

- Rotation by any angle from −180° to 180° with the methods Nearest and RotSprite
- Rotating the selection content when a selection exists
- Growing the target bounds so the rotated result is never cropped
- A Rotate dialog (angle as number plus slider, method) with a live preview, reached from Layer ▸ Rotate…
- An input size cap for RotSprite (512 px per side) with a notice
- One undo entry per rotation
- i18n keys for every new label, in `en.ts` and `de.ts`

### Out of scope

- A "Rotate (pixel art)" command in Image mode — because the legacy plan only mentions it as a possible later use
- Other resampling methods from §17.3 #1232 (Scale2x/3x/4x as a resize, hqx) — because the legacy plan only adds rotation
- Interactive on-canvas rotation handles — because the legacy plan specifies a dialog with an angle field

## Acceptance criteria

- [ ] AC-01 (always): Rotating by 0° leaves every pixel and the bounds of the target unchanged, with either method.
- [ ] AC-02 (option): Where the method is RotSprite, rotating by 90° gives exactly the same pixels and bounds as the existing 90° clockwise rotation.
- [ ] AC-03 (always): Every pixel of a rotation result, with either method, has a color (alpha included) that occurs in the source or is fully transparent; no new colors are created.
- [ ] AC-04 (always): The rotated result is never cropped: the target's bounds grow to hold the whole rotated content.
- [ ] AC-05 (always): One rotation is one undo entry; undo restores the target's pixels and bounds exactly.
- [ ] AC-06 (state): While a selection exists, the rotation turns only the selection content, and pixels outside the selection that the rotated content does not cover keep their values.
- [ ] AC-07 (event): When the user chooses Layer ▸ Rotate…, a dialog opens with an angle control (number field plus slider, −180° to 180°) and a method choice (Nearest / RotSprite).
- [ ] AC-08 (state): While the Rotate dialog is open, the canvas shows a live preview of the current angle and method, and the document stays unchanged until Apply; Cancel closes the dialog without a change or an undo entry.
- [ ] AC-09 (error): If the method is RotSprite and the input is wider or taller than 512 pixels, then a notice explains the limit and nothing is rotated.
- [ ] AC-10 (error): If the target layer is locked or there is no rotatable image content, then nothing changes and no undo entry is added.
- [ ] AC-11 (error): If an angle outside −180° to 180° is typed into the angle field, then it becomes the nearest bound.
- [ ] AC-12 (always, manual: visual quality judgement): A RotSprite rotation of a typical sprite (outlined shapes, 1-px lines) at 30° and 45° shows fewer broken or doubled line pixels than a Nearest rotation of the same sprite.
- [ ] AC-13 (state): While no selection exists, only the active layer's cel in the current frame rotates; other layers and frames keep their pixels and bounds.

## Technical notes

> Paths and line numbers are hints from 2026-10-28 and drift — grep for the names.

**Current state**

- `crates/pincel-core/src/command/transform_support.rs` — `Orientation` (≈L10: flips, `Rotate90Cw`, `Rotate90Ccw`, `Rotate180`) and `Raster` (≈L85–230: `get`, `put`, `blit`, `crop`, `oriented`, `orient_region`, `resample`). Works on canvas-sized straight-alpha RGBA8 buffers.
- `TransformCel { layer, frame, op, region, prior }` in `command/transform_cel.rs` (≈L20) and `TransformCanvas` in `command/transform_canvas.rs`; undo by stored prior cel.
- wasm: `transformLayer` / `transformCanvas` in `crates/pincel-wasm/src/transforms.rs`.
- UI: `ui/src/modes/pixel/image-ops.ts` (≈L87–97: `layer:rotate_90_cw` … `canvas:rotate_90_ccw`). The Image menu (`ui/src/modes/pixel/menus.ts` ≈L168) holds the canvas transforms; the Layer menu holds the layer (cel) transforms.
- Pixel mode has a live effect-preview pattern: `previewEffectParams` in `image-ops.ts` (≈L24) draws `ed.doc.previewEffect(...)` without touching the document.

**Design (from the legacy plan)**

- RotSprite: (1) upscale ×8 with three Scale2x (EPX) passes; (2)+(3) for each output pixel, sample the ×8 source with nearest-neighbour at the output pixel's inverse-rotated centre (block-centre rule, decided 2026-10-29) — never build a ×8 rotated buffer; (4) treat fully transparent as a color so edges don't bleed.
- Pure functions `scale2x(raster)`, `rotsprite(raster, angle_deg)`, `rotate_nearest(...)` in `pincel-core` (`command/transform_support.rs` next to `Raster`, or a new `transform/rotate.rs`); they move to `pincel-effects` only if Image mode adopts them.
- Command `RotateCel { layer, frame, angle, method, within: Option<Rect> }`: grows the cel bounds (Aseprite cels carry their own bounds), undo by stored prior cel.
- Performance note from the plan: ×8 of a 256×256 cel is 2048² ≈ 4 M pixels per pass. The RotSprite cap of 512 per side keeps the ×8 source at 4096² ≈ 67 MB; Nearest needs no cap beyond the canvas limits.
- Suggested tests: `scale2x_matches_reference` (hand-made 3×3 → 6×6 fixture), `rotsprite_0_degrees_is_identity`, `rotsprite_90_matches_orient_rotate90`, `rotsprite_keeps_palette_no_new_colors`, `rotate_cel_grows_bounds_and_reverts`, and a proptest: rotating by 0° or 360° is the identity for any small raster.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 1×1 cel rotated by 45° | The single pixel survives; no new colors (AC-03) |
| Fully transparent cel | Result stays fully transparent; no error |
| Angle 180° vs −180° | Both produce the same result |
| Rotated content would extend beyond the canvas | Bounds grow; nothing is cropped (AC-04) |
| Selection near the canvas edge rotated by 45° | Rotated content may extend outside the selection; pixels outside it that are not covered keep their values (AC-06) |
| 513×10 cel, RotSprite | Notice, no rotation (AC-09) |
| 513×10 cel, Nearest | Rotates; Nearest has no cap beyond the canvas limits |
| Cancel after moving the slider | Document and undo history unchanged (AC-08) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Positive angles turn clockwise, so +90° matches the existing 90° clockwise rotation | The plan requires RotSprite at 90° to equal the exact 90° path and names `Rotate90Cw` | Low |
| The angle accepts whole degrees | The plan gives a −180…180 number plus slider without a step | Low |
| Rotation stays RGBA-only like the existing transforms | `Raster` works on RGBA8 buffers and indexed painting is Phase 2 (§3.7) | Low |
| Typed angles outside −180…180 are clamped to the nearest bound | The control has that range; the plan gives no error behavior | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `rotate_by_zero_or_full_turns_is_identity_for_both_methods` (`crates/pincel-core/src/command/rotsprite.rs`); `rotate_cel_by_zero_keeps_every_pixel_and_the_bounds` (`crates/pincel-core/src/command/rotate_cel.rs`) | built |        |
| AC-02 | `rotsprite_90_matches_orient_rotate90` (`crates/pincel-core/src/command/rotsprite.rs`); `rotate_cel_rotsprite_90_matches_the_quarter_turn` (`crates/pincel-core/src/command/rotate_cel.rs`) | built |        |
| AC-03 | `rotation_keeps_palette_no_new_colors`, `single_pixel_survives_45_degrees`, `fully_transparent_stays_transparent` (`crates/pincel-core/src/command/rotsprite.rs`); `rotate_cel_adds_no_colours` (`crates/pincel-core/src/command/rotate_cel.rs`) | built |        |
| AC-04 | `rotation_grows_bounds_and_never_crops` (`crates/pincel-core/src/command/rotsprite.rs`); `rotate_cel_grows_bounds_and_reverts` (`crates/pincel-core/src/command/rotate_cel.rs`) | built |        |
| AC-05 | `rotate_cel_grows_bounds_and_reverts` (`crates/pincel-core/src/command/rotate_cel.rs`); `rotate_layer_turns_only_the_active_cel_in_one_undo_step` (`crates/pincel-wasm/src/rotate.rs`); `Apply commits the previewed rotation as one undo step (AC-08)` (`ui/e2e/rotsprite.spec.ts`) | built |        |
| AC-06 | `rotate_cel_within_selection_keeps_uncovered_outside_pixels` (`crates/pincel-core/src/command/rotate_cel.rs`); `rotate_layer_with_a_selection_turns_only_its_content` (`crates/pincel-wasm/src/rotate.rs`) | built |        |
| AC-07 | `Layer ▸ Rotate… opens a dialog with angle field, slider and method (AC-07)` (`ui/e2e/rotsprite.spec.ts`) | built |        |
| AC-08 | `the dialog previews live; Cancel leaves the document and history alone (AC-08)`, `Apply commits the previewed rotation as one undo step (AC-08)` (`ui/e2e/rotsprite.spec.ts`); `preview_rotate_leaves_document_and_history_untouched` (`crates/pincel-wasm/src/rotate.rs`) | built |        |
| AC-09 | `rotsprite_input_over_512_is_refused_and_nearest_is_not` (`crates/pincel-core/src/command/rotate_cel.rs`); `rotate_layer_refuses_rotsprite_over_the_cap` (`crates/pincel-wasm/src/rotate.rs`); `RotSprite over 512 px shows a notice and rotates nothing (AC-09)` (`ui/e2e/rotsprite.spec.ts`) | built |        |
| AC-10 | `rotate_cel_without_visible_content_changes_nothing` (`crates/pincel-core/src/command/rotate_cel.rs`); `rotate_layer_on_a_locked_layer_or_without_content_changes_nothing` (`crates/pincel-wasm/src/rotate.rs`); `a locked layer or an empty one rotates nothing (AC-10)` (`ui/e2e/rotsprite.spec.ts`) | built |        |
| AC-11 | `a typed angle outside ±180° snaps to the nearest bound (AC-11)` (`ui/e2e/rotsprite.spec.ts`) | built |        |
| AC-12 |      | open   |        |
| AC-13 | `rotate_cel_leaves_other_layers_and_frames_alone` (`crates/pincel-core/src/command/rotate_cel.rs`); `rotate_layer_turns_only_the_active_cel_in_one_undo_step` (`crates/pincel-wasm/src/rotate.rs`) | built |        |

## Decisions

- RotSprite as Scale2x ×3, nearest-neighbour rotation, ×8 downscale, with fully transparent treated as a color — because it is the established pixel-art rotation and keeps edges from bleeding (legacy plan).
- Rotation grows the bounds instead of cropping — because Aseprite cels carry their own bounds, so nothing needs to be lost (legacy plan).
- The live preview reuses Pixel mode's existing effect-preview pattern — because the plan prefers it where Pixel mode has one, and it does (`previewEffectParams`).
- 2026-10-29: Without a selection Layer ▸ Rotate… rotates only the active layer's cel in the current frame (`RotateCel`, AC-13); the entry lives in the Layer menu, not the Image menu (Context, AC-07) — RotSprite is used on one sprite part, the existing split puts canvas-wide transforms in the Image menu and cel transforms in the Layer menu, and running ×8 over every cel of every frame would multiply cost and memory (owner accepted the recommendation).
- 2026-10-29: The RotSprite downscale is a block-centre sample: each output pixel samples the ×8 source at its inverse-rotated centre, recorded in a Decision Log entry — it is deterministic and tie-free, makes AC-02 (exact 90°) straightforward, needs no ×8 rotated buffer, and mirrors Aseprite's parallelogram sampling (owner accepted the recommendation).
- 2026-10-29: The rotation code lives in `pincel-core` beside `Raster` as pure functions and moves to `pincel-effects` only if Image mode adopts it, recorded in a Decision Log entry — it works on `Raster` and changes bounds, unlike the same-size buffer effects in `pincel-effects`, and Image mode does not need it yet (owner accepted the recommendation).
- 2026-10-29: The RotSprite input cap drops from 1024 to 512 px per side (AC-09), and no ×8 rotated buffer is ever built; Nearest has no cap beyond the canvas limits — 8192² ≈ 268 MB per buffer plus intermediates risks out-of-memory in wasm on iPad / phones, the PWA reference target (§17.4) (owner accepted the recommendation).
- 2026-11-06 (spec-build): What turns — without a selection the bounding box of the cel's visible (alpha > 0) pixels, with one the selection rect (and its mask), each about its own centre — because a canvas-sized cel would otherwise turn about the canvas centre and hit the RotSprite cap for a small sprite, and the selection centre is the pivot users expect.
- 2026-11-06 (spec-build): "The existing 90° clockwise rotation" (AC-02) is `Raster::oriented(Rotate90Cw)`, the pixel path of `TransformCel`; both methods take that exact path for 0°, ±90° and 180°, and AC-02 is tested against it and against `TransformCel` on a full square cel — because that is the only existing 90° path in the core.
- 2026-11-06 (spec-build): The output box of a non-quarter turn keeps the source side's parity (one pixel larger when needed), so output pixel centres line up with the source centre — because otherwise a 1×1 cel at 45° samples only between pixels and vanishes (Edge cases table).
- 2026-11-06 (spec-build): Visible rotated pixels replace what lies under them (no blending), lifted pixels become transparent, fully transparent pixels keep their bytes — because blending would create new colours (AC-03) and AC-01 asks for every pixel unchanged at 0°.
- 2026-11-06 (spec-build): The cel grows to the union of its old bounds and the visible rotated content and may extend past the canvas; it never shrinks — because AC-04 forbids cropping and Aseprite cels carry their own bounds.
- 2026-11-06 (spec-build): The selection stays where it is after a rotation — because the spec only asks the content to turn and rotating the mask is not specified.
- 2026-11-06 (spec-build): A locked layer or nothing visible to rotate gives a status line and no dialog; the core returns `EmptyRegion`, `rotateLayer` returns `false` — because AC-10 only asks that nothing changes, and an empty dialog would invite a useless Apply.
- 2026-11-06 (spec-build): The RotSprite cap lives in the core (`CommandError::RotSpriteTooLarge`); the dialog shows the notice inline while RotSprite is chosen, disables Apply and previews the unchanged document — because the notice should appear before the user commits, and the core must refuse regardless of the UI.
- 2026-11-06 (spec-build): The dialog opens with 0° and RotSprite, sits top-right without a backdrop tint, and its i18n keys use the `pxRotate.*` prefix — because RotSprite is the reason for the dialog, the canvas preview must stay visible, and Image mode's parallel rotate work may add `rotate.*` keys.
- 2026-11-06 (spec-build): The suggested proptest became deterministic xorshift loops (`rotsprite.rs` tests) — because `pincel-core` has no `proptest` dev-dependency and adding one is a new dependency.
- 2026-11-06 (spec-build): New public API: `RotateCel` (`new`, `within`, `masked`, `input_rect`), `RotateMethod` (`name`, `from_name`), `ROTSPRITE_MAX_SIDE`, `CommandError::RotSpriteTooLarge`, `AnyCommand::RotateCel`; wasm `rotateLayer`, `previewRotate`, `rotateInputSize`, `rotspriteMaxSide`.
- 2026-11-06 (spec-build) red runs, each against a temporary break, reverted: `scale2x_matches_reference` (E3 rule swapped); `rotate_by_zero_or_full_turns_is_identity_for_both_methods` (0° flipped); `rotsprite_90_matches_orient_rotate90` (quarter-turn path off); `rotation_keeps_palette_no_new_colors`, `fully_transparent_stays_transparent` (colour written outside the source); `rotation_grows_bounds_and_never_crops` (no growth); `single_pixel_survives_45_degrees` (no parity rule); `angle_180_and_minus_180_match` (no normalisation); `rotate_method_names_round_trip`; command tests: lift every pixel, grow to the whole box, counter-clockwise, blend, revert no-op, ignore mask, ignore selection, cap off by one, empty content rotates, touch the next frame; wasm tests: ignore lock, ignore selection, preview without rotation, cap swallowed, extra undo entry, next frame / other layer also rotated (the AC-13 fixture first stayed green with 1-px cels and was widened to 2-px bars); e2e: slider min −90, no clamp, no preview, Apply not disabled, no lock check (all six red), Cancel without recompose and a doubled rotation (both AC-08 specs red). Identity at 0° holds even without the quarter-turn path, so its red run uses a flipping 0° instead.
