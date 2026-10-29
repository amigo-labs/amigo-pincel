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
- [ ] AC-03 (state): While a 3840×2160, 10-layer document holds 50 undo steps of the command mix recorded under `## Decisions`, its layers plus undo history take at most 1 GB.
- [ ] AC-04 (event): When a command that changes the canvas size is undone, then size, layers and pixels equal the state before it, as today.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- `DocSnapshot` in `crates/pincel-image-core/src/command/snapshot.rs` (≈L13, `snapshot()` ≈L21), used by paste (`paste.rs`), merge (`merge.rs`), flatten, scale and crop (`resize.rs`, `transform.rs`).
- Plan's approach: tile-based copy-on-write undo (e.g. 128×128 tiles: store only the tiles a command changed) for pixel commands; keep the snapshot only where the canvas size changes.
- Add the memory scenario to the `performance-benchmarks` table and its `## Measurements` section.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Merge down (two layers change) | Undo restores both layers byte-identically |
| Flatten (all layers replaced by one) | Undo restores every layer and its properties |
| Paste with "Expand canvas" (size changes) | Whole-document snapshot as today; undo restores size and pixels |
| A dab touching four tiles at a tile corner | Only those tiles are stored |
| A command that changes no pixel | Adds no tile data |

## Open questions

- Which command mix do the 50 undo steps of the memory budget consist of (e.g. brush strokes only, or including merges and pastes), and is the 1 GB measured as Rust-side buffer bytes or as the browser's wasm memory?
- What happens when the budget would be exceeded — drop the oldest undo steps, cap the history, or nothing?

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |

## Decisions

- Commands that change the canvas size keep the whole-document snapshot; only pixel commands move to stored changed regions (plan, 2026-10-28).
