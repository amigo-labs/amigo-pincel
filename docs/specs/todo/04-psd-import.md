---
id: todo-psd-import
title: Open PSD files in Image mode
status: todo
size: L (two M sessions; stop and ask before adding a dependency)
mode: Image
depends_on: [todo-image-layer-groups (for groups; flat import works without it)]
created: 2026-10-28
---

# Open PSD files in Image mode

## Goal

Pincel opens `.psd` files in Image mode:
- **Raster layers** come in with name, opacity, blend mode, visibility and offset.
- **Groups** come in once `03-image-layer-groups.md` has landed.
- Anything Pincel can't represent (adjustment layers, layer effects, masks, smart objects) is **dropped with a notice**, never silently.
- Save goes to `.ora`. Pincel doesn't write PSD.

## Why

The gap is listed in §18.5. Pixelorama, Resprite and Pixaki import PSD; Photopea and Affinity are built around it. Users arriving from Photoshop-adjacent tools hand over PSDs. Pincel opens neither PSD nor Krita files today.

## Current state (read these first)

**Decode.** `crates/pincel-image-core/src/codec/mod.rs` `decode()` (≈L47) calls `image::load_from_memory`.
- The workspace `image = 0.25` (root `Cargo.toml` ≈L34) has the features png, jpeg, webp, bmp, gif and tiff.
- The `image` crate has **no PSD decoder**. There is no PSD code or dependency anywhere (`Cargo.lock` included).

**Open path.** `open_image` (`crates/pincel-image-wasm/src/lib.rs` ≈L135) builds a **single-layer** document. `open_project` (≈L145) reads ORA.

**UI routing.** `ui/src/lib/fs/index.ts`
- `IMAGE_EXTENSIONS` (≈L87)
- picker types (`IMAGE_TYPES` ≈L73, `PICK.document` ≈L104)
- `FileFormat` (≈L124)
- `sniffFormat` (≈L168; PSD magic is `38 42 50 53`, "8BPS")
- `formatFromName` (≈L194), `mimeFor` (≈L228)
- `detectMode` in `ui/src/lib/shell/session.svelte.ts` sends everything that isn't aseprite to Image mode.

**Image editor.** `openBytes` in `ui/src/modes/image/core/controller.ts` (≈L551) branches on ORA versus `openImage`. For a non-ORA file, the first Save asks for an `.ora` name.

**Lists of supported formats to update together.**
- The `start.openHint` i18n key (`en.ts` ≈L40, `de.ts` ≈L44).
- `src-tauri/tauri.conf.json` `fileAssociations` (≈L47–122).
- The PWA manifest in `ui/vite.config.ts` (≈L51) has no `file_handlers`. No change needed.

## Design

### Parser: stop and ask before adding a dependency (CLAUDE.md §9, §12)

There are two options. Present both to the user. The recommendation depends on what the evaluation finds.

**A. The `psd` crate** (chinedufn/psd).
- Before proposing it, check:
  - its license (must allow MIT OR Apache-2.0 distribution);
  - its last release and maintenance;
  - that it builds for `wasm32-unknown-unknown`;
  - its added wasm size (`pnpm wasm:build:release`, compare the `.wasm`);
  - which color modes and depths it handles;
  - whether it exposes groups, blend-mode keys, opacity, visibility and RLE.
- A new dependency needs a Decision Log entry and the user's approval.

**B. An own minimal reader** in `crates/pincel-image-core/src/codec/psd.rs`, with no dependency.
- Header: version 1 (PSD), color mode 3 (RGB), depth 8.
- Layer and mask info: layer records with name, opacity, blend key, visibility flag, bounds, and channels with raw or PackBits RLE.
- Group markers come from the `lsct` section-divider blocks.
- Reject anything else with a clear error: PSB, 16/32-bit, CMYK, Lab, indexed, ZIP compression.
- Roughly 400–600 lines with tests, which is more than one M session. Split it into header/composite and layers.

**Fallback.** Both options can fall back to the **flattened composite** (the "image data" section at the end of the file, raw or RLE) when the layer section is missing or unsupported.

### Mapping to the Image document

- **Blend keys.** These map to the 12 Image-mode modes:
  - `norm` → Normal, `mul ` → Multiply, `scrn` → Screen, `over` → Overlay
  - `dark` → Darken, `lite` → Lighten
  - `div ` → Color Dodge, `idiv` → Color Burn
  - `hLit` → Hard Light, `sLit` → Soft Light
  - `diff` → Difference, `smud` → Exclusion
  - Any other key → Normal, plus a notice.
- **Layers** are placed at their bounds with `offset_copy` into canvas-sized buffers, the same way ORA offsets work.
- **Groups.** With 03 done, `lsct` open/close markers build the tree. Without it, flatten the groups and **fold group opacity and visibility into the children where possible**. Never show a hidden group's children.
- **Dropped content** (adjustment layers, text layers, layer effects, masks, smart objects):
  - Text and smart-object layers still carry rasterized pixels, so keep those pixels.
  - Show **one status-bar notice** listing what was dropped.
  - Don't fail the open.
- **Limits.** Respect `MAX_LAYERS` (999) and the canvas size and area limits (`MAX_CANVAS_DIM`, `MAX_CANVAS_AREA` in `ui/src/modes/image/stores/editor.svelte.ts`). A canvas over the limit is an error, never a crash.

### wasm and UI

- `open_psd(data) -> handle`, or let `open_image` sniff `8BPS` and branch. Recommendation: a separate `open_psd` that returns the dropped-content list for the notice, for example `{ handle, dropped: string[] }`.
- `ui/src/lib/fs`:
  - `FileFormat` `'psd'`, sniff `8BPS`, extension `psd`, MIME `image/vnd.adobe.photoshop`;
  - add it to `IMAGE_TYPES` and the Tauri dialog filters.
- `controller.ts` `openBytes` branches to `openPsd`. Saving behaves like opening a PNG today: it asks for an `.ora` target.
- Also update `start.openHint` (en + de) and `tauri.conf.json` `fileAssociations` (one `psd` entry, `role: "Viewer"` or `"Editor"`; pick one and say why).

## Tasks

1. **Evaluation, then stop and ask.**
   - Evaluate option A against option B with the checks above.
   - Write a short comparison in the session's STATUS note and ask the user.
   - Then a `docs(spec)` commit: Decision Log entry, plus §7 File I/O and §18 updates.
2. **M: core reader and mapping.**
   - Test fixtures: small hand-made `.psd` files in `crates/pincel-image-core/tests/fixtures/` (flat, two layers with opacity and blend, hidden layer, group, RLE, 16-bit for rejection).
   - **Add `*.psd binary` to `.gitattributes` in the same commit** (CLAUDE.md §6.4).
   - Tests:
     - `psd_two_layers_keep_order_opacity_blend`
     - `psd_offset_layer_is_placed_at_bounds`
     - `psd_unknown_blend_key_falls_back_to_normal`
     - `psd_16_bit_is_rejected_with_error`
     - `psd_without_layers_opens_composite`
3. **M: wasm, routing, UI notice, Tauri association and i18n.**
   - e2e `ui/e2e/psd.spec.ts`: open a fixture through the start screen's Open… (file-chooser pattern in `ui/e2e/ora.spec.ts`), check the layer count (`getByRole('button', { name: /^Hide / })`) and the size text (`'W × H · sRGB'`).

## Out of scope

- PSD export, PSB (large documents), CMYK, Lab and 16/32-bit.
- Adjustment layers, layer effects and masks as live features.
- Krita `.kra`. It is a zip with ORA-like `maindoc.xml`, which could be a cheap follow-up.
