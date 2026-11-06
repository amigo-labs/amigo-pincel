---
status: todo
created: 2026-11-06
size: L
mode: Image
design: [pincel.md §9.3, pincel.md §15 (2026-11-06 ComfyUI)]
depends_on: [comfyui-connect, mcp-live-desktop]
---

# Generate images with ComfyUI (Image mode)

## Context

With a ComfyUI server connected (`comfyui-connect`), Pincel generates images in three ways, chosen like krita-ai-diffusion does: text to image, image to image (refine the active layer with a strength below 100 %), and inpainting (new content inside the selection). The user works from a "Generate" panel; several variants come back, the user previews them on the canvas and applies one as a new layer. The same capability is the agent tool `gen.image`, so the chat agent (`ai-chat-panel`) can combine generation with Pincel's other tools.

Pincel uses built-in workflow templates made of ComfyUI core nodes only, so any stock ComfyUI works. Generation runs in the Tauri backend; `gen.*` are host tools of the desktop live endpoint, not part of `pincel-agent`, which stays free of I/O (Decision Log 2026-11-06). This spec covers Image mode; `comfyui-generate-pixel` adds Pixel mode.

## Terms

| Term | Meaning |
| ---- | ------- |
| Target box | The selection's bounding box clipped to the canvas, or the whole canvas without a selection — fixed when Generate is clicked (or `gen.image` is called) |
| Mask | The selection's shape including soft edges, fixed with the target box; none without a selection |
| Input | The active layer's pixels inside the target box, fixed with the target box (strength below 100 %, and the context around an inpainting mask) |
| Model family | SD 1.5, SDXL or Flux: from the catalog entry for catalog models (`comfyui-install`), else the "Model type" the user picks next to the model (default SDXL, remembered per checkpoint) |
| Generation size | Width and height for native pixel count N (SD 1.5: 512², SDXL and Flux: 1024²) and the target box's aspect ratio r, clamped to 1:4 – 4:1: width = √(N·r), height = √(N/r), each rounded to the nearest multiple of 64, at least 64 |
| Fitting | Scaling an image between the target box and the generation size without distortion: scaled to cover the destination, the overlap cropped evenly on both sides. Input and mask are fitted to the generation size; variants are fitted back to the target box |

## Scope

### In scope

- A "Generate" panel in the Image editor (desktop app only): prompt, negative prompt, model, optional LoRA with strength, strength, variant count, seed, Generate / Cancel, queue position and progress with live preview, variant strip, Apply / Discard
- Mode from the state: no selection and strength 100 % = text to image; strength below 100 % = image to image from the input; with a selection, generation is limited to the mask (inpainting at 100 %, refining below)
- Results as proposals until applied; Apply adds one new layer, one undo step
- Agent tools `gen.image` and `gen.models` on the live endpoint
- Errors: server not set up, not reachable, out of GPU memory, missing model, bad parameters, bad server answer
- English and German strings

### Out of scope

- Pixel mode — because downscaling and palette mapping are their own spec, `comfyui-generate-pixel`
- Outpainting, upscaling, control layers (ControlNet), live painting, regions — because the owner moved them to the backlog
- Custom ComfyUI workflows — because v1 ships built-in templates with core nodes only; backlog
- Headless `pincel-mcp` generation — because the headless server has no ComfyUI connection; `gen.*` are desktop host tools
- A history across generations — because a new generation replaces the strip (Assumptions)
- Upscaling the result beyond the generation size with an AI upscaler — because that is the backlog's upscaling feature; v1 scales with the normal resampler

## Acceptance criteria

