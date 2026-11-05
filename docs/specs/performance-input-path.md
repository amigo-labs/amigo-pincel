---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §5.2, pincel.md §9.4, pincel.md §17.4]
depends_on: [performance-benchmarks, pixel-pencil]
---

# Performance: input path

## Context

Pointer input decides how fast painting feels. Image mode already reads coalesced pointer samples, but hands them to the core and presents per sample; Pixel mode calls the core once **per pixel** of a drag (`applyTool`). Step 3 of the legacy plan `23-performance` (removed 2026-10-29, see git history) batches all samples of one animation frame, presents once per frame, and makes sure the browser never holds pointer events back for scrolling.

This spec owns the Pixel budget from `performance-benchmarks`: a pencil drag on a 256×256 sprite at zoom 32 runs at 60 fps with a dab latency of at most 8 ms, measured from the pointer event to the draw call (the work Pincel controls).

## Scope

### In scope

- Batching all pointer samples of one animation frame into one stroke update and one presentation, in both modes
- Pixel mode reading coalesced pointer samples
- Canvas pointer handling that never waits on page scrolling or zooming

### Out of scope

- The Pixel stroke command and its gesture API (`beginStroke` / `strokeTo` / `endStroke`) — because `pixel-pencil` defines them; this spec only feeds them batched samples
- Image-mode dab cost — because `performance-image-cache` owns the Image dab budget
- Pen pressure and tilt — because `pixel-pressure` and the existing Image pen support cover them
- Predicted pointer tip (`getPredictedEvents()`) — follow-up, because the plan marks it optional, it needs its own overlay design (drawing and removing the provisional tip), and the dab latency is to be measured first

## Acceptance criteria

- [ ] AC-01 (always): A stroke yields the same document pixels whether its pointer samples reach the core one at a time or batched per animation frame, in both modes.
- [ ] AC-02 (event): When the browser delivers several coalesced pointer samples in one event during a Pixel pencil drag, then the stroke passes through every sample's pixel.
- [ ] AC-03 (always): A touch or pen drag that starts on the canvas of either editor never scrolls, zooms or reloads the page.
- [ ] AC-04 (state, manual: the budget is defined for the reference laptop): While the user drags the Pixel pencil on a 256×256 sprite at zoom 32, the browser performance script measures a p95 frame time of at most 16.7 ms and a p95 dab latency of at most 8 ms, where dab latency runs from the page receiving the pointer event to the first draw call on the render canvas after it (`pixel-pencil-256-z32` › `dab-draw-latency-p95`) — not from the event's hardware `timeStamp`, and not to the frame that presents the draw.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- Image mode: `getCoalescedEvents` in `ui/src/modes/image/tools/pointer.ts` (≈L388). Batch all samples of one rAF into a single wasm call; present once per rAF.
- Pixel mode: `applyTool` per pixel in `crates/pincel-wasm/src/lib.rs` (≈L552); pointer handling in `ui/src/modes/pixel/pointer.ts`. Use `getCoalescedEvents`, and feed all coalesced samples of one animation frame to `pixel-pencil`'s `strokeTo` once per frame — `pixel-pencil` is built first (`depends_on`); `shading-ink` and `symmetry-tiled-mode` build on the same stroke API, so nothing is wired twice.
- Check `touch-action` / `pointer-events` CSS so the browser never waits on scroll handling (`touch-action: none` exists today on the layers panel and the tile editor, not verified on both canvas stages). AC-03 can be checked in e2e through the canvas stage's computed style and a touchscreen drag.
- Playwright cannot synthesize coalesced events; AC-01 / AC-02 are best proven at the wasm / core level.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Browser without `getCoalescedEvents` | Falls back to one sample per event; strokes stay continuous |
| Pointer leaves the canvas mid-drag with pending batched samples | The pending samples are applied before the stroke ends (the stroke is kept, as today) |
| A single click without movement | One dab, presented in the next frame |
| Two pointers (pinch) start on the canvas | Pinch zoom as today; no stroke is painted |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | Pixel: `crates/pincel-wasm/src/stroke.rs` › `stroke_to_batch_matches_per_sample_stroke_to` (uneven batches of 1–9 points vs one `strokeTo` per point; sizes, shapes, pixel-perfect, pressure, eraser). Image: `crates/pincel-image-core/tests/stroke_batching.rs` › `stroke_batched_per_frame_paints_like_one_sample_per_event` (pencil, eraser, recolor, smudge; soft brush, opacity, pressure) | built |        |
| AC-02 | `crates/pincel-wasm/src/stroke.rs` › `stroke_to_batch_passes_through_every_sample`; `ui/e2e/input-path.spec.ts` › "pixel: a move event with several coalesced samples paints through every sample (AC-02)" | built |        |
| AC-03 | `ui/e2e/input-path.spec.ts` › "pixel: a touch drag on the canvas paints and never scrolls, zooms or reloads (AC-03)", "image: … (AC-03)" | built |        |
| AC-04 | manual — `pnpm perf -g pixel` then `pnpm perf:check --browser e2e-perf/results/browser.json` (`pixel-pencil-256-z32` › `frame-p95`, `dab-draw-latency-p95`); measured 2026-11-05 under Decisions | open   |        |

