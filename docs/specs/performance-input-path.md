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

This spec owns the Pixel budget from `performance-benchmarks`: a pencil drag on a 256×256 sprite at zoom 32 runs at 60 fps with a dab latency of at most 8 ms.

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
- [ ] AC-04 (state, manual: the budget is defined for the reference laptop): While the user drags the Pixel pencil on a 256×256 sprite at zoom 32, the browser performance script measures a p95 frame time of at most 16.7 ms and a p95 dab latency of at most 8 ms.

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
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |

## Decisions

- The input path never waits for heavy work: samples are batched per animation frame and presented once (plan, 2026-10-28).
- 2026-10-29: `pixel-pencil` is built first and this spec feeds its `strokeTo` with all coalesced samples once per animation frame, instead of coalescing on today's `applyTool` / `applyLine` — the plan's design uses `strokeTo`, and `shading-ink` and `symmetry-tiled-mode` build on the same stroke API, so nothing gets wired twice (owner accepted the recommendation).
- 2026-10-29: The predicted pointer tip (`getPredictedEvents()`) is not part of this spec but a follow-up, after the dab latency is measured — the plan marks it optional, and it needs its own overlay design (owner accepted the recommendation).
