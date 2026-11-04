---
status: todo
created: 2026-11-04
size: M
mode: both
design: [pincel.md §9.2, pincel.md §15 (2026-10-11 "One effect wire format", 2026-10-29 colour slot, 2026-11-04 paint.net catalog)]
---

# Effect catalog v2: paint.net menu structure, per-mode filter, new parameter kinds

## Context

The owner wants every effect and adjustment paint.net has (official list: paint.net/doc/latest/Effects*Menu.html and AdjustmentsMenu.html). Today's catalog has 20 entries in four effect submenus (Blur, Sharpen, Distort, Noise) plus a flat Adjustments menu, identical in both modes, and three parameter kinds (number, switch, choice).

This spec is the foundation the following effect specs build on (`adjustments-paintnet`, `effects-blurs`, `effects-distort`, `effects-object-stylize-color-noise`, `effects-artistic-photo`, `effects-render`). It changes no effect's pixels. It:

1. reorganises the Effects menu into paint.net's nine submenus and moves today's entries into them;
2. shows each mode only the effects that make sense there (owner: "only the effects that make sense in Pixel mode"). The rule: Pixel mode drops *neighbourhood* effects that mix a pixel with its neighbours (blurs, sharpening, Edge Detect, Emboss, Relief, Reduce Noise) — they turn hard pixel edges into soft, anti-aliased ones; it keeps *per-pixel* colour operations (every adjustment, Add Noise), which change colours but leave every edge where it is;
3. adds the parameter kinds the paint.net effects need: angle, point, random seed and linked pair.

Decision Log 2026-11-04 ("Effect catalog follows paint.net's menu structure") records the structure, the per-mode filter and that every algorithm is written clean-room from paint.net's documented behaviour — paint.net has been closed source since 3.36, and pixel-identical results are not a goal.

The colour parameter kind is not part of this spec: `outline-effect` specifies it (wire slot `0xRRGGBBAA`, Decision Log 2026-10-29) and builds it with the first effect that uses it.

