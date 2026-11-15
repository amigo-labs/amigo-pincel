---
status: todo
created: 2026-11-15
size: L
mode: Image
design: [pincel.md §15 (2026-11-06 ComfyUI), pincel.md §15 (2026-11-06 ComfyUI managed install), pincel.md §15 (2026-11-15 AI upscaling)]
depends_on: [comfyui-generate, comfyui-install]
---

# AI upscale with ComfyUI (Image mode)

## Context

Image ▸ Scale Image… enlarges with Bicubic, which makes a small photo bigger but not sharper. Upscale models (the ESRGAN family) are trained to enlarge and sharpen at once, and ComfyUI runs them with two core nodes. The owner asked for this on 2026-11-15, pointing to a comparison of a 512 px photo and its 4× upscale; `comfyui-generate` had left upscaling to the backlog.

This spec adds the model upscale: Image ▸ AI Upscale… sends the composite to the connected ComfyUI server (`comfyui-connect`), enlarges the document and puts the result on top as a new layer. The managed install (`comfyui-install`) gets an upscale model in its catalog, and agents get the tool `gen.upscale`. `ai-upscale-refine` adds the second pass of the comparison, a tiled image-to-image refine that invents fine detail.

Decided by the owner (2026-11-15): the document is enlarged and the result becomes a new layer; models come from the catalog and from what the server already has; the entry point is a dialog in the Image menu. Desktop app only, like every ComfyUI feature (Decision Log 2026-11-06).

## Terms