- [ ] AC-01 (option): Where the app runs in a browser (PWA), the Generate panel is not shown.
- [ ] AC-02 (state): While no server is usable, the panel shows the status of `comfyui-connect` (not set up, not reachable, refused access, starting, too old, no model installed) with a "Set up…" button that opens View ▸ Image generation…, and Generate is disabled.
- [ ] AC-03 (always): The panel offers the server's checkpoints with their model family (a "Model type" choice for checkpoints outside the catalog), an optional LoRA (none by default) with a strength (0–2, default 1), a strength slider (1–100 %, default 100 %), a variant count (1–8, default 4) and a seed field (empty = random); model, LoRA, LoRA strength, strength, variant count and negative prompt persist across restarts; a stored model or LoRA the server no longer has falls back to the first available one with the notice "<name> is no longer installed"; the negative prompt is disabled for the Flux family, which ignores it.
- [ ] AC-04 (state): While the prompt is empty, or the target box is empty (selection entirely outside the canvas), Generate is disabled.
- [ ] AC-05 (event): When the user clicks Generate with no selection and strength 100 %, then the server generates the chosen number of variants from the prompt at the generation size for the canvas, and each variant is fitted to the canvas.
- [ ] AC-06 (event): When the user clicks Generate with strength below 100 %, then the input is sent to the server as the starting image together with the strength, and the variants are fitted to the target box.
- [ ] AC-07 (event): When the user clicks Generate with a selection, then the mask is sent with the request; in the previewed and applied result, pixels outside the mask equal the input, and pixels on a soft edge mix variant and input by the mask value.
- [ ] AC-08 (state): While a generation waits or runs, the panel shows its queue position ("Queued, 2 ahead") or the step progress ("Step 12 / 30") with the live preview image; drawing and every other editor action stay available, and changing the document does not change the target box, mask or input.
- [ ] AC-09 (event): When the user clicks Cancel, then this generation is removed from the server's queue or stopped if it runs, other jobs on the server are not affected, and nothing is added.
- [ ] AC-10 (event): When a generation finishes, then the strip shows one thumbnail per variant, the first is selected and shown as a preview on the canvas over the target box, and the document and its undo history are unchanged.
- [ ] AC-11 (event): When the user selects another thumbnail, then the canvas preview shows that variant instead.
- [ ] AC-12 (event): When the user clicks Apply, then a new layer named "Generated: " plus the first 30 characters of the prompt is added directly above the layer active at that moment (inside its group if it has one), holding the variant over the target box, its alpha multiplied by the mask when one was used and transparent outside the target box — as one undo step; strip and preview disappear.
- [ ] AC-13 (event): When the user clicks Discard or Generate again, then the current variants and their preview are dropped without changing the document.
- [ ] AC-14 (event): When the user switches tab while a generation runs or variants are shown, then the generation and the variants stay with their tab and are shown again when the user returns.
- [ ] AC-15 (error): If the server reports an error, returns fewer images than requested or no images, or becomes unreachable during a generation, then the panel shows the error (variants that did arrive stay usable), and Generate works again; an out-of-GPU-memory error reads "Not enough GPU memory — try fewer variants, a smaller model or a smaller selection".
- [ ] AC-16 (event): When an agent calls `gen.image` with `prompt` and optionally `negative`, `model` (default: the panel's model, else the first checkpoint), `lora` (default none), `lora_strength` (0–2, default 1), `strength` (0.01–1.0, default 1.0) and `seed` (integer ≥ 0, default random), then one variant is generated for the tab the call targets (the chat's tab, or the active tab for an external client) with its current selection and active layer, applied as in AC-12 without preview — variants the user has in the strip stay — and the result names the new layer, the seed and the model used.
- [ ] AC-17 (error): If `gen.image` names a model or LoRA the server does not have, or a value is out of range, then the call fails with `isError: true` and a message listing the valid names or the allowed range, before anything is sent to the server.
- [ ] AC-18 (event): When an agent calls `gen.models`, then the result lists the server's checkpoints and LoRAs by the names `gen.image` accepts.
- [ ] AC-19 (error): If an agent calls a `gen.*` tool while no server is usable, then the call fails with `isError: true` and the AC-02 status, in English; `tools/list` still lists the tools.
- [ ] AC-20 (error): If a `gen.image` call times out (`mcp-live-desktop` AC-19) or its tab closes, then its job is cancelled on the server and nothing is added.
- [ ] AC-21 (event): When the user closes a tab while its generation runs, then the generation is cancelled as in AC-09.
- [ ] AC-22 (always): The panel and its messages are translated in English and German.
- [ ] AC-23 (state, manual: needs a GPU and a real model): With SD 1.5 installed, "a red apple on a table" at strength 100 % produces four recognisable variants, and Apply adds one as a new layer that Undo removes; with a selection over the apple and "a green pear", only the selected area changes.

## Technical notes

