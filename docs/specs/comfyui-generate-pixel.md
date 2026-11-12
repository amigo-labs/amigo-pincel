---
status: todo
created: 2026-11-06
size: M
mode: Pixel
design: [pincel.md §3, pincel.md §15 (2026-11-06 ComfyUI)]
depends_on: [comfyui-generate]
---

# Generate images with ComfyUI (Pixel mode)

## Context

`comfyui-generate` brings the Generate panel and `gen.image` to Image mode. The owner wants both modes. Image models generate at about 512² to 1024², while a sprite is often 16² to 128² and may be indexed to a palette. In Pixel mode, Pincel therefore scales each variant to the target box and, for indexed sprites, maps its colors to the sprite's palette; RGBA sprites keep their colors (owner, 2026-11-06). The SDXL pixel-art LoRA from `comfyui-install` helps the model draw in a pixel-art style; any installed model works.

## Scope

### In scope

- The Generate panel and `gen.image` / `gen.models` in the Pixel editor: every criterion of `comfyui-generate` AC-02 – AC-21 applies, with "layer" meaning the active layer's cel in the current frame and the changes below
- Scaling each variant to the target box
- Mapping to the palette for indexed sprites and to gray for grayscale sprites
- The applied variant as a new layer with a cel in the current frame

### Out of scope

- Color reduction to N colors for RGBA sprites — because the owner chose "keep colors" for RGBA
- Dithering during palette mapping — because dithering patterns at sprite size read as noise rather than shading; can come as an option later
- Generating animation frames — because the owner asked for single images, and frames generated one by one do not stay consistent; backlog with control layers
- Changing the palette — because a sprite's palette is the artist's choice; the variant adapts to it

## Acceptance criteria

- [ ] AC-01 (always): In the desktop app, the Pixel editor shows the Generate panel; `comfyui-generate` AC-02 – AC-21 hold in Pixel mode, where the input is the active layer's cel in the current frame within the target box.
- [ ] AC-02 (event): When a generation finishes in Pixel mode, then the strip and the canvas preview show each variant at the target box's size: smaller targets averaged down, larger targets enlarged with hard pixel edges (the panel then notes "Larger than the model's output").
- [ ] AC-03 (state): While the sprite is indexed, every visible pixel of the preview and of the applied variant uses a palette entry other than the transparent one, the nearest by perceived color; pixels outside the mask, and pixels whose alpha fell below 50 % through averaging, are transparent; the palette is not changed.
- [ ] AC-04 (state): While the sprite is grayscale, the preview and the applied variant are gray (perceived luminance), alpha unchanged.
- [ ] AC-05 (state): While the sprite is RGBA, the variant keeps the colors and alpha it has after scaling.
- [ ] AC-06 (event): When the user clicks Apply, then a new layer is added above the active layer with a cel in the current frame holding the variant over the target box, limited to the mask, as one undo step.
- [ ] AC-07 (event): When the user clicks Generate at strength below 100 %, then the input is enlarged to the generation size with hard pixel edges before it is sent.
- [ ] AC-08 (event): When an agent calls `gen.image` on a Pixel document, then the variant is scaled and mapped as in AC-02 – AC-05 and applied as in AC-06 in the tab's current frame, and the result names the new layer, the seed and the model used.
- [ ] AC-09 (event): When the active layer is a tilemap or reference layer, then Apply adds a normal layer above it, and strength below 100 % uses the layer as the sprite shows it.
- [ ] AC-10 (error): If an indexed sprite's palette has no entry besides the transparent one, then Generate is disabled with "The palette has no visible color", and `gen.image` fails with `isError: true` and the same message.
- [ ] AC-11 (state, manual: needs a GPU and a real model): With SDXL and the pixel-art LoRA, "a small green slime monster" on a 32×32 indexed sprite yields variants that read as the subject in the sprite's palette.

## Technical notes

