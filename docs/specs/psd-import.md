---
status: todo
created: 2026-10-28
size: L
mode: Image
design: [pincel.md §7, pincel.md §18.5, pincel.md §15 (2026-10-11 OpenRaster project format)]
depends_on: [image-layer-groups]
---

# Open PSD files in Image mode

## Context

Pincel opens neither PSD nor Krita files today. The gap is listed in §18.5: Pixelorama, Resprite and Pixaki import PSD, and Photopea and Affinity are built around it; users arriving from Photoshop-adjacent tools hand over PSDs. This spec lets Image mode open `.psd` files with their raster layers, groups and layer properties. Anything Pincel cannot represent is dropped **with a notice, never silently**. Pincel does not write PSD: saving goes to `.ora`.

The `image` crate has no PSD decoder and there is no PSD code or dependency in the workspace, so the parser choice (a new dependency or an own reader) is still open — see Open questions.

## Scope

### In scope

- Opening 8-bit RGB `.psd` files in Image mode through every existing open route (start screen, File ▸ Open, recents, desktop file association)
- Raster layers with name, opacity, blend mode, visibility and offset
- Groups from PSD section dividers, with their name, opacity, visibility and blend mode
- Dropping unrepresentable content with one notice; text and smart-object layers keep their rasterized pixels
- Falling back to the flattened composite when the file has no layers
- Format detection by content and extension, picker filters, the start-screen hint (en + de), the desktop file association
- Save of an opened PSD goes to an `.ora` target

### Out of scope

- Writing PSD — because `.ora` is the Image-mode project format (Decision Log 2026-10-11) and a PSD writer is a separate, large codec
- PSB (large documents), CMYK, Lab and 16 / 32-bit depth — because they need separate decode paths for rare inputs; they are rejected with an error instead
- Adjustment layers, layer effects and masks as live features — because Image mode has no such layer kinds; their content is dropped with a notice
- Krita `.kra` — because it is a different container (a zip with an ORA-like `maindoc.xml`); a possible cheap follow-up

## Acceptance criteria

- [ ] AC-01 (event): When the user opens an 8-bit RGB `.psd`, then it opens in Image mode with the PSD's canvas size and one layer per PSD raster layer, in the PSD's stacking order, each with the PSD layer's name, opacity, visibility and blend mode.
- [ ] AC-02 (event): When a PSD layer's bounds are smaller than or offset from the canvas, then its pixels appear at those bounds and the rest of the layer is transparent.
- [ ] AC-03 (always): The PSD blend keys `norm`, `mul `, `scrn`, `over`, `dark`, `lite`, `div `, `idiv`, `hLit`, `sLit`, `diff`, `smud` open as Normal, Multiply, Screen, Overlay, Darken, Lighten, Color Dodge, Color Burn, Hard Light, Soft Light, Difference and Exclusion respectively.
- [ ] AC-04 (event): When a PSD layer uses any other blend key, then that layer opens with Normal and the open notice names the substitution.
- [ ] AC-05 (event): When a PSD contains layer groups (section-divider open / close markers), then they open as Image-mode groups with the PSD group's name, opacity and visibility, containing the same layers; a hidden PSD group's children never show in the composite.
- [ ] AC-06 (event): When a PSD contains content Pincel cannot represent (adjustment layers, layer effects, masks, text layers, smart objects), then the file still opens, text and smart-object layers keep their rasterized pixels as raster layers, the rest is left out, and one status-bar notice lists what was dropped.
- [ ] AC-07 (event): When a PSD has no layer section, then it opens as a single-layer document holding the file's flattened composite.
- [ ] AC-08 (error): If a PSD is PSB, uses a depth other than 8 bits per channel, or a color mode other than RGB, then the open fails with an error message that names the unsupported property, and the previously open document or screen stays unchanged.
- [ ] AC-09 (error): If the PSD's canvas exceeds Image mode's canvas size or area limit, then the open fails with an error message and no crash, and the previously open document or screen stays unchanged.
- [ ] AC-10 (error): If the file starts with the PSD signature but is truncated or internally inconsistent, then the open fails with an error message and no crash, and the previously open document or screen stays unchanged.
- [ ] AC-11 (always): A file is recognized as PSD by its content signature `8BPS` and by the extension `.psd`; the open pickers (web and desktop) offer `.psd`, and the start-screen hint lists PSD in English and German.
- [ ] AC-12 (event): When the user saves a document opened from a PSD, then Save asks for an `.ora` target instead of writing the PSD.
- [ ] AC-13 (event, manual: OS file association needs an installed desktop build): When the user opens a `.psd` from the operating system's file manager with the desktop app installed, then it opens in Pincel's Image mode.

## Technical notes

Hints from the legacy plan (2026-10-28); paths verified 2026-10-29, line numbers drift.

**Decode.** `crates/pincel-image-core/src/codec/mod.rs` `decode()` (≈L47) calls `image::load_from_memory`. Workspace `image = 0.25` (root `Cargo.toml` ≈L34) with features png, jpeg, webp, bmp, gif, tiff — no PSD decoder; no PSD code or dependency anywhere (`Cargo.lock` included).

**Parser options** (see Open questions):

