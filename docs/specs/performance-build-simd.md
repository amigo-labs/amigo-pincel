---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §10.1, pincel.md §15 (2026-10-13 one Worker, cf-build), pincel.md §15 (2026-10-26 releases)]
depends_on: [performance-benchmarks, performance-image-kernel]
---

# Performance: SIMD and build settings

## Context

The workspace ships without release tuning: the root `Cargo.toml` has no `[profile.release]` (no LTO, default codegen units), the wasm modules are built without SIMD, and the Cloudflare build (`scripts/cf-build.sh`) runs `wasm-opt` only when the build image happens to have it — otherwise it ships the unoptimized release build without a warning.

Step 5 of the legacy plan `23-performance` (removed 2026-10-29, see git history) tunes the release profile by measurement, builds both wasm modules with 128-bit wasm SIMD, and guards that every deployed build is an optimized release build. Together with `performance-image-kernel` it owns the budgets "full recompose of a 4K, 10-layer Image document ≤ 100 ms" and "cold start ≤ 1.5 s, warm ≤ 0.5 s".

## Scope

### In scope

- A release profile chosen by measured size and speed
- Both wasm modules built with 128-bit SIMD, with identical results to the scalar build
- A CI check that the website / PWA bundle carries optimized release wasm; the same for the Tauri bundle and the npm package

### Out of scope

- AVX / AVX2 for native binaries — because the editor runs the cores as wasm (128-bit SIMD only); native runtime dispatch belongs to `performance-desktop-jobs`, and the plan ranks it the lowest-priority lever
- wasm threads (`SharedArrayBuffer`, COOP/COEP, `wasm-bindgen-rayon`, nightly `build-std`) — because the plan decides on them only if the 4K budgets are still missed after the other steps, and they are a stop-and-ask dependency, toolchain and hosting decision
- GPU compositing — because it would be a large architecture change (pincel.md §4); stop and ask
- Changing the color pipeline — because linear-light blending is the existing `compose()` contract

## Acceptance criteria

- [ ] AC-01 (always): The release profile in use is the one recorded under `## Decisions`, which lists for each measured candidate the wasm module sizes and the compose bench medians it was chosen on.
- [ ] AC-02 (always): The wasm modules shipped in the website / PWA, the Tauri bundle and the npm package use 128-bit wasm SIMD, and the compose and effect tests produce identical output for the SIMD build and the scalar build.
- [ ] AC-03 (error): If the website bundle's wasm is not a release build that went through `wasm-opt`, then the CI check fails and names the module.
- [ ] AC-04 (always): All Rust tests of the six library crates pass under the chosen release profile, including `panic = "abort"` for wasm if it is chosen.
- [ ] AC-05 (state, manual: the budget is defined for the reference laptop): While the reference laptop runs the production build, a full recompose of a 3840×2160, 10-layer Image document takes at most 100 ms in the browser performance script.
- [ ] AC-06 (event, manual: the budget is defined for the reference laptop and network profile): When the production build is loaded without cache, then the start screen is interactive within 1.5 s; with a warm cache, within 0.5 s.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- Root `Cargo.toml`: candidates `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`, and `panic = "abort"` for wasm if tests allow; `opt-level = "s"` may win on load time — decide by the `performance-benchmarks` numbers.
- wasm SIMD: build both wasm crates with `-C target-feature=+simd128`. Write the kernel from `performance-image-kernel` so it auto-vectorizes, or use `core::arch::wasm32` intrinsics behind `cfg(target_feature = "simd128")` with a scalar fallback for native and tests. Minimum browsers per the plan: Chrome / Edge 91+, Firefox 89+, Safari 16.4+.
- `wasm-opt = false` in `crates/pincel-wasm/Cargo.toml` and `crates/pincel-image-wasm/Cargo.toml`; release CI runs `wasm-opt -O3` (`.github/workflows/release.yml` ≈L94, ≈L377); `scripts/cf-build.sh` runs it only `if command -v wasm-opt`. The CI check can use a size threshold or a custom-section marker.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Cloudflare build image without `wasm-opt` | The CI check (AC-03) fails instead of a silent unoptimized deploy |
| `panic = "abort"` breaks a test that expects unwinding | `panic = "abort"` is not used for that build (AC-04) |
| `opt-level = "s"` loads faster but composes slower | Open question (see below) |
| Browser without wasm SIMD (e.g. Safari < 16.4) | Open question (see below) |

## Open questions

- If `opt-level = "s"` wins load time (cold-start budget) but loses compose speed (recompose budget), which budget takes precedence?
- What does a browser without wasm SIMD get: a scalar fallback build, or an "unsupported browser" message — and what exactly does the user see?
- The Decision Log entry recording the minimum browser versions for wasm SIMD is not written yet — confirm the versions (Chrome / Edge 91+, Firefox 89+, Safari 16.4+) and write it with this spec.

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |

## Decisions

- The release profile is chosen by measured size and speed, not by default — no tuning without a number (plan, 2026-10-28).
- AVX / AVX2 do not apply to the editor: wasm SIMD is 128-bit only in every shipping engine (plan).
