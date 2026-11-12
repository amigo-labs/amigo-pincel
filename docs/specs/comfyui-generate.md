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
| AC-01 | e2e `the PWA has no Generate panel (AC-01)`, `the desktop app has a collapsed Generate panel (AC-01)` (ui/e2e/comfyui-generate.spec.ts) | built |  |
| AC-02 | e2e `without a usable server the panel shows the status and Set up… (AC-02)` (ui/e2e/comfyui-generate.spec.ts); `seed, model family and status (AC-02, AC-03)` (ui/e2e/comfyui-generate-params.spec.ts) | built |  |
| AC-03 | e2e `the panel offers models, LoRA, strength, variants and seed, and keeps them (AC-03)` (ui/e2e/comfyui-generate.spec.ts); `seed, model family and status (AC-02, AC-03)` (ui/e2e/comfyui-generate-params.spec.ts); `catalog_checkpoints_know_their_family`, `lora_feeds_model_and_clip_and_flux_drops_the_negative_prompt` (src-tauri/src/comfyui/generate/tests.rs) | built |  |
| AC-04 | e2e `Generate needs a prompt (AC-04)` (ui/e2e/comfyui-generate.spec.ts); `the target box is the selection clipped to the canvas, empty outside it (AC-04)` (ui/e2e/comfyui-generate-params.spec.ts) | built |  |
| AC-05 | e2e `text to image, image to image and inpainting send what the state says (AC-05 – AC-07)` (ui/e2e/comfyui-generate.spec.ts); `text_to_image_has_one_sampler_branch_per_variant_with_consecutive_seeds`, `a_job_uploads_queues_follows_and_keeps_the_variants` (src-tauri/src/comfyui/generate/tests.rs); `generation_size_keeps_the_pixel_count_and_the_ratio_limit`, `fit_covers_and_crops_evenly_without_distortion` (crates/pincel-image-core/src/generate.rs) | built |  |
| AC-06 | e2e `text to image, image to image and inpainting send what the state says (AC-05 – AC-07)` (ui/e2e/comfyui-generate.spec.ts); `image_to_image_encodes_the_input_and_denoises_by_the_strength` (src-tauri/src/comfyui/generate/tests.rs); `mask_and_input_come_from_the_selection_and_the_active_layer` (crates/pincel-image-core/src/generate.rs); `generation_inputs_come_from_the_active_layer_and_the_selection` (crates/pincel-image-wasm/src/generate.rs) | built |  |
| AC-07 | e2e `text to image, image to image and inpainting send what the state says (AC-05 – AC-07)`, `Apply adds one named layer over the target box, masked, as one undo step (AC-07, AC-12)` (ui/e2e/comfyui-generate.spec.ts); `inpainting_uses_vae_encode_for_inpaint_at_full_strength_and_a_noise_mask_below` (src-tauri/src/comfyui/generate/tests.rs); `fit_mask_and_apply_mask_blend_by_coverage` (crates/pincel-image-core/src/generate.rs); `a_fitted_variant_lands_as_one_named_layer_that_undo_removes` (crates/pincel-image-wasm/src/generate.rs) | built |  |
| AC-08 | e2e `a waiting or running generation shows its place and progress; the editor stays usable (AC-08)` (ui/e2e/comfyui-generate.spec.ts); `a_waiting_job_reports_how_many_are_ahead`, `a_job_uploads_queues_follows_and_keeps_the_variants`, `without_the_socket_a_job_is_polled_to_its_end` (src-tauri/src/comfyui/generate/tests.rs); `event_reads_only_this_prompts_events`, `preview_reads_both_frame_types` (src-tauri/src/comfyui/generate/job.rs) | built |  |
| AC-09 | e2e `Cancel removes the job and adds nothing (AC-09)` (ui/e2e/comfyui-generate.spec.ts); `cancel_deletes_a_waiting_job_and_leaves_other_jobs_alone`, `cancel_interrupts_a_running_job_by_its_own_id` (src-tauri/src/comfyui/generate/tests.rs) | built |  |
| AC-10 | e2e `finished variants show in the strip and on the canvas; the document stays (AC-10, AC-11)` (ui/e2e/comfyui-generate.spec.ts) | built |  |
| AC-11 | e2e `finished variants show in the strip and on the canvas; the document stays (AC-10, AC-11)` (ui/e2e/comfyui-generate.spec.ts) | built |  |
| AC-12 | e2e `Apply adds one named layer over the target box, masked, as one undo step (AC-07, AC-12)` (ui/e2e/comfyui-generate.spec.ts); `the layer name is "Generated: " and 30 characters of the prompt (AC-12)` (ui/e2e/comfyui-generate-params.spec.ts); `labelled_paste_names_the_layer_and_records_its_kind` (crates/pincel-image-core/src/command/paste.rs); `a_fitted_variant_lands_as_one_named_layer_that_undo_removes` (crates/pincel-image-wasm/src/generate.rs) | built |  |
| AC-13 | e2e `Discard and Generate again drop the variants (AC-13)` (ui/e2e/comfyui-generate.spec.ts); `releasing_a_running_job_cancels_it` (src-tauri/src/comfyui/generate/tests.rs) | built |  |
| AC-14 | e2e `a generation and its variants stay with their tab (AC-14)` (ui/e2e/comfyui-generate.spec.ts) | built |  |
| AC-15 | e2e `errors show in the panel; variants that arrived stay usable (AC-15)` (ui/e2e/comfyui-generate.spec.ts); `out_of_memory_keeps_the_variants_that_arrived`, `fewer_images_than_requested_finish_short_and_none_fail`, `a_model_the_server_refuses_is_reported_missing`, `a_server_that_stops_mid_generation_is_unreachable` (src-tauri/src/comfyui/generate/tests.rs); `failed_recognises_out_of_memory` (src-tauri/src/comfyui/generate/job.rs) | built |  |
| AC-16 | e2e `gen.image generates one variant and applies it; the panel keeps its variants (AC-16)`, `a generation error fails the call in English (AC-16, AC-19)` (ui/e2e/comfyui-generate-tools.spec.ts); `gen.image defaults to the panel model, else the first checkpoint (AC-16, AC-17)` (ui/e2e/comfyui-generate-params.spec.ts); `generation_tools_go_to_the_editor_and_an_expired_one_is_cancelled_there` (src-tauri/src/mcp/tests.rs) | built |  |
| AC-17 | e2e `bad names and values fail before anything is sent (AC-17)` (ui/e2e/comfyui-generate-tools.spec.ts); `gen.image defaults to the panel model, else the first checkpoint (AC-16, AC-17)` (ui/e2e/comfyui-generate-params.spec.ts); `bad_values_and_unknown_models_are_refused_before_anything_is_sent` (src-tauri/src/comfyui/generate/tests.rs) | built |  |
| AC-18 | e2e `gen.models lists the names gen.image accepts (AC-18)` (ui/e2e/comfyui-generate-tools.spec.ts) | built |  |
| AC-19 | e2e `without a usable server gen.* fail with the status (AC-19)` (ui/e2e/comfyui-generate-tools.spec.ts); `tools_list_matches_the_headless_server` (lists `gen.image`, `gen.models` after the document tools), `generation_tools_go_to_the_editor_and_an_expired_one_is_cancelled_there` (src-tauri/src/mcp/tests.rs) | built |  |
| AC-20 | e2e `an expired call or a closed tab cancels the job and adds nothing (AC-20)` (ui/e2e/comfyui-generate-tools.spec.ts); `generation_tools_go_to_the_editor_and_an_expired_one_is_cancelled_there` (src-tauri/src/mcp/tests.rs) | built |  |
| AC-21 | e2e `closing a tab cancels its generation (AC-21)` (ui/e2e/comfyui-generate.spec.ts) | built |  |
| AC-22 | e2e `the panel and its messages are German in the German UI (AC-22)` (ui/e2e/comfyui-generate.spec.ts); key parity of all six dictionaries in `pnpm check` (`Record<MessageKey, string>`) and `scripts/ui-lint.mjs` | built |  |
| AC-23 |      | open   |        |