- **A. The `psd` crate** (chinedufn/psd). Evaluate: license (must allow MIT OR Apache-2.0 distribution), last release and maintenance, builds for `wasm32-unknown-unknown`, added wasm size (`pnpm wasm:build:release`, compare the `.wasm`), color modes and depths it handles, whether it exposes groups, blend-mode keys, opacity, visibility and RLE.
- **B. An own minimal reader** in `crates/pincel-image-core/src/codec/psd.rs`, no dependency: header version 1, color mode 3 (RGB), depth 8; layer and mask info (layer records with name, opacity, blend key, visibility flag, bounds; channels raw or PackBits RLE); group markers from `lsct` section-divider blocks; reject everything else (PSB, 16/32-bit, CMYK, Lab, indexed, ZIP compression). Roughly 400–600 lines with tests — split header/composite and layers.
- Both can fall back to the flattened composite (the image-data section at the end of the file, raw or RLE).

**Mapping.** Layers are placed at their bounds with `offset_copy` into canvas-sized buffers, as ORA offsets are. Respect `MAX_LAYERS` (999) and `MAX_CANVAS_DIM` / `MAX_CANVAS_AREA` (`ui/src/modes/image/stores/editor.svelte.ts` ≈L62–66).

**Open path.** `open_image` (`crates/pincel-image-wasm/src/lib.rs` ≈L135) builds a single-layer document; `open_project` (≈L145) reads ORA. `openBytes` in `ui/src/modes/image/core/controller.ts` (≈L551) branches on ORA versus `openImage` and decodes before closing the current document. Planned: a separate `open_psd(data) -> { handle, dropped: string[] }` and an `openPsd` branch in `openBytes`.

**UI routing.** `ui/src/lib/fs/index.ts`: `IMAGE_TYPES` (≈L73), `IMAGE_EXTENSIONS` (≈L87), `FileFormat` (≈L124), `sniffFormat` (≈L168), `formatFromName` (≈L194), `mimeFor` (≈L228) — add `'psd'`, magic `38 42 50 53`, MIME `image/vnd.adobe.photoshop`, Tauri dialog filters. `detectMode` in `ui/src/lib/shell/session.svelte.ts` sends everything non-aseprite to Image mode.

**Format lists to update together.** `start.openHint` (`ui/src/lib/i18n/en.ts` ≈L40, `de.ts` ≈L44); `src-tauri/tauri.conf.json` `fileAssociations` (≈L47). The PWA manifest in `ui/vite.config.ts` has no `file_handlers` — no change.

**Fixtures.** Small hand-made `.psd` files in `crates/pincel-image-core/tests/fixtures/` (directory does not exist yet): flat, two layers with opacity and blend, hidden layer, group, RLE, 16-bit for rejection. Add `*.psd binary` to `.gitattributes` in the same commit as the first fixture (CLAUDE.md §6.4). Suggested tests: `psd_two_layers_keep_order_opacity_blend`, `psd_offset_layer_is_placed_at_bounds`, `psd_unknown_blend_key_falls_back_to_normal`, `psd_16_bit_is_rejected_with_error`, `psd_without_layers_opens_composite`.

**e2e.** `ui/e2e/psd.spec.ts`: open a fixture through the start screen's Open… (file-chooser pattern in `ui/e2e/ora.spec.ts`), check the layer count (`getByRole('button', { name: /^Hide / })`) and the size text (`'W × H · sRGB'`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| PSD layer with zero-size bounds (empty layer) | Opens as a fully transparent layer with its name and properties |
| Layer bounds partly outside the canvas | The part inside the canvas is kept, the rest is clipped |
| Hidden PSD layer | Opens hidden, pixels intact |
| File named `.psd` whose content is PNG (or another supported format) | Opens by its content, like any misnamed file today |
| PSD with only unrepresentable layers (e.g. adjustment layers only) | Opens, and the notice lists what was dropped; what the document then holds is part of the composite-fallback open question |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Only 8-bit RGB is supported, whichever parser is chosen; grayscale, indexed, bitmap, duotone and multichannel are rejected like CMYK | Option B rejects every mode but RGB, and the out-of-scope list names the other modes | Low — a crate might handle grayscale cheaply, which would be a scope extension |

## Open questions

- Parser: the `psd` crate (new dependency — needs the evaluation in Technical notes, a Decision Log entry and the user's approval, CLAUDE.md §9 / §12) or an own minimal reader in `pincel-image-core`? The plan says to stop and ask after the evaluation.
- The Decision Log entry for PSD import and the §7 File I/O / §18 updates in `pincel.md` are not written yet.
- Ship before `image-layer-groups` lands (flatten groups, fold group opacity and visibility into the children, never show a hidden group's children) or wait for it? The plan allows flat import without groups; this spec assumes groups are available.
- PSD groups default to the pass-through blend key `pass`, which Image mode does not support (isolated groups only). Map `pass` silently to an isolated Normal group, or list it in the open notice? Groups with non-Normal children can render differently than in Photoshop.
- Which problems in the layer section fall back to the flattened composite (with a notice) and which reject the file? The plan rejects ZIP-compressed channels and also falls back "when the layer section is missing or unsupported"; this includes what a PSD with only unrepresentable layers opens as (the composite, or one empty layer).
- A PSD with more than 999 layers (groups included): fail the open with an error, or open the first 999 and list the rest in the notice? The plan only says to respect `MAX_LAYERS`.
- Desktop file association role for `.psd`: `Viewer` or `Editor`? The plan asks to pick one and say why.

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

## Decisions

- Pincel does not write PSD; an opened PSD saves to `.ora` like an opened PNG does today — because `.ora` is the Image-mode project format and keeps layers.
- Unrepresentable content is dropped with one status-bar notice, never silently and never by failing the open — because a partial import is more useful than none, and silent loss is the bug class the groups / ORA specs fix.
- The wasm layer gets a separate `open_psd` that returns the dropped-content list along with the handle, rather than sniffing `8BPS` inside `open_image` — because the notice needs that list (plan recommendation).
