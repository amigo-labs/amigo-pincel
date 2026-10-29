---
status: todo
created: 2026-10-28
size: L
mode: Image
design: [pincel.md §4.3, pincel.md §4.4, pincel.md §6]
depends_on: [performance-benchmarks, performance-image-kernel]
---

# Performance: Image composite cache and dirty rects

## Context

Even with a fast kernel (`performance-image-kernel`), Image mode re-blends **every layer** on every redraw, although a brush dab only changes the active layer inside a small rect. And at "fit" zoom on a large image, `composite_region` composes the visible rect at full resolution and downscales afterwards, so every dab recomposes the whole image.

This spec, step 2 of the legacy plan `23-performance` (removed 2026-10-29, see git history), keeps a cached composite that is updated only where something changed, keeps the layers below and above the active one pre-blended during a stroke (the paint.net / Krita approach), and presents zoomed-out views from a downscaled pyramid of the cache. It owns the budgets "brush dab → pixels on screen ≤ 16 ms" and "pan / zoom at 60 fps" for a 3840×2160 document with 10 layers.

## Scope

### In scope

- A cached full-resolution composite per Image document, updated in the changed rect after each command or dab
- A stroke cache: the layers below and above the active layer blended once per stroke
- Downscaled presentation for zoom < 1 from a pyramid of the cached composite, updated in dirty rects
- A documented memory budget for the cache

### Out of scope

- Pixel mode — because it already has dirty-rect compose (M12.2) and meets its compose numbers
- Moving compose off the main thread — because `performance-background-jobs` covers heavy one-shot jobs; interactive painting stays on the main thread
- GPU compositing — because it would be a large architecture change (pincel.md §4); stop and ask only if the CPU paths miss the 4K budgets

## Acceptance criteria

- [ ] AC-01 (always): After any sequence of brush dabs, commands, undo, redo, layer moves and layer property changes (visibility, opacity, blend mode), the composite the editor presents at zoom ≥ 1 is byte-identical to a fresh full compose of the document.
- [ ] AC-02 (event): When the `pincel-image-core` bench runs a 100-dab brush stroke on a 3840×2160, 10-layer document including the presentation of each dab's changed region at fit zoom, then the median is at most 1.6 s (16 ms per dab).
- [ ] AC-03 (state, manual: the budget is defined for the reference laptop): While the user paints with the Brush on a 3840×2160, 10-layer document at any zoom, the browser performance script measures a p95 dab latency (pointer event → pixels on screen) of at most 16 ms.
- [ ] AC-04 (state, manual: the budget is defined for the reference laptop): While the user pans or zooms a 3840×2160, 10-layer document, the browser performance script measures a p95 frame time of at most 16.7 ms (60 fps).
- [ ] AC-05 (always): The cache of a document, including its downscaled levels, takes at most 1.34 × width × height × 4 bytes; downscaled levels are kept for every document size unless the memory scenario of `performance-benchmarks` misses its budget (see Edge cases for the fallback then).
- [ ] AC-06 (event): When a document is opened, or a command changes the canvas size (resize, scale, crop, rotate), then the next presented image equals a fresh full compose of the new document.
- [ ] AC-07 (always): After any sequence of brush dabs, commands, undo, redo, layer moves and layer property changes, the image the editor presents at zoom < 1 equals sampling the same downscaled pyramid built from a fresh full compose of the document. It need not equal today's "compose at full resolution, then bilinear downscale" output.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- `composite_region` in `crates/pincel-image-wasm/src/lib.rs` (≈L1534) composes the visible rect at full resolution, then downscales; its test `composite_region_crops_and_downscales` (≈L1851). `compose` in `crates/pincel-image-core/src/render/mod.rs`.
- Stroke cache: when a stroke starts on layer *k*, compose layers `0..k` into `below` and `k+1..` into `above` once; each dab blends `below` + active layer + `above` inside the dab rect.
- Mip pyramid (½, ¼, …) of the cached composite, updated incrementally in dirty rects; `composite_region` at zoom < 1 samples the matching level.
- Memory: composite + mips ≈ 1.33 × W × H × 4 bytes; against `MAX_CANVAS_AREA` (8192 × 8192 = 64 Mpx, `ui/src/modes/image/stores/editor.svelte.ts`) the worst case is ≈ 340 MB. No mips are skipped by default; a threshold is added only if the memory scenario in `performance-benchmarks` fails — then mips are skipped above 32 Mpx.
- Tests: proptest — the cache matches a fresh full compose after random command sequences including undo, redo and layer moves (AC-01).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Stroke on the bottom layer (nothing below) or top layer (nothing above) | Same result as a fresh full compose |
| Undo in the middle of a sequence of strokes | Presented image equals a fresh full compose of the undone state |
| Active layer hidden or locked while painting is attempted | No change to the presented image |
| Document at `MAX_CANVAS_AREA` | Cache fits the memory bound of AC-05 (≈ 340 MB), all levels kept |
| Memory budget of `performance-benchmarks` fails, document above 32 Mpx | No downscaled levels are kept; zoom < 1 is a bilinear downscale of the cached full-resolution composite (today's look, no recompose of the layers) |
| Zoom exactly 1 | Presented from the full-resolution cache, no pyramid level |

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

## Decisions

- The input path never waits for heavy work: paint into the cache and present; anything slower than a frame goes elsewhere (plan, 2026-10-28).
- The cache lives in the core per document, not in the UI — document state stays in Rust (CLAUDE.md §9, pincel.md §17.1).
- 2026-10-29: Zoom ≥ 1 stays byte-identical to a fresh compose; the zoom < 1 display may differ from today's bilinear downscale and must equal sampling the same box-filtered pyramid built from a fresh compose — display downscaling is not document content, and mips look better than single-step bilinear, which aliases on strong zoom-out (owner accepted the recommendation).
- 2026-10-29: Downscaled levels are never skipped by default (≈ 340 MB at 64 Mpx); only if the `performance-benchmarks` memory scenario fails are mips skipped above 32 Mpx, with zoom < 1 then bilinear-downscaled from the cached full-resolution composite — measure first, and the fallback still avoids recomposing every layer (owner accepted the recommendation).