**Terms.** *Effects menu* / *Adjustments menu*: the two top-level menus both editors show. *Submenu*: a group inside the Effects menu. *Entry*: one effect or adjustment in a menu. *Effect dialog*: the dialog an entry with parameters opens (today `EffectDialog`; `effect-panel` turns it into a docked panel — this spec's ACs hold for either). *Wire format*: the effect name plus the flat parameter vector both cores accept (Decision Log 2026-10-11). *Image area*: the whole document canvas (width × height of the document), independent of the active layer's bounds and of any selection.

## Scope

### In scope

- Effects menu with paint.net's submenus, in paint.net's order, filtered per mode; Adjustments menu filtered per mode
- Today's 20 entries moved into the new submenus; Box Blur shown as "Square Blur"
- Alphabetical order of entries inside each submenu and the Adjustments menu, in the UI language
- Parameter kinds angle, point, random seed and linked pair in the effect dialog, applied to the existing effects listed in the table below
- en + de strings for submenus, the renamed entry and the new controls
- Revision of `effects-adjustments` AC-01 (menu contents) and AC-02 (control per kind)

### Out of scope

- New effects or adjustments, and new parameters on existing effects — because the six follow-up specs add them; this spec only restructures what exists
- The colour parameter kind — because `outline-effect` owns it (Decision Log 2026-10-29)
- Changing any effect's output, wire name or parameter order — because the agent tools (MCP), saved scripts and both cores share the wire format; the restructure is UI only
- Restricting the wire format per mode — because the Pixel-mode core and its agent tools keep accepting every effect: an agent may still blur a sprite on purpose, and the filter is about which menu entries a person sees
- A favourites or recently-used effects list, or "Repeat last effect" — because they need remembered per-user state and a new shortcut, which is independent of how the menus are structured

## Acceptance criteria

- [ ] AC-01 (always): The Effects menu lists its submenus in the order Artistic, Blurs, Color, Distort, Noise, Object, Photo, Render, Stylize, and shows a submenu only when it has at least one entry in the current mode; the Adjustments menu stays a flat list.
- [ ] AC-02 (always): In Image mode every entry of this spec's mapping table appears in the submenu the table gives: Blurs (Gaussian Blur, Motion Blur, Radial Blur, Square Blur), Noise (Add Noise, Reduce Noise), Photo (Sharpen, Unsharp Mask), Stylize (Edge Detect, Emboss, Relief) and the Adjustments menu (Brightness / Contrast, Color Balance, Curves, Grayscale, Hue / Saturation, Invert Colors, Levels, Posterize, Threshold); the follow-up effect specs add entries and rename Grayscale, which this AC does not forbid.
- [ ] AC-03 (always): Pixel mode's menus show exactly the entries whose catalog modes include Pixel mode — of this spec's entries, Noise ▸ Add Noise and the nine adjustments; no blur, sharpen, Edge Detect, Emboss, Relief or Reduce Noise entry appears anywhere in Pixel mode's menus.
- [ ] AC-04 (always): Inside each submenu and in the Adjustments menu the entries are sorted alphabetically by the name shown in the current UI language, compared with that language's collation rules, and switching the UI language re-sorts them (in English Brightness / Contrast sorts before Color Balance, in German Farbbalance before Helligkeit / Kontrast).
- [ ] AC-05 (always): An entry with parameters ends in "…"; every entry is disabled while no document is open (unchanged from `effects-adjustments`).
- [ ] AC-06 (option): Where the UI language is German, the submenus read Künstlerisch, Weichzeichnen, Farbe, Verzerren, Rauschen, Objekt, Foto, Rendern, Stilisieren, and Square Blur reads "Quadratisches Weichzeichnen".
- [ ] AC-07 (always): For every entry that existed before this spec, the parameter vector sent at its default values, and after changing each parameter once, equals the vector the catalog sent before this spec for the same control values (a test pins the vectors of the previous catalog); so every applied effect gives the same pixels as before (a later spec that adds a parameter to an entry updates that entry's pin).
- [ ] AC-08 (always): The Pixel-mode core and its agent tools still accept every effect name in the wire format, including the effects AC-03 hides from Pixel mode's menus.
- [ ] AC-09 (state): While the effect dialog shows an angle parameter (Motion Blur Angle, Emboss Angle, Relief Angle), the user can set it both by pointing — a round dial whose handle points in the angle's direction — and by typing degrees in a number field; dragging the handle sets the angle to the direction from the dial's centre to the pointer, rounded to whole degrees in 0–359, in the effects' own convention (0° points right, 90° points down, as the image's y axis runs); typing a number moves the handle; a typed value outside 0–360 wraps around (370 becomes 10, −10 becomes 350).
- [ ] AC-10 (state): While the effect dialog shows a point parameter (Radial Blur Center), it shows X and Y as number fields in percent of the image area (0–100 % each, default 50 / 50, the centre — today's default), and the effect acts around that point.
- [ ] AC-11 (event): When the user presses the point parameter's "Pick on canvas" button and then presses a mouse, pen or single touch on the canvas, then the point is set to the pressed position (percent of the image area, rounded to 0.1, clamped to 0–100) and follows the pointer until it is released, the preview updates, and no tool stroke or document edit happens; picking then ends. Escape or a second press of the button ends picking without a change and keeps the dialog open; OK or Enter while picking applies the current values. Panning (Space-drag, middle button, two fingers) still pans while picking is armed.
- [ ] AC-12 (state): While the effect dialog shows a random-seed parameter (Add Noise Seed), it shows the seed number and a "Reseed" button; pressing Reseed sets a new seed different from the current one, in the parameter's range, and the preview updates; the same seed always gives the same pixels.
- [ ] AC-13 (state): While the effect dialog shows a linked pair (Square Blur Width and Height), it shows both values and a link toggle that is on by default; while linked, changing either value sets the other to the same value; while unlinked, each changes alone.
- [ ] AC-14 (event): When the user presses Reset, then every parameter returns to its default, including the angle, point, seed and link toggle.
- [ ] AC-15 (always): Every new control (dial, point fields, Pick on canvas, Reseed, link toggle) is reachable and operable by keyboard and has an accessible name in the UI language; the dial's number field accepts arrow keys like every number field.

## Technical notes

Hints, not contracts — paths drift; grep for the names (verified 2026-11-04).

**Catalog mapping** (`ui/src/lib/effects/catalog.ts`; wire ids unchanged):

| Wire id | Submenu | Name (en / de) | Modes | Parameter kinds |
| ------- | ------- | -------------- | ----- | --------------- |
| `gaussian_blur` | Blurs | Gaussian Blur / Gaußscher Weichzeichner | Image | Radius: number |
| `box_blur` | Blurs | Square Blur / Quadratisches Weichzeichnen | Image | Width + Height: linked pair |
| `motion_blur` | Blurs | Motion Blur / Bewegungsunschärfe | Image | Distance: number; Angle: angle 0–360 |
| `radial_blur` | Blurs | Radial Blur / Radialer Weichzeichner | Image | Amount: number; Center X/Y: point; Kind: choice |
| `sharpen`, `unsharp_mask` | Photo | Sharpen, Unsharp Mask | Image | unchanged |
| `edge_detect` | Stylize | Edge Detect | Image | unchanged |
| `relief` | Stylize | Relief | Image | Angle: angle; Amount unchanged |
| `emboss` | Stylize | Emboss | Image | Angle: angle; rest unchanged |
| `add_noise` | Noise | Add Noise | both | Seed: random seed 0–9999 |
| `reduce_noise` | Noise | Reduce Noise | Image | unchanged |
| nine adjustments | Adjustments | unchanged | both | unchanged |

- `EffectDef` gains a mode list (`'pixel' | 'image'`) and the submenu id; `EffectGroup` becomes the nine submenu ids plus `adjust`. `effectMenus` (`ui/src/lib/effects/menu.ts`) takes the mode; both editors call it (`ImageEditor.svelte`, `modes/pixel/menus.ts`). i18n keys `effect.group.<id>` in `en.ts` / `de.ts`; drop the four old group keys.
- `ParamDef` gains the kinds; `toParams` keeps the wire order (a point is two slots, a linked pair two slots, a seed and an angle one slot each). The link toggle is UI state, not a wire slot.
- Pick on canvas needs the editor to route the next canvas press to the dialog instead of the active tool; both editors already block tool input while `ui.modalOpen`-style flags are set. In this spec only Image mode has a point parameter (Radial Blur); Pixel mode needs the routing once one of its entries gets a point parameter.
- `assertCatalogCoversAllEffects` and the Rust `every_effect_name_is_accepted` test stay green — no id changes.
- E2e: `ui/e2e/effects.spec.ts` opens "Effects ▸ Blur ▸ Gaussian Blur…" today; the submenu is now "Blurs".

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A mode in which a submenu has no entries (Pixel: Blurs, Photo, Stylize) | The submenu is not shown at all (AC-01) |
| Angle typed outside 0–360 | Wraps around (AC-09), because an angle is circular: 370° is the same direction as 10° |
| Pick on canvas, then the user presses outside the image (pasteboard) | The point is clamped to 0–100 % on each axis |
| Pick on canvas armed, then Cancel is pressed | Picking ends with the dialog; later canvas presses go to the active tool again |
| Pick armed, then the user presses a tool key (B) or Ctrl+Z | Nothing happens, as for any key while an effect is open |
| Reseed when the range has a single value | Not reachable: every seed range has at least two values |
| User unlinks, sets Width 5 and Height 9, then links again | Both take the value of the field edited last (9) |
| UI language switched while a menu is open | The menu closes or shows the new order the next time it opens |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Pixel mode drops the neighbourhood effects (blurs, sharpening, Edge Detect, Emboss, Relief, Reduce Noise) from its menus | The owner asked for only the effects that make sense in Pixel mode; mixing neighbours turns hard pixel edges into anti-aliased ones, while per-pixel colour operations leave edges hard | Medium: a pixel artist who used Gaussian Blur loses the menu path (the agent tools still have it) |
| Unsharp Mask sits in Photo next to Sharpen | paint.net has no Unsharp Mask; Photo ▸ Sharpen is its nearest home | Low |
| Point values are percent of the image area, not pixels | The wire already carries the centre as a fraction; percent needs no document size in the catalog | Low |
| The link toggle is on by default | paint.net's Square Blur is square (one radius); a linked pair gives that by default while still allowing a rectangle | Low |
| The dial follows the effects' convention (90° down) rather than paint.net's counter-clockwise one | AC-07 keeps the wire meaning unchanged; flipping it would change what existing angle values do | Low |
| Entries sort by translated name | paint.net sorts its menus by the localized names | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/effect-menus.spec.ts` › image mode: paint.net submenus with the Image-mode entries, sorted; `ui/e2e/effect-catalog.spec.ts` › submenus in paint.net order with English and German names | built |        |
| AC-02 | `ui/e2e/effect-menus.spec.ts` › image mode: paint.net submenus with the Image-mode entries, sorted | built |        |
| AC-03 | `ui/e2e/effect-menus.spec.ts` › pixel mode: only per-pixel effects, no neighbourhood effect | built |        |
| AC-04 | `ui/e2e/effect-menus.spec.ts` › switching the UI language re-sorts and renames the entries; › image mode: paint.net submenus … sorted | built |        |
| AC-05 | `ui/e2e/effect-menus.spec.ts` › every effect entry is disabled until the document is loaded; › image mode: paint.net submenus … ("…" on entries with parameters, none on Sharpen / Invert Colors) | built |        |
| AC-06 | `ui/e2e/effect-catalog.spec.ts` › submenus in paint.net order with English and German names; `ui/e2e/effect-menus.spec.ts` › switching the UI language re-sorts and renames the entries | built |        |
| AC-07 | `ui/e2e/effect-catalog.spec.ts` › every catalog entry has pinned vectors of the previous catalog; › `<id>`: wire vectors equal the previous catalog's (20 tests) | built |        |
| AC-08 | `pincel-wasm: effects::tests::apply_effect_knows_every_effect_name_on_a_pixel_document`; `pincel-agent: tests::draw::effect_apply_accepts_every_effect_on_a_pixel_document` | built |        |
| AC-09 | `ui/e2e/effect-params.spec.ts` › angle: the dial and the degree field set the same angle (AC-09); `ui/e2e/effect-catalog.spec.ts` › wrapAngle wraps typed angles outside 0–360 | built |        |
| AC-10 | `ui/e2e/effect-params.spec.ts` › point: X / Y in percent, default centre, the effect follows it (AC-10); `ui/e2e/effect-catalog.spec.ts` › radial_blur: wire vectors equal the previous catalog's | built |        |
| AC-11 | `ui/e2e/effect-params.spec.ts` › point: Pick on canvas sets the point without a tool stroke (AC-11); › point: a picked point updates the preview (AC-11); › touch device › pen and single touch pick the point; two fingers still pinch (AC-11) | built |        |
| AC-12 | `ui/e2e/effect-params.spec.ts` › seed: Reseed draws a new seed, the same seed gives the same pixels (AC-12); `pincel-effects: noise::add_noise::tests::add_noise_is_reproducible_for_a_fixed_seed` | built |        |
| AC-13 | `ui/e2e/effect-params.spec.ts` › linked pair: Square Blur width and height move together while linked (AC-13) | built |        |
| AC-14 | `ui/e2e/effect-params.spec.ts` › Reset returns angle, point, seed and link to their defaults (AC-14) | built |        |
| AC-15 | `ui/e2e/effect-params.spec.ts` › the new controls work from the keyboard (AC-15); › German UI › the new controls are named in German (AC-15) | built |        |

## Decisions

- 2026-11-04: The owner chose "effects + adjustments" from paint.net's official docs and "only the effects that make sense in Pixel mode"; the plan the owner approved splits the work into this catalog spec plus six effect specs. Written without a further dialogue while the owner's goal "all planned specs implemented" was active.
- 2026-11-04: The colour kind stays with `outline-effect` instead of moving here — because no effect in this spec uses colour, so a colour control here could not be tested through any menu entry.
- 2026-11-04: Clarification pass settled without a dialogue: neighbourhood vs. per-pixel rule for the Pixel filter; image area = document canvas; angle dial in the effects' own convention with wrap-around; Escape while picking ends only the picking; AC-07 pinned by a test of the previous vectors.
- 2026-11-04: AC-02, AC-03, AC-04 and AC-07 reworded so the follow-up effect specs can add entries without breaking them (behaviour of this spec unchanged).
- 2026-11-04 (spec-build): AC-07's pins were green at once for every entry except `radial_blur`, whose point is now edited in percent (red until `toParams` divided by 100) — the pins guard behaviour that already existed; that is their purpose.
- 2026-11-04 (spec-build): AC-08 needed no code — both tests were green at once, because the menu filter lives only in the UI catalog (`EffectDef.modes`) and the cores never saw it.
- 2026-11-04 (spec-build): A typed angle of exactly 360 stays 360 (inside 0–360, AC-09 wraps only values outside), the dial shows it at 0°; the wire meaning of 360 is unchanged.
- 2026-11-04 (spec-build): The dial is an SVG `role="slider"`: arrows step 1°, Page Up / Down 15°, Home / End 0° / 359°, wrapping around; the degree field wraps on commit (`change`), not per keystroke, so typing "-10" is not mangled mid-entry.
- 2026-11-04 (spec-build): Pick on canvas runs through a shared `canvasPick` channel (`ui/src/lib/effects/canvas-pick.svelte.ts`); Image mode's `MainCanvas` routes a primary press to it in the capture phase after the pinch and Space-pan checks, so tools never see it. Pixel mode has no point parameter yet and no routing (Technical notes).
- 2026-11-04 (spec-build): Space-pan in both editors no longer swallows a Space *tap* on a focused button or checkbox (`isSpaceActivated` in `shortcuts.ts`): a hold that pans cancels the activation on keyup. Without it the Pick on canvas, Reseed and link controls were not operable by Space (AC-15), while Space-drag must still pan with the Pick button focused (AC-11).
- 2026-11-04 (spec-build): The link toggle is a checkbox labelled "Link Width and Height" stored in the value map under `size_linked` (never a wire slot), so Reset restores it with the other defaults.
- 2026-11-04 (spec-build): The menu e2e checks containment, sort order and submenu order rather than exact lists in Image mode, and derives Pixel mode's exact list from the catalog's `modes`, so the follow-up effect specs can add entries without editing these tests.
