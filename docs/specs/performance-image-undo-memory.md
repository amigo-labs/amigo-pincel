---
status: todo
created: 2026-10-28
size: L
mode: Image
design: [pincel.md §6.2, pincel.md §6.3]
depends_on: [performance-benchmarks]
---

# Performance: memory-efficient undo for large images

## Context

Image-mode undo for structural commands — paste, merge, flatten, scale, crop — clones the whole document. On a 3840×2160 document with 10 layers that is about 330 MB per undo step, so a few steps exhaust the 4 GB wasm32 memory. Step 7 of the legacy plan `23-performance` (removed 2026-10-29, see git history) stores only the parts of the image a command changed, keeping the whole-document snapshot only where the canvas size changes. It owns the budget "≤ 1 GB for a 4K, 10-layer document with 50 undo steps" from `performance-benchmarks`.

## Scope

### In scope

- Undo storage for pixel-changing Image commands that grows with the changed area, not with the document
- Unchanged undo / redo behavior for every Image command
- The memory budget as a measured scenario

### Out of scope

- Commands that change the canvas size — because they keep the whole-document snapshot (plan)
- Pixel-mode undo — because Pixel commands already store pixel diffs (pincel.md §6.3) on small sprites
- Images beyond 64 Mpx — because wasm32's 4 GB limit (`MAX_CANVAS_AREA`) stays; larger images are out of scope until someone needs them

## Acceptance criteria

- [ ] AC-01 (always): After any random sequence of Image commands (strokes, fills, effects, paste, merge, flatten, layer operations, scale, crop, resize), undoing every step restores the original document byte-identically, and redoing every step restores the final document byte-identically.
- [ ] AC-02 (always): The undo memory added by a command that changes pixels only inside a rect of one layer is at most the RGBA size of that rect rounded up to the tile grid recorded under `## Decisions`.
- [ ] AC-03 (state): While a 3840×2160, 10-layer document holds 50 undo steps — 40 brush strokes (each within 512 px), 5 whole-layer adjustments or effects, 3 merge-downs and 2 pastes of 1024×1024 without canvas expansion — the browser's wasm memory (`WebAssembly.Memory` `buffer.byteLength`) is at most 1 GB, and a native test runs the same sequence and asserts that the undo stack's reported byte count equals the bytes it actually stores.
- [ ] AC-04 (event): When a command that changes the canvas size is undone, then size, layers and pixels equal the state before it, as today.
- [ ] AC-05 (event): When a new command pushes the stored undo data of a document above 512 MB, then the oldest undo steps are dropped (and, if that is not enough, redo steps) until it is at most 512 MB; the step just performed is never dropped, and undo of every remaining step still restores its state byte-identically.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- `DocSnapshot` in `crates/pincel-image-core/src/command/snapshot.rs` (≈L13, `snapshot()` ≈L21), used by paste (`paste.rs`), merge (`merge.rs`), flatten, scale and crop (`resize.rs`, `transform.rs`).
- Plan's approach: tile-based copy-on-write undo (e.g. 128×128 tiles: store only the tiles a command changed) for pixel commands; keep the snapshot only where the canvas size changes.
- The memory scenario (mix and measurement of AC-03) is in the `performance-benchmarks` budget table; its numbers go into that spec's `## Measurements` section. wasm memory never shrinks, so `buffer.byteLength` is the number that hits the 4 GB wall.
- History cap: entries and bytes. The byte cap (512 MB of stored undo data, AC-05) is checked after each push.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Merge down (two layers change) | Undo restores both layers byte-identically |
| Flatten (all layers replaced by one) | Undo restores every layer and its properties |
| Paste with "Expand canvas" (size changes) | Whole-document snapshot as today; undo restores size and pixels |
| A dab touching four tiles at a tile corner | Only those tiles are stored |
| A command that changes no pixel | Adds no tile data |
| One command alone stores more than 512 MB (e.g. flatten of a huge document) | It is kept as the only step; all older undo and redo steps are dropped (AC-05) |
| History over 512 MB after a push, redo stack non-empty | Oldest undo steps go first, then redo steps, until under the cap |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `crates/pincel-image-core/tests/undo_round_trip_proptest.rs` › `undo_all_then_redo_all_is_byte_identical` (proptest: strokes, fills, whole-layer adjustments, paste with and without expand, merge down, flatten, add / remove / duplicate / move layer, opacity, selection, crop, scale, resize; undo all equals the original `Document`, redo all the final one) | built |        |
| AC-02 | `crates/pincel-image-core/src/command/pixels.rs` › `undo_bytes_stay_within_the_tile_rounded_rect`, `merged_stroke_stores_each_tile_once`, `edit_that_changes_nothing_stores_no_tiles`; `crates/pincel-image-core/src/command/tiles.rs` › `write_at_tile_corner_saves_four_tiles`, `write_clips_edge_tiles_to_the_layer`, `write_without_change_saves_nothing` | built |        |
| AC-03 | Native: `crates/pincel-image-core/tests/undo_memory_budget.rs` › `budget_session_reports_its_undo_bytes_and_fits_the_budget` (50 steps; `stored_bytes() == recount_bytes()`; document 284 MiB + undo 273 MiB). Browser: `pnpm perf` `image-memory-4k` `wasm-memory-mb` 874.69 MiB (two runs, 2026-11-05, reference machine, release wasm; 2362.19 MiB before) | built |        |
| AC-04 | `crates/pincel-image-core/tests/undo_round_trip_proptest.rs` › `undo_of_expanding_paste_restores_size_and_pixels`, `undo_of_canvas_size_changes_restores_the_document` | built |        |
| AC-05 | `crates/pincel-image-core/src/command/mod.rs` › `byte_cap_drops_oldest_steps_and_keeps_the_rest_exact`, `byte_cap_keeps_a_step_larger_than_the_cap_alone`, `stored_bytes_follow_undo_redo_and_new_pushes` | built |        |