## Decisions

- 2026-11-06: V1 modes: text to image, image to image, inpainting into the selection (owner).
- 2026-11-06: Built-in templates with core nodes plus a model picker; custom workflows later (owner).
- 2026-11-06: Results are variants previewed on the canvas; Apply adds a new layer aligned to the selection (owner).
- 2026-11-06: A dedicated Generate panel; the editor stays usable during generation (owner).
- 2026-11-06: Live progress over ComfyUI's WebSocket with `tokio-tungstenite` (owner approved the dependency).
- 2026-11-06 (clarification pass): A variant whose aspect ratio differs from the target box is scaled to cover it and cropped evenly, never distorted (owner). Cancel touches only its own job, because the agent's and other tabs' jobs share the server queue.
- 2026-11-12 (spec-build): Each variant is its own sampler branch (`KSampler` → `VAEDecode` → `SaveImage`) with seed `seed + i`, not one latent batch: image to image and inpainting encode a single image, and repeating a latent needs `RepeatLatentBatch`, which is not among the required core nodes (comfyui-connect); a branch per variant also makes every variant reproducible from its seed, so `gen.image` with that seed gives the same picture.
- 2026-11-12 (spec-build): The mask is uploaded through `POST /upload/image` and read with `LoadImageMask` (red channel), not through `/upload/mask`, which writes a mask into the alpha of an already uploaded image.
- 2026-11-12 (spec-build): The progress socket uses the synchronous tungstenite API that `tokio-tungstenite` re-exports, with only its `handshake` feature: the generation jobs run on blocking threads like the rest of the ComfyUI backend, so no `tokio` or `futures` dependency is added. Servers behind `https` get no socket (it would need a TLS stream crate); their jobs are followed by polling `/queue` and `/history`, without live preview.
- 2026-11-12 (spec-build): A finished job's images stay in the backend until the editor fetches them as raw bytes (`comfyui_variant`, binary IPC response) and releases the job; the editor keeps them at generation size and fits the selected one when it previews or applies it (Technical notes). Generation size, fitting (supersampling when shrinking, bicubic when growing) and masking live in `pincel-image-core::generate`, called through `pincel-image-wasm`; input and mask come from the core, never from the canvas.
- 2026-11-12 (spec-build): The new layer's name starts with the English "Generated: " in every language: a layer name is document content, which the UI never translates (AGENTS.md §5.4). The step is a new history kind `generate` ("Generate Image").
- 2026-11-12 (spec-build): A stored LoRA the server no longer has falls back to "None", the first entry of the LoRA list (AC-03); a stored model falls back to the first checkpoint. Both show "<name> is no longer installed".
- 2026-11-12 (spec-build): Prompt and seed are panel fields shared by the tabs and not stored (AC-03 lists what persists); the panel settings are the IndexedDB pref `comfyGenerate`, the Model type choices per checkpoint included. The panel starts collapsed, so no ComfyUI starts before the user expands it (comfyui-connect AC-03, Decisions).
- 2026-11-12 (spec-build): A layer that is fully transparent in the target box generates as at 100 % (edge cases) — with a selection that is text to image whose result is then masked, since there is no context to inpaint around.
- 2026-11-12 (spec-build): The live preview image shows in the panel next to the progress; the canvas overlay shows the selected finished variant (AC-10). The overlay lies above all layers, so a layer above the active one is previewed under the variant; the applied layer sits directly above the active layer (AC-12).
- 2026-11-12 (spec-build): The backend counts its running jobs and sets the managed install's guard (`Service::set_generating`, comfyui-install AC-14) itself; the `comfyui_set_generating` command stays, unused by the editor.
- 2026-11-12 (spec-build): `gen.*` calls are forwarded to the webview like every call (the endpoint only lists them, mcp-live-desktop AC-06) and run there beside the queue of calls, so a generation of minutes does not hold up other calls; the endpoint tells the webview when a call timed out (`mcp-call-expired`), which cancels its job (AC-20). `gen.image` on a Pixel tab fails with "gen.image works on Image documents only" until `comfyui-generate-pixel`.
- 2026-11-12 (spec-build): Red runs. Rust jobs: the generation tests failed with the job runner stubbed (`Failed { BadAnswer }`); the workflow-template tests were written with the templates and green at once. Core: every `generate::` test and the labelled paste failed with the helpers stubbed. E2e: `comfyui-generate.spec.ts` failed 14 of 15 against the build before the UI block (the PWA test of AC-01 was green, as the PWA had no panel before either); `comfyui-generate-tools.spec.ts` failed 7 of 7 against the build before the `gen.*` block; the Node-side `comfyui-generate-params.spec.ts` was written after its code and green at once. The wasm binding tests were green at once on top of the red-then-green core.
- 2026-11-12 (spec-build): AC-04's empty target box cannot be reached in the editor — a selection lives on the canvas, so its bounds always overlap it; the rule is tested on `targetBox` (Node-side spec).