Scaling down averages the covered source pixels (area averaging), scaling up repeats pixels (nearest neighbour); perceived color distance and luminance are measured in OKLab. Tests: Rust unit tests for scaling and mapping on fixed images; Playwright against the mocked Tauri bridge as in `comfyui-generate`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Indexed sprite without a transparent index | Pixels outside the mask are not written (the new layer's cel stays empty there) |
| Palette with one visible color | Every visible pixel takes it (AC-03) |
| 1×1 target box | One pixel, the variant's average color mapped as AC-03 – AC-05 |
| Sprite 2048² | Enlarged from the generation size with hard edges; noted (AC-02) |
| Linked cels on the active layer | Only read; Apply adds a new layer with its own cel |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Pixels below 50 % alpha after averaging become transparent | An indexed pixel is either a palette color or transparent; the midpoint keeps a shape's outline where half of the covered source pixels were inside it | Thin features at the mask edge may vanish |
| Downscaling averages, upscaling repeats pixels | Averaging keeps shapes readable at small sizes; repeating keeps hard edges where the model's output is too small | Soft edges before palette mapping in RGBA sprites |
| Input enlarged with hard edges (AC-07) | The model then sees pixel art as blocks, which the pixel-art LoRA expects | none |
| Nearest color is measured perceptually (OKLab) | Plain RGB distance picks visibly wrong palette entries for greens and blues | none |
| Grayscale sprites get gray results | A grayscale sprite can only hold gray values | none |
| Tilemap and reference layers get a normal layer above | A tilemap cel holds tile indices, not pixels; a reference layer is not exported | none |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | e2e `the desktop Pixel editor has a collapsed Generate panel beside the Assistant (AC-01)`, `variants show at the target box size on the strip and the canvas; the document stays (AC-01, AC-02)`, `closing a Pixel tab cancels its generation (AC-01)` (ui/e2e/comfyui-generate-pixel.spec.ts); the panel, its store and its jobs are the ones of Image mode (`ui/src/lib/comfyui/`), whose `comfyui-generate*.spec.ts` run unchanged against them | built |  |
| AC-02 | e2e `variants show at the target box size on the strip and the canvas; the document stays (AC-01, AC-02)`, `a target larger than the generation is enlarged with hard edges and noted (AC-02)` (ui/e2e/comfyui-generate-pixel.spec.ts); Node `Pixel mode notes a target box larger than the generation (comfyui-generate-pixel AC-02)` (ui/e2e/comfyui-generate-params.spec.ts); `fit_shrinking_averages_the_covered_pixels`, `fit_shrinking_by_a_fraction_weighs_partial_coverage`, `fit_shrinking_weighs_color_by_alpha`, `fit_growing_repeats_pixels_with_hard_edges`, `fit_crops_the_overlap_evenly` (crates/pincel-core/src/generate.rs); `generation_preview_is_the_box_size_in_palette_colors` (crates/pincel-wasm/src/generate.rs) | built |  |
| AC-03 | `to_sprite_pixels_indexed_takes_the_nearest_color_by_oklab`, `to_sprite_pixels_indexed_below_half_alpha_is_transparent`, `to_sprite_pixels_indexed_with_one_visible_color_uses_it`, `prepare_variant_outside_the_mask_is_transparent`, `add_generated_layer_fills_an_indexed_cel_with_the_transparent_index` (crates/pincel-core/src/generate.rs); `generation_preview_is_the_box_size_in_palette_colors`, `add_generated_layer_is_one_undo_step_with_an_indexed_cel` (crates/pincel-wasm/src/generate.rs) | built |  |
| AC-04 | `to_sprite_pixels_grayscale_is_perceived_luminance_with_alpha`, `shown_rgba_takes_palette_and_gray_values` (crates/pincel-core/src/generate.rs); `grayscale_previews_gray_with_its_alpha` (crates/pincel-wasm/src/generate.rs) | built |  |
| AC-05 | `to_sprite_pixels_rgba_keeps_colors_and_alpha` (crates/pincel-core/src/generate.rs); e2e `variants show at the target box size on the strip and the canvas; the document stays (AC-01, AC-02)` (ui/e2e/comfyui-generate-pixel.spec.ts) | built |  |
| AC-06 | e2e `Apply adds a layer above the active one with the variant in the box, as one undo step (AC-06)` (ui/e2e/comfyui-generate-pixel.spec.ts); `add_generated_layer_adds_one_layer_above_the_anchor_as_one_undo_step` (crates/pincel-core/src/generate.rs); `add_generated_layer_is_one_undo_step_with_an_indexed_cel` (crates/pincel-wasm/src/generate.rs) | built |  |
| AC-07 | e2e `strength below 100 % sends the active layer enlarged with hard edges (AC-07)` (ui/e2e/comfyui-generate-pixel.spec.ts); `generation_input_enlarges_the_active_layer_with_hard_edges` (crates/pincel-wasm/src/generate.rs); `generation_input_reads_the_layer_unmasked_and_none_when_transparent` (crates/pincel-core/src/generate.rs) | built |  |
| AC-08 | e2e `gen.image on a Pixel document applies one variant as a new layer (comfyui-generate-pixel AC-08)` (ui/e2e/comfyui-generate-tools.spec.ts) | built |  |
| AC-09 | `add_generated_layer_above_a_tilemap_is_a_normal_layer_above_it`, `generation_input_reads_the_layer_unmasked_and_none_when_transparent` (a linked cel is read) (crates/pincel-core/src/generate.rs); `a_tilemap_layer_is_read_as_shown_and_gets_a_normal_layer_above` (crates/pincel-wasm/src/generate.rs) | built |  |
| AC-10 | `to_sprite_pixels_indexed_without_a_visible_color_fails` (crates/pincel-core/src/generate.rs); `a_palette_without_a_visible_color_refuses_generation` (crates/pincel-wasm/src/generate.rs); Node `a palette without a visible color reads the same in the panel and in gen.image (comfyui-generate-pixel AC-10)` (ui/e2e/comfyui-generate-params.spec.ts) | built |  |
| AC-11 |      | open   |        |

## Decisions

- 2026-11-06: Pixel mode is part of the plan from the start, as its own spec (owner: "everything in both modes").
- 2026-11-06: Scale to the target; indexed sprites map to their palette, RGBA keeps colors (owner).
- 2026-11-12 (spec-build): Indexed and grayscale sprites cannot be opened or created in the editor yet (the `.aseprite` reader refuses them, indexed painting is Phase 2), so AC-03, AC-04 and AC-10 are proven in the core and the wasm binding on sprites whose color mode the test sets, and AC-10's message is pinned Node-side as the same text in the panel and in `gen.image`; the panel already disables Generate through `Document.canHoldGenerated`. Apply writes the new layer's cel in the sprite's color mode (palette indices, value + alpha): it only fills a new cel, so it needs none of the `SetPixel` payload decision that keeps indexed painting in Phase 2 (Decision Log 2026-11-06 already maps results to the palette).
- 2026-11-12 (spec-build): Scaling, mapping, the input and the new layer live in `pincel-core::generate`, called through `pincel-wasm`. The generation size repeats `pincel-image-core`'s rule (both cores stay separate, Decision Log "Two cores, one shell"). The input below 100 % and the mask are fitted to the generation size with the same rule as the variants (nearest neighbour when growing, area average when shrinking), so a sprite larger than the generation size is averaged down for the model.
- 2026-11-12 (spec-build): Averaging weights each source pixel's color by its alpha and its covered area; an indexed pixel is visible from alpha 128 (50 % rounded up). Perceived distance is squared OKLab distance in `f64`, ties to the lower index; grayscale takes the gray of the same OKLab lightness.
- 2026-11-12 (spec-build): The new layer gets a canvas-sized cel like Paste's, filled with the transparent index in indexed sprites, so pixels outside the mask are "not written" (edge case "Indexed sprite without a transparent index"); it becomes the active layer, as after Paste. Its undo step is the history kind `generate` ("Generate Image"), shared with Image mode.
- 2026-11-12 (spec-build): The Generate panel, its settings, jobs and per-tab variants moved from Image mode to `ui/src/lib/comfyui/` (`GeneratePanel.svelte`, `generation.svelte.ts`, `job-routing.ts`) and serve both editors; each mode hands the panel a `GenTarget` for the active tab (`ui/src/modes/image/generate/target.ts`, `ui/src/modes/pixel/generate-target.ts`), and a tab's generation is keyed `<mode>:<tab id>`, since Pixel and Image tab ids overlap. One shared store keeps one event listener, one set of settings and one server state for both editors. `EditorDocument.generateImage` is no longer optional.
- 2026-11-12 (spec-build): In Pixel mode the strip and the canvas preview show each variant as Apply would write it (scaled, masked, in the sprite's colors), encoded as a box-sized PNG by the core; the canvas preview is an `<img>` over the target box with hard edges. The note "Larger than the model's output" shows with the finished variants when the target box is wider or taller than the generation size.
- 2026-11-12 (spec-build): New public API: `pincel-core::generate` (`generation_size`, `fit`, `apply_mask`, `generation_mask`, `generation_input`, `has_visible_color`, `to_sprite_pixels`, `prepare_variant`, `shown_rgba`, `add_generated_layer`, `GenerateError`), `HistoryKind::Generate`; `pincel-wasm`: `generationSize`, `generationMaskPng` and on `Document` `generationMask`, `generationInput`, `canHoldGenerated`, `generationPreview`, `addGeneratedLayer` (the target box as `[x, y, w, h]`, so no binding needs more than seven arguments).
- 2026-11-12 (spec-build): Full gates: every gate green except the e2e gate's two `tool-flyout-harness.spec.ts` tests, which failed on Vite's `EPERM` rename (AGENTS.md Gotchas) and passed when rerun alone; `cargo clippy -p pincel-tauri --all-targets -- -D warnings` and `cargo test -p pincel-tauri` green (no `src-tauri` change).
- 2026-11-12 (spec-build): Red runs. Core: all 20 `generate::` tests failed with the functions stubbed. The wasm binding tests were green at once on top of the red-then-green core. E2e: `comfyui-generate-pixel.spec.ts` failed 6 of 6 and the Pixel `gen.image` test failed against the build before the UI block (no Generate panel in the Pixel editor; `gen.image` refused Pixel documents); the two Node-side tests were written after their code and green at once. Image mode's `comfyui-generate*.spec.ts` stayed green on the shared panel. The tilemap binding test (AC-09) was added after the code and green at once.
