---
id: todo-performance
title: Performance — make it fly (budgets, compositing, caching, SIMD, workers, build)
status: todo
size: XL (seven M sessions; measure first, every step proves its gain)
mode: both (Image mode first: that's where the time goes)
depends_on: []
created: 2026-10-28
---

# Performance: make it fly

## Goal

The owner's words: "maximum performance, the thing has to fly". Concretely, these are the **budgets**. Every one of them gets a measurement that CI or a script repeats.

| Scenario | Budget | Today |
|----------|--------|-------|
| Brush dab → pixels on screen, Image mode, 4K (3840×2160), 10 layers, any zoom | **≤ 16 ms** (60 fps) | Far over: see the measurements below |
| Pan / zoom, Image 4K 10 layers | 60 fps, no recompose while panning | to measure |
| Full recompose, Image 4K 10 layers (wasm, mid laptop) | ≤ 100 ms, **off the input path** | to measure (native: ≈ 4 s extrapolated) |
| Pixel mode: pencil drag on 256×256 at zoom 32 | 60 fps (M12 target), dab latency ≤ 8 ms | per-pixel wasm calls (see `01-pixel-pencil.md`) |
| Open a 4K PNG | ≤ 1 s to first paint | to measure |
| Effect preview (Gaussian r = 10) on 4K | ≤ 500 ms, **UI never blocks** | runs on the main thread |
| Autosave and export of a 4K multi-layer doc | **zero UI jank** (no frame > 50 ms) | ORA encoding on the main thread every 30 s |
| Cold start (no cache) → start screen interactive | ≤ 1.5 s on a mid laptop / 4G; warm ≤ 0.5 s | to measure |

## Measured today (2026-10-28, native release build in the dev container; wasm is typically 1.2–2× slower)

**Image-mode `compose()`** (`crates/pincel-image-core/src/render/mod.rs`) on a full canvas:

| Canvas | Layers | Full compose |
|--------|--------|--------------|
| 1024×1024 | 2 | **101 ms** |
| 2048×2048 | 2 | **322 ms** |
| 2048×2048 | 5 | **825 ms** |
| 4096×4096 | 2 | **1 262 ms** |

**Root causes, read in the code:**
1. **`composite_layer` (≈L116–170) converts sRGB → linear with `powf` for source and backdrop, and back with `powf`. That is 9 `powf` calls per pixel per layer.**
   - A quick prototype with a 256-entry sRGB → linear table and a 4096-entry linear → sRGB table blended a 2048² layer in **28 ms instead of 199 ms (≈ 7×)**, before any other change.
2. Per-pixel `get_pixel` / `set_pixel` return and take `Option<Color>`, with bounds checks and struct round-trips. Use direct row slices instead.
3. No fast paths:
   - a Normal-blend opaque source simply overwrites;
   - fully transparent source spans could be skipped per row;
   - a layer with opacity 0 or that is invisible is already skipped, which is good.
4. `composite_region` (`crates/pincel-image-wasm/src/lib.rs` ≈L1534) **composes the visible rect at full resolution and downscales afterwards**. At "fit" zoom on a large image, every dab recomposes the whole image.
5. There is **no composite cache**. Every redraw re-blends all layers, although a stroke only changes the active layer inside a small dirty rect.
6. Everything runs on the **main thread**: compose, effects, PNG/ORA encode for export and autosave.

**Pixel mode** is in better shape:
- Compose of 256² is ≈ 0.13 ms (1 layer) and 1.4 ms (4 layers), per the STATUS.md M12 baselines.
- Dirty-rect compose exists (M12.2).
- Its hot spot is input: one `applyTool` wasm call **per pixel**. `01-pixel-pencil.md` fixes that with stroke segments.
- Large sprites (up to 4096²) at low zoom haven't been measured. Measure them in step 0.

**Build:**
- The root `Cargo.toml` has **no `[profile.release]` tuning**: no LTO, codegen-units at the default.
- The wasm crates set `wasm-opt = false` for wasm-pack, while release CI runs `wasm-opt -O3` (`.github/workflows/release.yml` ≈L94, L377). Good, but verify that the **deployed website/PWA uses the release, wasm-opt build**.
- **No SIMD** (`+simd128`).
- **No workers.** No cross-origin isolation (COOP/COEP), so no wasm threads.

## Principles

- **Measure, change, measure.**
  - Every performance commit message states before → after on the benchmark it targets.
  - No speculative tuning without a number.
- **Exactness first.** Today's output defines correctness. LUT and integer paths must produce the **same bytes** as the current float path, or differ by at most 1 LSB where documented.
  - Keep the existing compose snapshot tests (`crates/pincel-image-core/tests/compose_snapshots.rs`).
  - Add a randomized equivalence test, old versus new, under proptest.
- **The input path never waits for heavy work.** Paint into a cache and present; anything slower than a frame goes to a worker or idle time.

## Steps (one M session each, in order)

### 0. Benchmarks and budgets (measure first)

- Add a criterion bench `crates/pincel-image-core/benches/compose.rs`, mirroring the Pixel one (`cargo bench -p pincel-core --bench compose`):
  - full compose 1K / 2K / 4K × 2 / 5 / 10 layers;
  - `compose_region` at zoom 1 and at fit;
  - a brush stroke of 100 dabs;
  - Gaussian blur r = 10 via `pincel-effects`.
- Add a Pixel bench for a 4096² sprite at low zoom.
- In-browser measurement:
  - a Playwright perf script that is **not** part of the 60 s e2e gate (CLAUDE.md §7.5), e.g. `ui/e2e-perf/`, run by hand or by a nightly job;
  - it uses the existing **Frame-Time Probe** (View ▸ Frame-Time Probe, `menu.probe`) and `performance.now()` around drag sequences;
  - it reports p50/p95 frame time and dab latency for each scenario in the budget table.
- **Record the numbers in a table in STATUS.md**, the same way as the M12 baselines.
- `docs(spec)`: a new §4.x "Performance budgets" with the table above, plus a Decision Log entry ("budgets are contracts; regressions block").

### 1. Image compositing kernel (biggest single win)

- LUTs:
  - `SRGB_TO_LINEAR: [f32; 256]`;
  - linear → sRGB through a 4096- or 8192-entry `u8` table, choosing the size by the exactness test;
  - built once (`std::sync::OnceLock` or `const fn`).
- Row-slice access on `ImageBuffer` (`row(y) -> &[u8]`, `row_mut`). There is no per-pixel `Option`.
- Fast paths:
  - Normal and sa = 1 → copy;
  - sa = 0 → skip;
  - da = 0 → straight copy with opacity;
  - whole transparent rows skipped through a per-row "any alpha" bitmap. Optional; measure it.
- Keep `blend_channel` for the 11 other modes, but on LUT-converted values.
- **Target ≥ 10× on full compose**, verified by the step 0 bench, with byte-identical output (or ≤ 1 LSB, documented) under the snapshot and equivalence tests.

### 2. Composite cache and dirty rects (Image mode)

- A cached full-resolution composite per document in the core, plus a dirty rect, updated **only in the changed rect** after each command or dab.
- **Stroke cache:**
  - When a stroke starts on layer *k*, compose layers `0..k` into `below` and `k+1..` into `above` once.
  - Each dab then blends only `below` + active layer + `above` inside the dab rect.
  - This is the paint.net / Krita approach.
- **Downscaled presentation:**
  - Keep a mip pyramid (½, ¼, …) of the cached composite, updated incrementally in dirty rects.
  - `composite_region` at zoom < 1 samples the matching mip level instead of composing at full resolution.
  - Fixes root cause 4.
- **Memory budget:** one composite plus mips is about 1.33 × W × H × 4 bytes. That's fine against `MAX_CANVAS_AREA` (64 Mpx → 340 MB worst case). **Document it, and skip mips above a threshold if needed.**
- Tests: the cache matches a fresh full compose after random command sequences, including undo, redo and layer moves (proptest).

### 3. Input path

- **Image mode** already uses `getCoalescedEvents` (`ui/src/modes/image/tools/pointer.ts` ≈L388). Batch all samples of one animation frame into a single wasm call; present once per rAF.
- **Pixel mode:** use `getCoalescedEvents`, with segments via `01-pixel-pencil.md`'s `strokeTo`.
- Optional: `getPredictedEvents()` to draw a provisional tip ahead of the pointer, which lowers perceived latency on pens. Measure with the step 0 script.
- Check `touch-action` / `pointer-events` CSS, so the browser never waits on scroll handling.

### 4. Off the main thread

- Move **autosave and export encoding** (PNG/ORA/JPEG/WebP) and **effect apply/preview** into a **Web Worker** that holds its own wasm instance.
  - Hand it document bytes (ORA) or layer buffers as transferable `ArrayBuffer`s.
  - This needs no cross-origin isolation.
  - The UI stays at 60 fps while saving.
- Effect previews: run them on a downscaled proxy first (instant), then the full-resolution result in the worker. Cancel a stale request when the parameters change.
- Decision Log entry: worker architecture, message protocol, and what stays on the main thread (interactive painting, which needs the document).

### 5. SIMD and build settings

- Root `Cargo.toml`: `[profile.release] lto = "fat"`, `codegen-units = 1`, `opt-level = 3`, and `panic = "abort"` for wasm if it doesn't break tests.
  - **Measure the size and speed trade-off.** `opt-level = "s"` may win on load time; decide by the step 0 numbers.
- **wasm SIMD.**
  - Build the two wasm crates with `-C target-feature=+simd128`. All target browsers support it: Chrome/Edge 91+, Firefox 89+, Safari 16.4+. Record the minimum in the Decision Log.
  - Write the kernel from step 1 so that it auto-vectorizes, or use `core::arch::wasm32` intrinsics behind `cfg(target_feature = "simd128")` with a scalar fallback for native and tests.
- **AVX / AVX2 don't apply to the editor.** Both the PWA and the Tauri app run the cores as wasm in the browser or webview, and wasm SIMD is 128-bit only. Engines lower it to SSE/AVX encodings, but at 128-bit width, and no 256-bit wasm SIMD has shipped.
  - AVX2 matters only for **native** binaries: `pincel-mcp` (spec 18) and a future CLI. There, use **runtime dispatch** (`is_x86_feature_detected!("avx2")` with a scalar or SSE2 fallback), never a blanket `target-cpu=x86-64-v3`, which would exclude pre-2013 x86 CPUs.
  - Apple Silicon and ARM use 128-bit NEON either way.
  - Expected gain over SSE2 is ≈ 1.3–2× in the kernel, less once the work is memory-bound. **Lowest priority of all the levers here.**
- Verify that **deployed builds are release plus wasm-opt** (Cloudflare build, Tauri bundle, npm package).
  - Add a CI check that fails if the website bundle's wasm is a dev build. A size threshold or a custom section marker would do.

### 6. Threads (decide after 1–5 are measured)

- Real multi-core compositing and effects in the browser need **wasm threads**: `SharedArrayBuffer`, which needs **COOP/COEP headers**, plus `wasm-bindgen-rayon` or equivalent.
- Costs:
  - cross-origin isolation can break third-party embeds;
  - the build needs nightly `build-std`.
- The Tauri app can set the headers on its custom protocol, and Cloudflare can set them for the PWA.
- **Stop and ask:** this is a dependency, toolchain and hosting decision. Only pursue it if the 4K budgets are still missed after steps 1–5.

### 7. Memory and undo for large images

- Image mode undo for structural commands clones the whole document (`DocSnapshot`, `crates/pincel-image-core/src/command/snapshot.rs`, used by paste, merge, flatten, scale and crop). On 4K × 10 layers that is about 330 MB per step.
- Move to **tile-based copy-on-write undo** (e.g. 128×128 tiles: store only the tiles a command changed) for pixel commands. Keep the snapshot only where the canvas size changes.
- Add a memory budget to the step 0 table (≤ 1 GB for 4K × 10 layers with 50 undo steps).

## Regression guard

- Bench numbers live in STATUS.md, and every performance PR updates them.
- A nightly or manual workflow runs the criterion benches and the browser perf script. It fails when a budget is exceeded by more than 10 %.
- **No perf tests in the 60 s PR gate** (CLAUDE.md §7.5); `#[ignore]` with a linked issue if any slow test is unavoidable.

## Out of scope

- GPU compositing of layers (WebGPU compute). A possible step 8 if the CPU paths with SIMD, threads and caching don't meet the 4K budgets. It would be a large architecture change (spec §4); stop and ask.
- Changing the color pipeline (linear-light blending is a spec decision, Decision Log 2026-10-27).
