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

- AC-03 (dab latency p95 ≤ 16 ms) may not be reachable by this spec as measured: in the dev container the p95 is 32.1 ms and the p50 is ≈ 26.5 ms both at fit zoom and at 100 %, where presenting is only a crop of the cache — so what is left looks like pointer event → handler → next frame alignment (the probe counts until the first frame that starts after the draw), not compose or presentation work. Is AC-03 met on the reference laptop, or does the remaining latency move to `performance-input-path` (batching per rAF, `getPredictedEvents`), with AC-03 reworded to the work this spec owns (e.g. the AC-02 bench per dab)? (spec-build, 2026-11-03)

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `region_at_zoom_one_matches_a_fresh_compose_after_any_edits` (proptest, `crates/pincel-image-core/src/render/cache.rs`); `composite_region_matches_a_fresh_compose_after_edits_undo_and_redo` (`crates/pincel-image-wasm/src/lib.rs`) | built |        |
| AC-02 | `cargo bench -p pincel-image-core --bench compose -- image_cache` — `image_cache/brush_100_dabs_fit_4k_10layers` in `crates/pincel-image-core/benches/compose.rs`: median 80.2 ms (dev container, native release, float compose path) against the 1.6 s budget | built |        |
| AC-03 | `pnpm perf -g "brush, pan"` in `ui/` (`ui/e2e-perf/scenarios.perf.ts`, `image-brush-4k`, `dab-latency-p95`): 32.1 ms in the dev container (before: 728.6 ms) — over the 16 ms budget, see Open questions; manual sign-off on the reference laptop pending | built |        |
| AC-04 | `pnpm perf -g "brush, pan"` in `ui/` (`image-pan-zoom-4k`, `frame-p95`): 16.8 ms in the dev container (before: 350 ms), i.e. every frame at the 60 Hz vsync; manual sign-off on the reference laptop pending | built |        |
| AC-05 | `byte_size_stays_within_the_budget_and_keeps_every_level` (`crates/pincel-image-core/src/render/cache.rs`) | built |        |
| AC-06 | `region_after_canvas_size_changes_matches_compose`, `region_detects_a_size_change_it_was_not_told_about` (`crates/pincel-image-core/src/render/cache.rs`); the resize step of `composite_region_matches_a_fresh_compose_after_edits_undo_and_redo` (`crates/pincel-image-wasm/src/lib.rs`) | built |        |
| AC-07 | `region_below_zoom_one_matches_a_fresh_pyramid_after_any_edits` (proptest), `blend4_matches_the_premultiplied_average`, `halve_matches_the_premultiplied_box_average` (`crates/pincel-image-core/src/render/cache.rs`); `composite_region_below_zoom_one_samples_the_pyramid_of_a_fresh_compose` (`crates/pincel-image-wasm/src/lib.rs`) | built |        |

## Decisions

