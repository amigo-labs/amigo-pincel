---
status: todo
created: 2026-11-15
size: L
mode: Image
design: [pincel.md §15 (2026-11-06 ComfyUI), pincel.md §15 (2026-11-15 AI upscaling)]
depends_on: [ai-upscale, comfyui-generate]
---

# AI upscale: refine details in tiles (Image mode)

## Context

An upscale model (`ai-upscale`) enlarges and sharpens, but it invents little: a 512 px portrait at 4× has clean edges and no pores. The comparison the owner pointed to (2026-11-15) got its detail from a second, generative pass. That pass is image to image at a low strength (`comfyui-generate`): the model repaints the enlarged image a little and adds fine texture, while shapes and faces stay.

Diffusion models work at about one megapixel, and an upscaled image has up to 67. So the refine runs in overlapping tiles that are blended back together. ComfyUI users do this with custom nodes; Pincel's workflows use core nodes only (Decision Log 2026-11-06), so Pincel cuts and blends the tiles itself and sends each tile as an ordinary image-to-image job (Decision Log 2026-11-15).

This spec adds "Refine details" to the AI Upscale dialog and to `gen.upscale`.

## Terms

Terms of `ai-upscale` (composite, factor, target size, size limits, unchanged, nothing stored) and `comfyui-generate` (model family) apply. "Checkpoint" is the diffusion model of the refine; "upscale model" stays the model of the upscale pass.

| Term | Meaning |
| ---- | ------- |
| Upscaled image | The server's upscale result at the target size, colour only (`ai-upscale` AC-11), before alpha is applied |
| Tile edge T | The model family's native edge: 512 px for SD 1.5, 1024 px for SDXL and Flux |
| Overlap V | T / 8: 64 px or 128 px |
| Tile plan | Per axis of length L: one tile covering the axis if L ≤ T; else n = ⌈(L − V) / (T − V)⌉ tiles of length T at the offsets round(i × (L − T) / (n − 1)) for i = 0 … n − 1, halves rounded up. The plan is every combination of a horizontal and a vertical tile, row by row from the top left |
| Tile weight | Per axis, a tile has two edges. Toward an edge on the image border the edge weight is 1; toward any other edge it is min(d + 1, V) / V, d being the pixel's distance from that edge (0 for the tile's outermost pixel). The axis weight is the smaller of the two edge weights, and a pixel's weight in a tile is the product of its horizontal and vertical axis weight |
| Padded tile | A tile whose edges are not multiples of 64 px is enlarged to the next multiple of 64 on the right and bottom by repeating its last column and row; the result is cropped back to the tile's size. Full tiles (T × T) need no padding |
| Strength | How far the refine may change a tile, 5 – 60 % |

## Scope

### In scope

- "Refine details" in the AI Upscale dialog: checkpoint, prompt, strength, seed, tile count, progress per tile, Cancel
- The tiled refine: tile plan, one image-to-image job per tile, blending, the result as the upscale's new layer
- `gen.upscale` parameters for the refine, and a time limit that fits many tiles
- Strings in all six UI locales

### Out of scope

- A face or skin fix — because the owner decided (2026-11-15) that selecting the face and using Generate at a low strength covers it
- Refining without upscaling (factor 1), and a tiled image to image for large documents in the Generate panel — because this dialog is about upscaling; the Generate panel's large-document mode is its own feature
- Refining only a selection — because the dialog upscales the whole document (`ai-upscale`, out of scope)
- LoRA and negative prompt for the refine — because at a low strength the image itself steers the result, and the dialog stays small
- Control layers (ControlNet tile) — because they need models outside the catalog; backlog "control layers"
- A prompt per tile (automatic captions) — because it needs a vision model Pincel does not have
- Strengths above 60 % — because tiles then invent content that no longer matches across their borders
- Sending tiles in parallel — because a ComfyUI server works through one queue

## Acceptance criteria

### Dialog

- [ ] AC-01 (always): The AI Upscale dialog has a "Refine details" checkbox, off while nothing is stored. While it is on, the dialog shows the server's checkpoints with their model family ("Model type" for checkpoints outside the catalog, as `comfyui-generate` AC-03; default: the model stored by the Generate panel if the server has it, else the server's first checkpoint), a prompt field that may stay empty, a strength slider (5 – 60 %, default 30 %), a seed field as the Generate panel's (empty = random; anything but a whole number ≥ 0 marks the field invalid and disables Upscale) and the number of tiles of the tile plan for the chosen factor and checkpoint ("66 tiles"). The checkbox, the checkpoint and the strength persist across restarts; prompt and seed do not. A stored checkpoint the server no longer has is replaced by that default, with the notice "<name> is no longer installed".
- [ ] AC-02 (state): While "Refine details" is on and the server has no checkpoint, the dialog shows "No model installed" with the "Set up…" button and Upscale is disabled; switching the checkbox off enables Upscale again.
- [ ] AC-03 (always): The tile plan gives 6 tiles (3 × 2) for a 2048 × 1152 target with SDXL, 66 tiles (11 × 6) for 9216 × 5184 with SDXL, 15 tiles (5 × 3) for 2048 × 1152 with SD 1.5, and 1 tile for 400 × 300 with either; every tile lies inside the image, the tiles cover every pixel, and neighbouring tiles overlap by at least V.

