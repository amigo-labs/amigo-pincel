---
status: implemented
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

- [x] AC-01 (always): For every blend mode, layer opacity, layer visibility, layer offset and pixel alpha, the new compose output is byte-identical to the compose output before this change, or differs by at most 1 per channel where `## Decisions` documents that case.
- [x] AC-02 (always): The existing Image compose snapshot tests pass without updating any snapshot.
- [x] AC-03 (event): When the `pincel-image-core` compose bench runs on the same machine before and after this change, then the full-compose median of each scenario (1024², 2048², 4096² × 2, 5, 10 layers) is at least 10× lower after it.
- [x] AC-04 (always): Hidden layers and layers with opacity 0 leave the output unchanged, as today.

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

## Summary

> Completed: 2026-11-05

- Image-mode compose no longer calls `powf`: sRGB → linear and `a / 255` are 256-entry tables, linear → sRGB is a threshold table read with one comparison
- Byte-identical to the old float path for every blend mode, opacity, offset and alpha (proptest against the kept float reference); compose snapshots unchanged
- Monomorphized blend per layer, integer alpha rounding and an exhaustively checked copy path for opaque Normal sources
- 10.6× – 19.4× faster full compose on the reference machine (1024² – 4096², 2 – 10 layers)

## Open questions

<!-- none -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `render::tests::fast_kernel_is_byte_identical_to_the_float_path`, `render::tests::every_mode_opacity_and_alpha_pair_matches_the_float_path`, `render::tests::encoder_matches_the_float_conversion_around_every_threshold`, `render::tests::decode_table_matches_the_float_conversion_for_every_byte`, `render::tests::round_byte_matches_round_for_every_float_up_to_255`, `render::tests::an_opaque_normal_source_takes_the_copy_path` (`crates/pincel-image-core/src/render/mod.rs`) — passed 2026-11-05 | verified | ba53299 |
| AC-02 | `compose_normal_over_gradient_snapshot`, `compose_multiply_and_screen_stack_snapshot` (`crates/pincel-image-core/tests/compose_snapshots.rs`) — passed 2026-11-05; snapshots last changed 2026-10-11 (`4b0ce16`, rename), before the kernel | verified | ba53299 |
| AC-03 | `cargo bench -p pincel-image-core --bench compose -- image_compose`, `ba53299^` (`--save-baseline before`) vs. `HEAD` on the reference machine, 2026-11-05: every scenario ≥ 10× (lowest 4096² × 5 layers, 8 508 → 801 ms = 10.6×); numbers under Decisions | verified | f6e5d0e |
| AC-04 | `render::tests::hidden_layer_is_skipped`, `render::tests::zero_opacity_layer_leaves_the_output_unchanged` (`crates/pincel-image-core/src/render/mod.rs`), also in the AC-01 proptest — passed 2026-11-05 | verified | ba53299 |

## Decisions

- Exactness first: today's output defines correctness; a faster path must produce the same bytes, or at most 1 LSB per channel where documented here (plan, 2026-10-28).
- Target ≥ 10× on full compose, measured by the `performance-benchmarks` compose bench (plan).
- The size of the linear → sRGB table (4096 or 8192 entries) is chosen by the exactness test: the smallest table that passes AC-01 (plan).
- 2026-11-03 (spec-build) — Exact without a wider tolerance: AC-01 holds byte for byte, so no 1-LSB case is documented. sRGB → linear and `a / 255` are 256-entry tables of the very values the float path computed. Linear → sRGB is not a table of rounded values but the 255 thresholds where the float path's rounding steps up (binary search over the float bit patterns, which order like the floats for non-negative values); the byte is the number of thresholds at or below the value. Buckets of 2¹⁶ neighbouring floats hold at most one threshold each (2¹⁷ does not; the build asserts it), so encoding is one table read and one comparison. Exactness of the encoder rests on the float path being monotonic between thresholds: the test checks ±4 096 floats around every threshold, both ends of every bucket and a million spread values.
- 2026-11-03 (spec-build) — Further exact shortcuts: the blend mode is monomorphized per layer; `co / ao` is skipped when `ao == 1.0`; the alpha byte rounds with integer arithmetic instead of `f32::round`, which is a `roundf` call on baseline x86-64 (checked against `round` on every 7th float up to 255); an opaque Normal source is copied, enabled only after an exhaustive check that all 65 536 (source byte, backdrop alpha) pairs give the source byte back. The old float path stays in the unit tests as the reference.
- 2026-11-03 (spec-build) — Red runs: with the encoder comparing `>` instead of `>=` at a threshold, and with the backdrop decoded one byte off, the equivalence tests failed.
- 2026-11-03 (spec-build) — AC-03 measured in this container (4-core Intel Xeon @ 2.10 GHz, Linux), `cargo bench -p pincel-image-core --bench compose -- image_compose`, criterion medians before → after: 1024² × 2 / 5 / 10 layers 175.6 / 507.5 / 1 111 ms → 14.3 / 51.6 / 107.6 ms; 2048² 738 / 2 081 / 5 478 ms → 50.6 / 160.4 / 458.5 ms; 4096² 3 501 / 10 050 / 17 893 ms → 239 / 784 / 1 613 ms. Other jobs shared the machine during both runs.
- 2026-11-05 (spec-check) — AC-03 re-measured on the reference machine (AMD Ryzen 7 PRO 6850H), criterion medians `ba53299^` → `HEAD`: 1024² × 2 / 5 / 10 layers 176.9 / 482.6 / 998.6 ms → 9.1 / 32.8 / 68.3 ms; 2048² 683.5 / 1 945 / 4 885 ms → 35.5 / 155.1 / 340.4 ms; 4096² 3 023 / 8 508 / 16 688 ms → 205.1 / 801.4 / 1 402 ms. The container run of 2026-11-03 had 1024² × 5 at 9.8× on a shared machine; on the reference machine it is 14.7×.