- The input path never waits for heavy work: paint into the cache and present; anything slower than a frame goes elsewhere (plan, 2026-10-28).
- The cache lives in the core per document, not in the UI — document state stays in Rust (CLAUDE.md §9, pincel.md §17.1).
- 2026-10-29: Zoom ≥ 1 stays byte-identical to a fresh compose; the zoom < 1 display may differ from today's bilinear downscale and must equal sampling the same box-filtered pyramid built from a fresh compose — display downscaling is not document content, and mips look better than single-step bilinear, which aliases on strong zoom-out (owner accepted the recommendation).
- 2026-10-29: Downscaled levels are never skipped by default (≈ 340 MB at 64 Mpx); only if the `performance-benchmarks` memory scenario fails are mips skipped above 32 Mpx, with zoom < 1 then bilinear-downscaled from the cached full-resolution composite — measure first, and the fallback still avoids recomposing every layer (owner accepted the recommendation).
- 2026-11-03 (spec-build): `performance-image-kernel` is not in this branch (its spec is still `todo`, `render/mod.rs` still runs the float path). The cache only calls `compose_region`, so it picks the kernel up unchanged once it lands; every number below is measured on the float path.
- 2026-11-03 (spec-build): Commands report what they change: `Command::damage` (default `Damage::Full`, always correct) — `SetPixels` / `MoveSelection` their edit rects, selection, rename, lock and a new (transparent) layer nothing — and `CommandBus` collects it until `take_damage`. The alternative, diffing layers, would need a copy of every layer; the default keeps any command that does not override it correct.
- 2026-11-03 (spec-build): The cache is `pincel_image_core::RenderCache` (new public API: `new`, `update`, `region`, `composite`, `size`, `level_count`, `byte_size`; plus `command::Damage`, `Command::damage`, `CommandBus::take_damage`). `pincel-image-wasm` keeps one per handle next to the bus and serves `composite_region` from it; the UI is unchanged.
- 2026-11-03 (spec-build): Levels are tiled (128 px of their own level) and built lazily: a damaged rect is recomposed at once only inside full-resolution tiles that are already valid, and the pyramid tiles above it go stale; `Damage::Full` (layer properties, moves, canvas changes) marks everything stale, and the next presentation composes only the tiles it shows. The cache also rebuilds itself when the canvas size differs from the document's, whatever damage it was given.
- 2026-11-03 (spec-build): Pyramid level `l` is `ceil(w / 2^l) × ceil(h / 2^l)`, the 2×2 box filter of level `l - 1` in premultiplied alpha with integer rounding (edge pixels average what exists). Zoom < 1 picks the largest level whose scale `2^l` is at most the reduction `min(w / out_w, h / out_h)` and samples it bilinearly (8-bit fixed-point weights, premultiplied, integer rounding), so zoom in (½, 1) samples the full-resolution level. Integer math makes the incremental and the fresh pyramid byte-identical by construction.
- 2026-11-03 (spec-build): AC-05 counts every byte the cache holds (pixels and tile flags). Levels are added only while the total stays within `1.34 × w × h × 4`; for real documents that is every level down to 1×1 (checked up to 8192 × 8192), only tiny canvases (e.g. 3 × 3) stop early because of rounding up.
- 2026-11-03 (spec-build): No stroke cache of the layers above the active one. Pre-blending them into one buffer cannot be byte-identical (blend modes and per-layer rounding are not associative), which AC-01 requires; and a full-size `below` buffer would add another `w × h × 4` bytes on top of the 1.33 the pyramid takes, past AC-05's 1.34. Each dab instead recomposes all layers in its own rect, which meets AC-02 with a wide margin (see Evidence); revisit only if AC-03 misses for big brushes.
- 2026-11-03 (spec-build): The memory fallback (no levels above 32 Mpx) is not built. `image-memory-4k` misses its 1 GB budget already without this cache (2318 MiB, undo memory, owned by `performance-image-undo-memory`), and at 4K it could not trigger the fallback anyway; the cache adds ≈ 44 MB there.
- 2026-11-03 (spec-build): AC-02's bench (`image_cache/brush_100_dabs_fit_4k_10layers`) opens and presents the document at fit zoom once outside the measurement, then per dab: the Pencil stroke through a `CommandBus`, `take_damage`, `RenderCache::update` and presenting the dab's changed rect at ½ scale. Measured in the dev container (4 shared cores, native release, float compose path, 2026-11-03): 80.2 ms median for 100 dabs. Before, presenting at fit zoom recomposed the whole visible canvas per frame: `image_region/fit_4k_10layers` 8.37 s per presentation (`image_stroke/brush_100_dabs_4k_10layers`, the dabs alone, 1.15 ms). Presenting the whole canvas into a 1920×1080 viewport from a current cache (`image_cache/fit_viewport_4k_10layers`, pan / zoom without edits): 10.3 ms.
- 2026-11-03 (spec-build): Browser script, dev container (Xeon 2.1 GHz, 4 threads shared with other jobs, Chromium 153, release wasm, float compose path), before (`composite_region` recomposing the visible rect) and after (the cache) built and run alternately in one session: `image-brush-4k` dab latency p50 / p95 127 / 728.6 ms → 26.6 / 32.1 ms (three after-runs: p95 32.1, 36.1, 32.1), frame p95 600 → 16.8 ms; `image-pan-zoom-4k` frame p95 350 → 16.8 ms (frame max 1366.7 → 33.4 ms); `image-recompose-4k` recompose p50 333 → 411 ms — a full recompose now also rebuilds the pyramid; that budget belongs to `performance-image-kernel`. A local split of the brush scenario (not committed) gave the same dab latency p50 at fit zoom (26.8 ms) as at 100 % (26.5 ms), where presenting is a plain crop.
- 2026-11-03 (spec-build): `composite_region_matches_a_fresh_compose_after_edits_undo_and_redo` was green before the wasm change — `composite_region` recomposed on every call, so exactness at zoom ≥ 1 already held; its red run is the core proptest with the damage handling disabled (both proptests fail). The zoom < 1 wasm test was red (bilinear `scale` ≠ pyramid sample).