### Refining

- [ ] AC-04 (event): When the user clicks Upscale with "Refine details" on, then the upscale runs as in `ai-upscale` AC-08, and after it every tile of the tile plan is cut from the upscaled image and sent, one after the other, as an image-to-image generation with the chosen checkpoint: the padded tile as the starting image at its own size (no fitting, no scaling), one variant, the sampler preset of the model family (`comfyui-generate`), the strength as the starting-image strength, the dialog's prompt, no negative prompt and no LoRA. Every tile uses the same seed: the seed field's value, or one random seed.
- [ ] AC-05 (always): In the refined image, a pixel covered by one tile equals that tile's result (cropped back from its padding), and a pixel covered by several tiles is their mean weighted by the tile weights, each channel within ±1 of the exact value (8-bit rounding).
- [ ] AC-06 (state): While the refine runs, the dialog shows "Refining tile 12 / 66" with the queue position or the step progress of that tile in the form of `comfyui-generate` AC-08 ("Step 3 / 30"), and the Cancel button; `ai-upscale` AC-09 holds for the whole run.
- [ ] AC-07 (event): When the last tile is blended, then the result is applied as in `ai-upscale` AC-10 and AC-12 with the refined image in place of the upscaled image: one undo step, one new layer "Upscaled " plus the factor, alpha from the composite.
- [ ] AC-08 (event): When the user clicks Cancel during the refine — at any moment before the result is applied — or the app quits, then the running tile is stopped, no further tile is sent, other jobs on the server are not affected, and the document is unchanged as in `ai-upscale` AC-13.
- [ ] AC-09 (error): If a tile's generation fails as in `comfyui-generate` AC-15 (server error, no image, server unreachable) or returns an image that cannot be decoded or has another size than the padded tile sent, then no further tile is sent, the dialog shows "Refining failed at tile 12 of 66" with the error, its settings are enabled again, the document is unchanged, and Upscale can be clicked again; an out-of-GPU-memory error reads "Not enough GPU memory for this checkpoint".

### Agent tool

