---
status: todo
created: 2026-10-28
size: M
mode: Image
design: [pincel.md §4.1, pincel.md §4.2, pincel.md §15 (2026-10-27 HSL blend modes)]
depends_on: [performance-benchmarks]
---

# Performance: Image compositing kernel

## Context

Image-mode `compose()` is the biggest single cost in the editor: 101 ms for 1024×1024 with 2 layers, 1 262 ms for 4096×4096 with 2 layers (native release, 2026-10-28). Reading the code showed why: the layer blend converts source and backdrop from sRGB to linear light and back with `powf` — nine `powf` calls per pixel per layer — and reads and writes each pixel through bounds-checked `Option<Color>` accessors with no fast paths. A quick prototype with lookup tables alone blended a 2048² layer in 28 ms instead of 199 ms (≈ 7×).

This spec, step 1 of the legacy plan `23-performance` (removed 2026-10-29, see git history), makes the kernel at least 10× faster **without changing a single output byte** (or at most 1 LSB where documented). The plan calls it the biggest single win; it can start right after `performance-benchmarks`.

## Scope

### In scope

- Faster Image-mode compositing for all blend modes, opacities and layer offsets
- Proof of exactness against today's output (snapshot tests plus a randomized old-versus-new equivalence test)
- Proof of the speed-up on the `performance-benchmarks` compose bench

### Out of scope

- Changing the color pipeline (blending in linear light) — because it is the existing `compose()` contract and the reference output this spec must reproduce
- The composite cache, dirty rects and downscaled presentation — because `performance-image-cache` covers them
- wasm SIMD and release-profile tuning — because `performance-build-simd` covers them
- Pixel-mode compose — because it already meets its compose numbers (M12 baselines); its hot spot is input (`performance-input-path`)

## Acceptance criteria

- [ ] AC-01 (always): For every blend mode, layer opacity, layer visibility, layer offset and pixel alpha, the new compose output is byte-identical to the compose output before this change, or differs by at most 1 per channel where `## Decisions` documents that case.
- [ ] AC-02 (always): The existing Image compose snapshot tests pass without updating any snapshot.
- [ ] AC-03 (event): When the `pincel-image-core` compose bench runs on the same machine before and after this change, then the full-compose median of each scenario (1024², 2048², 4096² × 2, 5, 10 layers) is at least 10× lower after it.
- [ ] AC-04 (always): Hidden layers and layers with opacity 0 leave the output unchanged, as today.

## Technical notes

Hints from the legacy plan (2026-10-28); line numbers drift.

- `crates/pincel-image-core/src/render/mod.rs`: `srgb_to_linear` / `linear_to_srgb` (≈L12–25, `powf`), `compose` (≈L35), `composite_layer` (≈L116–170). Blend functions in `render/blend.rs` (`blend_channel`, 11 modes besides Normal, plus the HSL modes per Decision Log 2026-10-27).
- Plan's approach: a 256-entry `f32` sRGB → linear table; a 4096- or 8192-entry `u8` linear → sRGB table, size chosen by the exactness test; tables built once (`std::sync::OnceLock` or `const fn`). Row-slice access on `ImageBuffer` (`row(y) -> &[u8]`, `row_mut`), no per-pixel `Option`. Fast paths: Normal with source alpha 1 → copy; source alpha 0 → skip; backdrop alpha 0 → straight copy with opacity; optionally skip fully transparent rows (measure it). Keep `blend_channel` for the other modes on LUT-converted values.
- Write the kernel so it can auto-vectorize later (`performance-build-simd`).
- Tests: keep `crates/pincel-image-core/tests/compose_snapshots.rs`; add a proptest equivalence test (`compose_proptest.rs` exists) that keeps the old float path as the reference.
- Each performance commit message states before → after on the bench.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Layer partly or fully outside the canvas (offset) | Only the overlap is blended, byte-identical to today |
| 1×1 canvas | Byte-identical to today |
| Opacity 1/255 or 254/255 | Byte-identical to today (or ≤ 1 per channel, documented) |
| Fully transparent layer | Output unchanged |
| Non-separable HSL modes (Hue, Saturation, Color, Luminosity) | Byte-identical to today |
| Source pixel with alpha 255 over transparent backdrop | Byte-identical to today |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Linear-light blending is the existing behavior to preserve | The `compose()` doc comment says so; the plan cites a Decision Log entry of 2026-10-27 for it, which §15 does not contain (its 2026-10-27 compose entry covers the HSL modes) | Low |
| "10×" is measured natively with criterion, on one machine before and after | The plan: "verified by the step 0 bench" | Low |

## Open questions

<!-- none -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |

## Decisions

- Exactness first: today's output defines correctness; a faster path must produce the same bytes, or at most 1 LSB per channel where documented here (plan, 2026-10-28).
- Target ≥ 10× on full compose, measured by the `performance-benchmarks` compose bench (plan).
- The size of the linear → sRGB table (4096 or 8192 entries) is chosen by the exactness test: the smallest table that passes AC-01 (plan).