- **Templates.** Three workflow templates in ComfyUI's API format: text to image (`CheckpointLoaderSimple` → optional `LoraLoader` → `CLIPTextEncode` ×2 → `EmptyLatentImage` → `KSampler` → `VAEDecode` → `SaveImage`), image to image (`LoadImage` → `VAEEncode`, `KSampler` denoise = strength), inpainting (`LoadImage` + `LoadImageMask` → `VAEEncodeForInpaint` at 100 %, `VAEEncode` + `SetLatentNoiseMask` below). Pincel fills prompt, seed (one seed per generation; variants are a batch with consecutive seeds), size, denoise, batch size and the uploaded input and mask (`POST /upload/image`, `/upload/mask`). Sampler presets per model family: SD 1.5 and SDXL 30 steps, CFG 7, `dpmpp_2m` / `karras`; Flux.1 schnell 4 steps, CFG 1, `euler` / `simple`, negative prompt left empty. Input and mask are fitted to the generation size before the upload.
- **Generation size.** Sides rounded to multiples of 64.
- **Progress.** `tokio-tungstenite` WebSocket to `/ws?clientId=…` for queue, `progress` and preview messages (Decision Log 2026-11-06, owner approved); results are read through `/history/{prompt_id}` and `/view` — `SaveImageWebsocket` is not a core node. Cancel: `POST /queue` with `delete` for a waiting job; `POST /interrupt` only when the running job is this one.
- **Applying.** Variants are kept at generation size and fitted when previewed or applied, so eight variants of a large canvas do not sit in memory at full size. The variant enters the document through the Image core's add-layer / paste commands (one undo step); a pending live object is committed first, as before any command (Decision Log 2026-11-05). The preview is a canvas overlay, never document state. App code never reads canvas pixels (AGENTS.md §9): input and mask come from the core.
- **Tests.** Rust tests against a mock ComfyUI (HTTP + WebSocket, fixed images, queue, errors); Playwright against the mocked Tauri bridge with element screenshots of the canvas. AC-23 is manual.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Selection of 3×3 pixels | Generated at the generation size for a 1:1 box, fitted down to 3×3 |
| Selection partly outside the canvas | The target box is clipped to the canvas |
| Very wide canvas (10000×500) | Generated at the 4:1 limit, fitted: the variant is scaled to cover 10000×500 and cropped top and bottom |
| Active layer fully transparent in the target box at strength below 100 % | Generates as at 100 % from the prompt, because there is nothing to refine; a hidden layer with content is used as it is |
| Active layer is a group | The input is the group's composite; Apply puts the new layer above the group |
| Active layer is locked | It is only read; Apply adds a new layer as usual |
| Active layer deleted before Apply | Apply uses the layer active at that moment (AC-12) |
| LoRA made for another model family | ComfyUI reports an error or produces noise; the error shows as AC-15 |
| Agent calls `gen.image` while the user's own generation runs | Both queue on the server; the panel shows "Queued, 1 ahead" |
| Two tabs generate at once | Both queue on the server; each panel shows its own job |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A new generation replaces the strip | Keeps v1 small; krita-ai-diffusion's history is a later extension | Users lose earlier variants on a new generation |
| Image to image reads the active layer, not the visible composite | Owner's choice (2026-11-06) | A user expecting the whole picture must merge first |
| The agent's `gen.image` applies its single variant directly | An agent cannot look at a preview strip; the chat's Undo per answer covers mistakes | none |
| Aspect ratio limited to 1:4 – 4:1 | SD-family models produce repeated or broken subjects beyond that | Very wide targets are cropped more (Fitting) |
| Variant count 1–8, default 4 | Four is krita-ai-diffusion's default and fits a strip; eight keeps a batch within mid-range VRAM | Large models may run out of memory at 8 (AC-15 message) |
| Apply places the variant over the target box fixed at Generate, even if the selection moved | The variant was generated for that area and context | A user who moved the selection must move the layer |
| The layer name uses 30 characters of the prompt | Readable in the layers panel; names may repeat | none |
| Closing a tab cancels its generation | Its result would have no document to land in | none |

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
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |
| AC-20 |      | open   |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |
| AC-23 |      | open   |        |

## Decisions

- 2026-11-06: V1 modes: text to image, image to image, inpainting into the selection (owner).
- 2026-11-06: Built-in templates with core nodes plus a model picker; custom workflows later (owner).
- 2026-11-06: Results are variants previewed on the canvas; Apply adds a new layer aligned to the selection (owner).
- 2026-11-06: A dedicated Generate panel; the editor stays usable during generation (owner).
- 2026-11-06: Live progress over ComfyUI's WebSocket with `tokio-tungstenite` (owner approved the dependency).
- 2026-11-06 (clarification pass): A variant whose aspect ratio differs from the target box is scaled to cover it and cropped evenly, never distorted (owner). Cancel touches only its own job, because the agent's and other tabs' jobs share the server queue.