- [ ] AC-10 (event): When an agent calls `gen.upscale` with `refine: true` and optionally `checkpoint` (default as in the dialog, AC-01: the dialog's stored checkpoint, else the model stored by the Generate panel, else the server's first checkpoint — each only if the server has it), `prompt` (default empty), `strength` (0.05 – 0.6, default 0.3) and `seed` (as `gen.image`'s, default random), then the upscale is refined as in AC-04, AC-05 and AC-07, and the result additionally names the checkpoint, the seed and the number of tiles. The checkpoint's model family is found as for `gen.image`: from the catalog, else the stored "Model type", else SDXL. Without `refine` or with `refine: false`, the call behaves as in `ai-upscale` AC-18.
- [ ] AC-11 (error): If `gen.upscale` with `refine: true` names a checkpoint the server does not have, a strength or seed out of range, or the server has no checkpoint, then the call fails with `isError: true` and a message listing the valid names or the allowed range, before anything is sent to the server; `checkpoint`, `prompt`, `strength` or `seed` without `refine: true` fail the same way, saying they need `refine`. If a tile fails as in AC-09 during an agent's call, no further tile is sent, the document is unchanged, and the call fails with `isError: true` naming the tile and the error in English.
- [ ] AC-12 (always): A `gen.upscale` call with `refine: true` times out when the `gen.*` limit of `mcp-live-desktop` AC-19 passes without a tile finishing: the limit starts again when the upscale pass finishes and with every finished tile, and time a tile waits in the server's queue counts. On a timeout, on the tab closing and on the conditions of `ai-upscale` AC-21 at the moment the result is applied, that AC applies, and no further tile is sent.

### Language and hardware

- [ ] AC-13 (always): The new dialog controls, progress texts and errors are translated in all six UI locales (en, de, es, ja, fr, pt-BR); tool results and tool errors are English.
- [ ] AC-14 (state, manual: needs a GPU and real models; a visual judgement): With SDXL and Real-ESRGAN x4plus installed, upscaling a 512 × 288 portrait at 4× with "Refine details" at 30 % and an empty prompt gives a top layer in which a person, at 100 % zoom, finds no straight edge or brightness step along the tile borders of the plan (x = 512, 1024 and 1536; y = 128 and 1024), recognises the same face as in the original, and sees skin and hair texture that the same upscale without refine does not show.

## Technical notes

- **Where it runs.** The backend cuts, sends and blends the tiles natively, so no tile crosses the IPC boundary (Gotchas: Tauri IPC is slow for bulk bytes); the webview receives the final image once, as in `ai-upscale`. The tile plan, the tile weights and the blend are pure functions of `pincel-image-core`, which `pincel-tauri` links for this (Decision Log 2026-11-15); the dialog reads the tile count through `pincel-image-wasm`.
- **Tile jobs.** The existing image-to-image template and job code of `comfyui-generate` with one sampler branch, without its fitting: the tile is encoded at its padded size. Fitting (cover and crop to a generation size) would shift and scale a tile against its neighbours and the alpha; padding to multiples of 64 keeps every pixel in place and satisfies every model family's latent grid.
- **Order.** Tiles are sent one after the other, each queued when the previous one has finished, so Cancel never leaves queued tiles behind.
- **Memory.** The blend accumulates weighted sums row band by row band or in place; it must not hold all tiles at once (66 tiles of 1024² are 277 MB as RGBA).
- **New public API** (for review): tile plan, tile weight and blend functions in `pincel-image-core`; a tile-count function in `pincel-image-wasm`; the refine parameters of `gen.upscale`.
- **Tests.** Unit tests for plan, weights and blend in `pincel-image-core` (AC-03, AC-05); Rust tests against the mock ComfyUI server for the job sequence, seed, cancel, errors and the time limit; Playwright against the mocked Tauri bridge for the dialog. AC-14 is manual.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Target 2048 × 600 with SDXL | One row of 3 tiles of 1024 × 600, each sent padded to 1024 × 640 and cropped back (AC-04) |
| Target smaller than T on both axes (400 × 300) | One tile covering the image, sent padded (448 × 320) below the model's native size; no blending (AC-03, AC-05) |
| Target 8192 × 8192 with SD 1.5 | 361 tiles; the count is shown before the start, there is no upper limit (Assumptions) |
| Flux.1 schnell as checkpoint | Its preset (4 steps, no negative prompt) applies per tile, as in `comfyui-generate` |
| Empty prompt | Sent as an empty prompt; the image steers the result at a low strength |
| Server stops at tile 30 of 66 | "Refining failed at tile 30 of 66" (AC-09); nothing is applied, the finished tiles are dropped |
| Another program queues jobs between Pincel's tiles | Each tile waits its turn; the dialog shows the queue position for that tile (AC-06). An agent's call times out if one tile waits and runs longer than the limit (AC-12) |
| The checkpoint is removed from the server between two tiles | The next tile fails with the server's error (AC-09) |
| Cancel after the last tile, while the tiles are blended | Still cancels; nothing is applied (AC-08) |
| An agent passes `strength` without `refine: true` | The call fails, saying the parameter needs `refine` (AC-11) |
| Agent refine of 66 tiles takes 6 minutes | No timeout while tiles keep finishing (AC-12) |
| Same seed, same input, model, prompt and strength | The same requests are sent; the result is as reproducible as the server's sampler is |
| Composite with transparent areas | Every tile is sent, also a fully transparent one, refined over the black-filled colour (`ai-upscale`, Technical notes); alpha comes from the composite (AC-07) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Default strength 30 %, range 5 – 60 % | Common values for tiled refines: below 20 % little changes, above 50 % faces drift and tiles disagree | The best value depends on model and image; the slider persists |
| The same seed and prompt for every tile | Keeps the texture character even across tiles; per-tile prompts need a vision model | A prompt naming one object ("a face") can make tiles without it invent one at high strengths; the default prompt is empty |
| Overlap T / 8 with a linear blend | Enough to hide seams at low strengths, and it keeps the tile count low | Visible seams at strengths near 60 %; AC-14 checks 30 % |
| The whole refine is dropped when one tile fails | A half-refined image has a visible border between refined and unrefined areas | A late failure costs minutes; Upscale can be started again |
| Prompt and seed are not stored | They belong to one image; a stored prompt would silently steer the next, different image | The user retypes a prompt for a second try on the same image |
| `gen.upscale`'s `strength` defaults to 0.3, not to the dialog's stored value | A number can be the same default on every machine, so a call without it means the same everywhere; model names cannot, because every server has other models | An agent does not pick up the user's preferred strength unless told |
| No upper limit on the number of tiles | The count is shown before the start and depends on the user's GPU speed, which Pincel cannot know | A 361-tile refine on a slow GPU takes very long; Cancel ends it |
| Tiles below the model's native size are sent as they are | Scaling them up would change their pixel grid against the alpha; small targets are rare for an upscale | Lower quality on targets far below one megapixel |
| The dialog stays modal for the whole refine | The step resizes the document; edits meanwhile would be covered | A 66-tile refine blocks the editor for minutes; the tile count is shown before the start and Cancel is always available |

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
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |

## Decisions

- 2026-11-15: Tiles are cut and blended by Pincel and sent as ordinary image-to-image jobs — built-in workflows use ComfyUI core nodes only (Decision Log 2026-11-06), and tiled-upscale nodes are custom nodes.
- 2026-11-15: No face or skin fix (owner); see `ai-upscale`.
- 2026-11-15: `gen.upscale` with refine measures its time limit per tile (AC-12) — a refine of many tiles runs longer than the fixed `gen.*` limit of `mcp-live-desktop` AC-19, which stays as it is for every other call.