| Term | Meaning |
| ---- | ------- |
| Upscale model | A model file the server offers for its `UpscaleModelLoader` node (in ComfyUI's `models/upscale_models`) |
| Composite | The whole canvas as the editor shows it without overlays: all visible layers with their opacity and blend modes |
| Factor | 2, 3 or 4 — 2 and 4 are the scales upscale models have; 3 is the step between them that still fits the size limits when 4 does not |
| Target size | Document width × factor by document height × factor, in pixels |
| Size limits | Image mode's limits: 16384 px per edge and 8192 × 8192 px in area (`image-files` AC-16) |
| Server status | The status of `comfyui-connect`: not set up, not reachable, refused access, starting, stopped, too old, no model installed (= no checkpoint), connected |
| Server usable for upscaling | The server status is connected or no model installed — upscaling loads no checkpoint |
| Unchanged | Said of the document after a cancelled or failed upscale: pixels, layers, size and undo history are as before Upscale was clicked, apart from the commit of a pending live object (AC-08) |
| Nothing stored | No value for that setting has been saved yet on this machine (first use of the dialog) |

## Scope

### In scope

- Image ▸ AI Upscale… dialog in the Image editor (desktop app only): factor, upscale model, size readout, progress, Cancel
- Applying the result: document and existing layers scaled to the target size, the result as a new top layer, one undo step
- An upscale model package in the managed install's catalog (Real-ESRGAN x4plus)
- Upscale models listed in View ▸ Image generation… and by `gen.models`
- Agent tool `gen.upscale` on the live endpoint
- Errors: server not usable, no upscale model, server too old for upscaling, image too large, out of GPU memory, bad server answer
- Strings in all six UI locales

### Out of scope

- A generative refine pass (image to image in tiles) — because it is the next spec, `ai-upscale-refine`
- Pixel mode — because pixel art scales by whole factors with hard edges (Nearest, RotSprite); a model upscaler blurs it into something that is no longer pixel art
- A face or skin fix — because the owner decided (2026-11-15) that selecting the face and using Generate at a low strength covers it
- Factors other than 2, 3 and 4, and a free target size — because most upscale models have a scale of 2 or 4 and Scale Image… covers free sizes afterwards
- Running the model several times to reach a factor above its own scale — because one pass plus resampling is predictable in time and memory (Assumptions)
- Upscaling only a selection or a single layer — because the result has the target size of the whole document; a part cannot be larger than its canvas
- The PWA and headless `pincel-mcp` — because neither has a ComfyUI connection (Decision Log 2026-11-06)
- Arbitrary upscale model downloads — because every catalog package needs a pinned hash and a shown license (`comfyui-install`); other models are added to the server by the user

## Acceptance criteria

### Dialog

- [ ] AC-01 (option): Where the app runs in a browser (PWA), Image ▸ AI Upscale… is not shown.
- [ ] AC-02 (always): In the desktop app, Image ▸ AI Upscale… sits directly below Scale Image… and is enabled while a document is open. The dialog shows the factor choice 2× / 3× / 4× (2× while nothing is stored), the server's upscale models in the server's order (the first one while nothing is stored), the document's size and the target size in pixels ("1024 × 768 → 2048 × 1536", updated when the factor changes), and the buttons Upscale and Cancel. Factor and model persist across restarts; a stored model the server no longer has falls back to the first one with the notice "<name> is no longer installed".
- [ ] AC-03 (event): When the dialog opens, and again after an error of AC-14, then a server is needed in the sense of `comfyui-connect` AC-03 and AC-06 (discovery; an installation Pincel can start is started and shows "starting" until it answers). While the server is not usable for upscaling — not set up, not reachable, refused access, starting, stopped or too old — the dialog shows that status with a "Set up…" button that closes it and opens View ▸ Image generation…, and Upscale is disabled; the dialog follows the status, so Upscale becomes available when a starting server answers.
- [ ] AC-04 (state): While the connected server has no upscale model, the dialog shows "No upscale model installed" with the "Set up…" button, and Upscale is disabled.
- [ ] AC-05 (error): If the server is usable for upscaling but lacks a node type the upscale workflow needs, then the dialog shows "This ComfyUI is too old — update it" and names the missing node types, and Upscale is disabled; the Generate panel's status is not affected.
- [ ] AC-06 (state): While a factor's target size exceeds the size limits, that factor cannot be chosen, and a stored factor that cannot be chosen falls back to the largest one that can; while even 2× exceeds them, the dialog shows "The image is too large to upscale" and Upscale is disabled.
- [ ] AC-07 (state): While a generation of this tab waits or runs or its variants are shown (`comfyui-generate` AC-08, AC-10), the dialog shows "Finish or discard the generation first"; while an upscale started by an agent runs in any tab (AC-18), it shows "An upscale is already running"; in both cases Upscale is disabled.

### Upscaling

- [ ] AC-08 (event): When the user clicks Upscale, then a pending live object is committed as before any command — the only change a cancelled or failed upscale leaves behind — and the composite's colour is sent to the server to be upscaled with the chosen model.
- [ ] AC-09 (state): While the upscale waits or runs, the dialog shows its queue position ("Queued, 2 ahead") or its progress in percent and a Cancel button, the settings are disabled, and the dialog cannot be closed (no close button, Escape or click outside); Cancel stops the upscale (AC-13); the document is unchanged.
- [ ] AC-10 (event): When the server returns the result, then, as one undo step named "AI Upscale" in the History panel: the document has the target size; every existing layer is scaled as Scale Image… with Bicubic scales it (order, groups, names, visibility, opacity and blend modes unchanged); a new layer named "Upscaled " plus the factor ("Upscaled 4×") is added at the top of the layer stack outside every group, visible, at 100 % opacity with the normal blend mode, and is the active layer; the selection is dropped; the dialog closes. Undo restores the size, the layers, every pixel and the selection; Redo applies the step again without contacting the server.
- [ ] AC-11 (always): The new layer covers the whole canvas at exactly the target size, whatever scale the model has: the server is asked to deliver the model's output resampled to the target size, so a 4× model at factor 2 and a 2× model at factor 4 both give the target size, and Pincel never receives an image larger than the size limits. A server result of any other size is refused as a bad answer (AC-14).
- [ ] AC-12 (always): The new layer's colour comes from the server's result, and its alpha equals the alpha of the composite as it was sent (AC-08), scaled to the target size as Scale Image… with Bicubic scales it: a fully opaque composite gives a fully opaque layer, a fully transparent composite a fully transparent one.
- [ ] AC-13 (event): When the user clicks Cancel while the upscale waits or runs — at any moment before the result is applied — or the app quits, then the job is removed from the server's queue or stopped if it runs, other jobs on the server are not affected, and the document is unchanged; after Cancel the dialog shows its settings again.
- [ ] AC-14 (error): If the server reports an error, returns no image, an image that cannot be decoded or one that does not have the target size, or becomes unreachable during the upscale, then the dialog shows the error, its settings are enabled again, the document is unchanged, and Upscale can be clicked again; an out-of-GPU-memory error, recognised as in `comfyui-generate` AC-15, reads "Not enough GPU memory — try a smaller factor or a smaller image".

### Models

- [ ] AC-15 (always): The package list of the install wizard and of adding or removing packages later (`comfyui-install` AC-04, AC-11) additionally offers "Real-ESRGAN x4plus (upscaler)" with its download size and license (BSD-3-Clause), selected when the wizard opens for a new installation; it can be selected and removed independently of every other package and does not count as a checkpoint. Its license notice, download, hash check, errors and retry follow `comfyui-install` AC-05 – AC-09.
- [ ] AC-16 (event): When the package has been installed — also into a managed installation created before this feature existed — then the managed server lists the model as an upscale model once it runs again, and when the package is removed, the model is gone from that list.
- [ ] AC-17 (state): While a server is connected, View ▸ Image generation… lists its upscale models next to its checkpoints and LoRAs (`comfyui-connect` AC-04).

### Agent tools

- [ ] AC-18 (event): When an agent calls `gen.upscale` with optionally `factor` (2, 3 or 4; default 2) and `model` (default: the dialog's stored model if the server has it, else the server's first upscale model), then a server is needed as for every `gen.*` call (`comfyui-connect` AC-03), and the document of the tab the call targets (the chat's tab, or the tab active when the call arrives for an external client) is upscaled as in AC-08 and AC-10 – AC-12 without a dialog — also when another tab has become active meanwhile — and the result names the new layer, the model and the new document size.
- [ ] AC-19 (error): If `gen.upscale` names a model the server does not have, a factor other than 2, 3 or 4, or a factor whose target size exceeds the size limits, then the call fails with `isError: true` and a message listing the valid model names, the allowed factors or the size limits, before anything is sent to the server.
- [ ] AC-20 (error): If an agent calls `gen.upscale` while the server is not usable for upscaling (AC-03), it has no upscale model (AC-04), it is too old (AC-05), a generation is pending in that tab (AC-07), another upscale runs in the app (dialog or agent), or the targeted tab is not an Image-mode document, then the call fails with `isError: true` and that reason in English, nothing is sent to the server, and `tools/list` still lists the tool.
- [ ] AC-21 (error): If a `gen.upscale` call times out (`mcp-live-desktop` AC-19, the `gen.*` limit), its tab closes, the server fails as in AC-14, or — when the result arrives — the document's size differs from the size at the start of the call or a generation is pending in that tab (AC-07), then its job is cancelled on the server where it still runs, the document is unchanged, and the call fails with `isError: true` naming the reason in English (for a server failure the texts of AC-14).
- [ ] AC-22 (event): When an agent calls `gen.models`, then the result additionally lists the server's upscale models by the names `gen.upscale` accepts; the checkpoint and LoRA lists are unchanged.

### Language and hardware

- [ ] AC-23 (always): The menu item, the dialog, its notices and errors and the package name are translated in all six UI locales (en, de, es, ja, fr, pt-BR); tool results and tool errors are English.
- [ ] AC-24 (state, manual: needs a GPU and the real model; a visual judgement): With Real-ESRGAN x4plus installed, upscaling a 512 × 288 photo at 4× gives a 2048 × 1152 document; toggling the top layer's visibility at 100 % zoom, a person sees fine structures (hair, text, edges) drawn narrower and clearer in the top layer than in the Bicubic-scaled layer below it; Undo restores the 512 × 288 document.

## Technical notes

- **Upload.** The composite is uploaded as three colour channels: straight (not premultiplied) colour, fully transparent pixels as black. Upscale models take no alpha; Pincel scales the alpha itself (AC-12).
- **Workflow.** One template in ComfyUI's API format: `LoadImage` → `ImageUpscaleWithModel` (model from `UpscaleModelLoader`) → `ImageScale` to the target width and height (method `lanczos`, no crop) → `SaveImage`. Resampling on the server keeps a 4× model's output at factor 2 or 3 (up to four times the target's pixels) out of Pincel's memory. Upload, queue, progress, cancel and result download work as in `comfyui-generate` (`/upload/image`, WebSocket progress, `/history/{prompt_id}`, `/view`). ComfyUI tiles a large input inside `ImageUpscaleWithModel` itself.
- **Node check.** `UpscaleModelLoader`, `ImageUpscaleWithModel` and `ImageScale` are checked on their own (AC-05), not added to `comfyui-connect`'s required list, so a server without them still generates.
- **Model list.** From the `object_info` of `UpscaleModelLoader` (`model_name`), read with the checkpoints and LoRAs.
- **Catalog.** `RealESRGAN_x4plus.pth` from the Real-ESRGAN project's GitHub release (downloadable without login, about 64 MB, BSD-3-Clause), into `<folder>/models/upscale_models`. `extra_model_paths.yaml` gains `upscale_models: upscale_models` and is rewritten when a package is added, so an older managed installation picks the folder up (AC-16). URL, size and SHA-256 go into the pins table.
- **Applying.** One command of `pincel-image-core` scales the document (the Scale Image code path) and adds the layer, as one history entry. App code never reads canvas pixels (AGENTS.md §9): the composite comes from the core. The result of a 4× upscale can be 100 MB as PNG; it crosses the IPC boundary once, compressed, and is decoded in the core (Gotchas: Tauri IPC is slow for bulk bytes).
- **New public API** (for review): a core command for the upscale step and a history kind; in `pincel-image-wasm` a function returning the composite for upload and one applying the result; host tool `gen.upscale` and the `upscale_models` field of `gen.models` in `pincel-tauri`.
- **Tests.** Rust tests against the mock ComfyUI server (`src-tauri/src/comfyui/testing.rs`) and unit tests in `pincel-image-core`; Playwright against the mocked Tauri bridge (`ui/e2e/tauri-mock.ts`), as `ui/e2e/comfyui-generate.spec.ts` does. AC-24 is manual.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 4× model, factor 2 | The server reduces the model's 4× output to the target size (AC-11) |
| 2× or 1× model, factor 4 | The server enlarges the model's output to the target size (AC-11); softer than a 4× model, but the size is as chosen |
| 5000 × 4000 document | 2× would be 80 MP, over the area limit: "The image is too large to upscale" (AC-06) |
| 3000 × 2000 document | 2× (24 MP) and 3× (54 MP) can be chosen, 4× (96 MP) cannot (AC-06); with a 4× model the server computes 96 MP and delivers the target size (AC-11) |
| Document with hidden layers | Hidden layers are not part of the composite; they are scaled like every layer and stay hidden |
| Cut-out with transparent background | Colour is upscaled from the composite's straight colour with fully transparent pixels as black, alpha is scaled separately (AC-12); soft edges may show a faint dark fringe (Assumptions) |
| Live text or a floating selection is open | Committed when Upscale is clicked (AC-08); the commit stays as its own undo step if the upscale is cancelled or fails |
| Server stops during the upscale | Error as AC-14; the dialog asks for the server again (AC-03), which starts an installation Pincel can start, and Upscale is available once it answers |
| The server delivers an image that does not have the target size | Refused as a bad answer (AC-11, AC-14); the document is unchanged |
| An agent tool call arrives while the dialog's upscale runs | It fails at once, as every call during a modal dialog (`mcp-live-desktop` AC-18); the document cannot change under the dialog |
| The upscale never progresses (a stuck server) | The dialog keeps showing its progress; Cancel ends it (AC-13). The dialog has no time limit of its own (Assumptions) |
| An agent calls `gen.upscale` while the dialog's upscale runs, or two agents call it at once | The later call fails with "an upscale is already running" (AC-20) |
| The user clicks Generate while an agent's upscale runs in that tab | The generation starts; when the upscale's result arrives, the call fails and the document is unchanged (AC-21) |
| `gen.upscale` targets a Pixel-mode tab or no document is open | The call fails, naming Image mode (AC-20) |
| Another program's jobs are in the server's queue | The dialog shows the queue position (AC-09); Cancel removes only Pincel's job (AC-13) |
| User edits the document while an agent's `gen.upscale` runs | Edits stay in the scaled layers below the new layer; only a changed document size fails the call (AC-21) |
| Managed install without the upscaler package | "No upscale model installed" with "Set up…" (AC-04); the package can be added there (AC-15) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| One model pass, then resampling to the target size on the server | ComfyUI does not report a model's scale before it runs; one pass keeps time predictable, and resampling there keeps Pincel's memory within the size limits | A 2× model at factor 4 is softer than two passes would be; a 4× model at factor 2 near the limits needs server memory for four times the target |
| Default factor 2× | The smallest result; 4× of a 4 MP photo is already 64 MP | Users of small images pick 4× once; the choice persists |
| The dialog is modal while the upscale runs | A model upscale is one pass without sampling steps, and the step resizes the document, so edits meanwhile would be covered by the new layer | A slow machine blocks the editor for minutes; Cancel is always available |
| The dialog's upscale has no time limit | The user watches the progress and can cancel; a limit would have to guess the machine's speed | A stuck server is only ended by Cancel |
| The `gen.*` time limit (5 minutes) is enough for an agent's upscale | The model pass has no sampling steps; results near the size limits are rare for agent calls | A slow GPU with a large image times out; the dialog has no limit |
| The upscaler package is selected in a new installation's wizard | 64 MB next to checkpoints of several GB, and without it Image ▸ AI Upscale… only shows "No upscale model installed" | Users who never upscale download 64 MB; they can deselect it |
| One upscale at a time in the app | One result of up to 256 MB is in memory at a time, and the server works through one queue anyway | An agent has to wait for the dialog's upscale |
| Transparent pixels are sent as black | Upscale models take three colour channels; alpha is scaled by Pincel | A faint dark fringe on soft edges of cut-outs |
| Real-ESRGAN x4plus is the catalog model | BSD-3-Clause allows commercial use, downloads without login, 64 MB; other well-known upscalers carry non-commercial licenses | Users who prefer another model add it to the server themselves |
| The result layer is opaque for an opaque composite and covers the scaled layers | The owner chose "document + new layer"; hiding or deleting the layer shows the Bicubic-scaled original for comparison | none |

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
| AC-24 |      | open   |        |

## Decisions

- 2026-11-15: The document is enlarged and the upscale becomes a new top layer, one undo step (owner) — existing layers stay, scaled with Bicubic, and the user can compare by hiding the new layer.
- 2026-11-15: Upscale models come from the catalog (Real-ESRGAN x4plus) and from whatever the connected server has (owner) — the managed install works out of the box, and users with their own ComfyUI keep the models they prefer.
- 2026-11-15: Entry point is the dialog Image ▸ AI Upscale…, not the Generate panel (owner) — it changes the document's size like its neighbour Scale Image…, and the panel stays about generating.
- 2026-11-15: No face or skin fix; selection plus Generate at a low strength covers it (owner).
- 2026-11-15: Upscaling does not need a checkpoint, so the status "no model installed" does not block it — an upscale model is the only model the workflow loads.
- 2026-11-15: The selection is dropped by the upscale and restored by Undo, as Resize Canvas does — the selection mask is canvas-sized, and a resampled mask of a 4× larger canvas would be a guess at what the user selected.
- 2026-11-15: The server resamples the model's output to the target size (`ImageScale`, a core node) — otherwise a 4× model at factor 2 or 3 would hand Pincel up to four times the target's pixels, beyond the size limits the factor choice just checked.
- 2026-11-15: The upscale node types are checked separately from `comfyui-connect`'s required list — a server without them must keep generating.