## Decisions

- The input path never waits for heavy work: samples are batched per animation frame and presented once (plan, 2026-10-28).
- 2026-10-29: `pixel-pencil` is built first and this spec feeds its `strokeTo` with all coalesced samples once per animation frame, instead of coalescing on today's `applyTool` / `applyLine` — the plan's design uses `strokeTo`, and `shading-ink` and `symmetry-tiled-mode` build on the same stroke API, so nothing gets wired twice (owner accepted the recommendation).
- 2026-11-05 (spec-build): New wasm API `Document.strokeToBatch(xy: Int32Array, pressure: Float32Array)` (pincel.md §17.5): paints exactly what one `strokeTo` per point paints and emits one `dirty-rect` for the batch; an empty `pressure` means full pressure. The Pixel pointer handler queues samples (`ui/src/modes/pixel/stroke-batch.ts`), the render tick flushes them in one call before it drains events and presents, and pointer-up flushes before `endStroke` — one wasm call and one dirty event per frame instead of one per sample.
- 2026-11-05 (spec-build): Image mode queues brush samples per animation frame (`ui/src/modes/image/tools/frame-batch.ts`) and sends each batch prefixed with the last point sent, the way the core already joins consecutive commands of one stroke; the press dab stays its own command (a click is one dab) and shows in the next frame. Pointer-up and a cancelled pointer flush what is queued, so pending samples are kept as the stroke's pixels.
- 2026-11-05 (spec-build): Red runs — AC-01 / AC-02 at the wasm level failed against a `strokeToBatch` stub that painted nothing. Green on the first run, kept as pins: the Image core test (the core already joins split strokes byte-identically, which the UI batching relies on), the AC-02 e2e test (Pixel mode already iterated coalesced samples) and AC-03 (both canvas stages already carried `touch-action: none`).
- 2026-11-05 (spec-build): Measurements on the reference machine, release wasm, `pnpm perf`, back to back. `pixel-pencil-256-z32`: frame p95 16.7 → 16.7 / 16.8 ms, dab latency p50 / p95 23.8 / 31.6 → 24.3 / 31.4 and 23.1 / 31.3 ms. `image-brush-4k`: dab latency p50 / p95 before 23.2 / 31.4, 25.1 / 32.5, 23.5 / 31.4 ms; after 24.3 / 45.1 (one 83 ms frame, also seen before), 23.9 / 32.1, 23.8 / 31.9 ms — no change beyond noise. The scenarios send one pointer event per frame, so batching has nothing to merge there; it pays off with pens and high-rate mice that deliver several samples per frame.
- 2026-11-05 (owner): AC-04's 8 ms dab-latency budget stays, measured from the pointer event to the draw call — the work Pincel controls — not to the next frame that starts after the draw. Settles the Open question of 2026-11-05 (the old definition measured p95 ≈ 31 ms, unreachable by any app change).
- 2026-11-05 (spec-build): The browser performance script starts the draw latency when the page receives the event (`performance.now()` in the probe's first capture listener), not at the event's `timeStamp`: Chrome dispatches `pointermove` aligned to the frame, and in the Pixel scenario that queueing alone measured p50 7.9 / p95 16.1 ms (timeStamp → dispatch), while the app's own work (dispatch → draw) measured p50 0.6 / p95 0.9 ms. Neither the dispatch wait nor the wait for the presenting frame is the app's work. The metric is `dab-draw-latency-p50` / `-p95` beside the unchanged `dab-latency` (timeStamp → presented), which the Image brush budget of `performance-image-cache` keeps; `ui/e2e-perf/budgets.json` checks the Pixel budget against `pixel-pencil-256-z32/dab-draw-latency-p95`.
- 2026-11-05 (spec-build): Measured on the reference machine (release wasm, `pnpm perf -g pixel`, three runs): `dab-draw-latency` p50 / p95 0.6 / 0.9, 0.6 / 0.9, 0.5 / 0.9 ms; frame p95 16.7 / 16.8 / 16.8 ms. Both AC-04 budgets are met on this machine; the budget check recorded `dab-draw-latency-p95` in `ui/e2e-perf/budgets-met.json`. AC-04 stays open until the manual check on the reference laptop is entered.
- 2026-10-29: The predicted pointer tip (`getPredictedEvents()`) is not part of this spec but a follow-up, after the dab latency is measured — the plan marks it optional, and it needs its own overlay design (owner accepted the recommendation).