## Decisions

- Commands that change the canvas size keep the whole-document snapshot; only pixel commands move to stored changed regions (plan, 2026-10-28).
- 2026-10-29: The 50-step budget mix is 40 brush strokes (≤ 512 px each), 5 whole-layer adjustments or effects, 3 merge-downs and 2 pastes of 1024² (no expand) on 3840×2160 × 10 layers, measured as the browser's `WebAssembly.Memory` `buffer.byteLength` plus a native test of the undo stack's byte accounting — that is a realistic session, and wasm memory is the number that hits the 4 GB wall because it never shrinks (owner accepted the recommendation).
- 2026-11-05 (spec-build): The undo tile grid is 64 × 64 pixels, starting at the layer's top-left corner, edge tiles clipped to the layer (`pincel_image_core::command::UNDO_TILE`). A pixel edit saves a tile only where its new pixels differ, and saves each tile once per command, so AC-02's bound is the rect rounded out to this grid.
- 2026-11-05 (spec-build): Undo storage swaps instead of copying twice: `SetPixels` writes its new pixels on the first apply and keeps only the tiles that are not in the document — the old ones while applied, the new ones while undone — so apply and revert are the same tile swap. Merged stroke segments add only tiles the stroke has not saved yet. `MergeDown` keeps the removed upper layer and the lower layer's changed tiles; `PasteLayer` without canvas growth keeps only the active index (undo takes the layer back out); `AddLayer`, `DuplicateLayer` and `PasteLayer` hold their layer only while undone; `FlattenImage` moves the old layers into its snapshot instead of copying them. Canvas-size changes (scale, crop, resize, expanding paste) keep the whole-document snapshot (AC-04). Merge Group, Merge Visible and Flatten replace several layers and keep their snapshot.
- 2026-11-05 (spec-build): New public API in `pincel-image-core`: `Command::undo_bytes` (default 0; pixel and selection-mask bytes a command holds), `UndoStack::stored_bytes` (tracked on push, merge, undo, redo and drop), `UndoStack::recount_bytes` (the same sum recomputed from the commands — AC-03's "bytes it actually stores"), `UndoStack::set_byte_cap`, `DEFAULT_UNDO_BYTE_CAP` (512 MiB), `command::UNDO_TILE`. `command::PixelEdit` is gone (no user outside `SetPixels`). `pincel-agent`'s batch command sums its parts.
- 2026-11-05 (spec-build): The byte cap counts mebibytes (512 × 2²⁰ bytes) and is checked after every push. A push discards the redo branch first (spec §7.2), so after a push only undo steps are left to drop; the newest step is never dropped.
- 2026-11-05 (spec-build): Measured on the reference machine, back to back: `pnpm perf` `image-memory-4k` wasm memory 2362.19 → 874.69 MiB (128-px tiles: 881.94 MiB), longest frame 1050 → 567 ms, `image-brush-4k` dab latency p50 / p95 ≈ 23.5 / 32 ms before and after. `cargo bench` `image_stroke/brush_100_dabs_4k_10layers` 1.75 ms before, 2.15 / 1.78 ms after (two runs; the machine was shared, run-to-run spread ≈ 20 %); `image_cache/brush_100_dabs_fit_4k_10layers` 45.2 → 44.9 / 35.9 ms. 64-px tiles copy less per dab than 128-px ones (a 20-px dab saves 1–4 tiles of 16 KiB).
- 2026-11-05 (spec-build): AC-01's proptest found that redo of Flatten Image and Merge Visible gave the new layer a fresh id; both now keep the id from their first apply, so redo restores the document byte-identically.
- 2026-11-05 (spec-build): The native AC-03 test stands in a flat fill for the five whole-layer adjustments (same undo data, fast in a debug build); it takes ≈ 10 s in a debug build, most of it the three 4K merge-down composites. Red runs: the AC-02 / AC-03 / AC-05 tests did not compile before (no byte accounting existed); AC-01's proptest failed on Flatten until the id fix.
- 2026-10-29: The history is capped by bytes as well as entries: above 512 MB of stored undo data the oldest steps (then redo steps) are dropped, never the current step — paint.net and Krita use a memory-limited history, and running out of memory is worse than losing old undo steps (owner accepted the recommendation).
