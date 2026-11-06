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

- 2026-11-06: Pixel mode is part of the plan from the start, as its own spec (owner: "everything in both modes").
- 2026-11-06: Scale to the target; indexed sprites map to their palette, RGBA keeps colors (owner).
