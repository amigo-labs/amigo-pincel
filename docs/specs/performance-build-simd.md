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
- [ ] AC-07 (error): If the browser does not support wasm SIMD, then the start screen shows a translated notice "This browser is too old for Pincel (needs Chrome/Edge 91+, Firefox 89+, Safari 16.4+)" and New and Open are disabled; no editor is loaded.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- Root `Cargo.toml`: candidates `lto = "fat"`, `codegen-units = 1`, `opt-level = 3`, and `panic = "abort"` for wasm if tests allow; `opt-level = "s"` may win on load time — decide by the `performance-benchmarks` numbers.
- wasm SIMD: build both wasm crates with `-C target-feature=+simd128`. Write the kernel from `performance-image-kernel` so it auto-vectorizes, or use `core::arch::wasm32` intrinsics behind `cfg(target_feature = "simd128")` with a scalar fallback for native and tests. Minimum browsers per the plan: Chrome / Edge 91+, Firefox 89+, Safari 16.4+.
- Release profile: the runtime budgets (recompose, interaction) take precedence over load time. The hot crates (`pincel-core`, `pincel-image-core`, `pincel-effects`) get `opt-level = 3` through `[profile.release.package.<crate>]` overrides; other crates may use `opt-level = "s"` if it measurably helps. The start screen loads no wasm (editors are `import()`ed), so cold start barely depends on wasm size.
- SIMD detection: the start screen feature-detects with `WebAssembly.validate` on a tiny module that uses a SIMD instruction. No scalar fallback build ships. New i18n keys (in `en.ts` and `de.ts`): `start.unsupportedBrowser` (the notice text of AC-07).
- `wasm-opt = false` in `crates/pincel-wasm/Cargo.toml` and `crates/pincel-image-wasm/Cargo.toml`; release CI runs `wasm-opt -O3` (`.github/workflows/release.yml` ≈L94, ≈L377); `scripts/cf-build.sh` runs it only `if command -v wasm-opt`. The CI check can use a size threshold or a custom-section marker.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Cloudflare build image without `wasm-opt` | The build installs a pinned binaryen release first; if that download fails, the build fails, and a module that still skipped `wasm-opt` fails the CI check (AC-03) instead of a silent unoptimized deploy |
| `panic = "abort"` breaks a test that expects unwinding | `panic = "abort"` is not used for that build (AC-04) |
| `opt-level = "s"` loads faster but composes slower | The recompose budget wins: the hot crates keep `opt-level = 3`; `"s"` only for the other crates |
| Browser without wasm SIMD (e.g. Safari < 16.4) | The start screen shows the unsupported-browser notice and disables New / Open (AC-07) |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | Root `Cargo.toml` `[profile.release]` (`lto = "fat"`, `codegen-units = 1`) is candidate B of the 2026-11-06 measurement under Decisions; `ui/e2e/release-profile.spec.ts` › "the release profile is the measured candidate B (AC-01)", "the release profile overrides neither opt-level nor panic (AC-01)", "the release profile has no package or build overrides (AC-01)" — 3 passed at 943f6533, all 3 red against a profile changed to `lto = "thin"` + `opt-level = "s"` + `[profile.release.package.pincel-core]` | built  | 943f6533 |
| AC-02 | `.cargo/config.toml` (`+simd128` for `wasm32-unknown-unknown`); `ui/e2e/wasm-simd-check.spec.ts` › "a module built with simd128 passes (AC-02)", "a module without simd128 fails and is named (AC-02)", "the built wasm packages use simd128 (AC-02)"; the check `scripts/check-wasm-simd.mjs` runs in `scripts/cf-build.sh` (website / PWA) and in `.github/workflows/release.yml` (web, desktop and npm jobs); `ui/e2e-simd/simd-equivalence.simd.ts` › "pincel-image-wasm: the SIMD build composes and runs every effect like the scalar build (AC-02)", "pincel-wasm: …" (`pnpm wasm:build:scalar && pnpm simd:check`, in `release.yml` and the nightly `perf.yml`); the Image battery composes through all 11 non-normal blend modes (list checked against `BlendMode` by svelte-check, each mode read back from the document). `pnpm wasm:build:release && pnpm wasm:build:scalar && pnpm simd:check` at 58701f19: 2 passed, every output byte-identical | built  | 58701f19 |
| AC-03 | `ui/e2e/wasm-release-check.spec.ts` › "an optimized release module passes (AC-03)", "a module that skipped wasm-opt fails and is named (AC-03)", "a dev build fails (AC-03)"; the check `scripts/check-wasm-release.mjs` runs in `scripts/cf-build.sh` (the website / PWA bundle) and in `.github/workflows/release.yml` after `wasm-opt` (Tauri bundle, npm package) | built |        |
| AC-04 | `cargo test --release -p pincel-core -p pincel-effects -p pincel-trace -p aseprite-writer -p pincel-wasm -p pincel-image-core -p pincel-image-wasm -p pincel-agent -p pincel-mcp` under the chosen profile: 2225 passed, 0 failed (2026-11-06, 11 min 40 s); the release workflow's Rust gate (`.github/workflows/release.yml`, web job) now runs with `--release` and since 58701f19 covers `pincel-agent` and `pincel-mcp` as well. Re-run at 58701f19 (Rust sources as at 9354d5c5): 2385 passed, 0 failed, 0 ignored in 55 test binaries (10 min 27 s) | built  | 58701f19 |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 | `ui/e2e/browser-support.spec.ts` › "without wasm SIMD the start screen explains why and loads no editor (AC-07)", "with wasm SIMD the start screen shows no notice (AC-07)" | built |        |

## Decisions

- The release profile is chosen by measured size and speed, not by default — no tuning without a number (plan, 2026-10-28).
- AVX / AVX2 do not apply to the editor: wasm SIMD is 128-bit only in every shipping engine (plan).
- 2026-10-29: Runtime budgets take precedence over load time; the hot crates (`pincel-core`, `pincel-image-core`, `pincel-effects`) keep `opt-level = 3` through per-package release overrides, other crates may use `"s"` — compose speed is felt on every interaction, while load time is paid once and then served from the service-worker cache (owner accepted the recommendation).
- 2026-10-29: No scalar fallback build; a browser without wasm SIMD gets a translated "browser too old" notice on the start screen with New / Open disabled — a second build doubles CI and bundle work for a share of browsers well under 1 % (owner accepted the recommendation).
- 2026-10-29: Both wasm modules are built with `+simd128`; minimum browsers are Chrome / Edge 91, Firefox 89 and Safari 16.4 (Decision Log 2026-10-29) — these are the first releases with fixed-width wasm SIMD enabled by default (owner accepted the recommendation).
- 2026-11-06 (spec-build): SIMD detection is `supportsWasmSimd()` (`ui/src/lib/platform/wasm-simd.ts`): `WebAssembly.validate` on a 31-byte module whose one function runs `i8x16.splat` and `i8x16.popcnt`. Without SIMD the start screen shows `start.unsupportedBrowser` and disables both New cards, Open and the recent files, and the shell's editor loaders refuse to load, so no other way in (drop, clipboard offer, startup document) mounts an editor either. Red run: the AC-07 e2e test failed (no notice) before the check existed.
- 2026-11-06 (spec-build): The release check (`scripts/check-wasm-release.mjs`) reads the modules' custom sections: `wasm-opt` writes no `name` section unless asked for debug info (`-g`), and a dev build carries DWARF `.debug_*` sections, so either one fails the check and names the module — no size threshold to keep in step with the code, no marker to add. Today's `pnpm wasm:build:release` output fails it (both modules carry `name`), as it should until `wasm-opt` has run. Consequence of AC-03 as specified: a Cloudflare build image without `wasm-opt` now fails the deploy (the edge case in the table) instead of shipping the unoptimized build. Red run: the AC-03 tests failed without the script.
- 2026-11-06: The Cloudflare build fetches a pinned binaryen release (`version_133`, the static `wasm-opt` binary from GitHub) when the image has no `wasm-opt`, and runs it unconditionally — the first deploy after AC-03 failed because Cloudflare's image has no binaryen, so the check rejected both modules as designed; the release workflow keeps apt's binaryen.
- 2026-11-06 (spec-build): SIMD is switched on for every `wasm32-unknown-unknown` build in `.cargo/config.toml`, dev builds included, so the e2e suite runs the code that ships and no delivery can miss the flag; the kernels rely on LLVM auto-vectorisation, no `core::arch::wasm32` intrinsics. The build check `scripts/check-wasm-simd.mjs` reads the module's `target_features` section (rustc lists `+simd128` there) on wasm-pack's output, before `wasm-opt`: a SIMD instruction cannot vanish in `wasm-opt` (it fails to validate without SIMD), so no instruction decoder is needed. `wasm-opt` gets `--enable-simd` everywhere it runs. Red run: "the built wasm packages use simd128" failed on the packages built before the flag ("simd128 is not among its target features").
- 2026-11-06 (spec-build): The SIMD / scalar equivalence runs in Node against two release builds (the shipped packages and `pnpm wasm:build:scalar`, which overrides the flag with `RUSTFLAGS=-C target-feature=-simd128` in its own target directory): both cores compose four layers through every non-normal blend mode and run every catalog effect at its defaults on 160×120 pixels with gradients, noise and mixed alpha, and every output must match byte for byte. Release builds, because an unoptimised build barely vectorises. It is not part of `pnpm test:e2e` (no scalar build there) but of the nightly `perf.yml` and of `release.yml` before the web bundle is built (`pnpm simd:check`, own Playwright config, no browser); two more release builds in every PR's CI were not worth it for a property that only a toolchain or kernel change can break. It passed at once, as an equivalence must on code without float reassociation (LLVM vectorises floats only where the result stays bit-exact); the check's own red run is AC-02's SIMD check above.
- 2026-11-06 (spec-build): Release profile measured on the reference laptop (Ryzen 7 PRO 6850H, Windows 11, Chromium 156), all candidates with `+simd128`. Module sizes are wasm-pack's output before `wasm-opt` (not installed locally; CI's `wasm-opt -O3` shrinks every candidate further). Browser numbers: `pnpm perf -g "brush, pan"` on the release bundle, `image-recompose-4k` recompose median (full recompose of the 4K, 10-layer document) per run, runs of all candidates interleaved; native numbers: `cargo bench --bench compose` criterion medians (1 s warm-up, 3 s measurement).

  | Candidate | `pincel_wasm_bg.wasm` | `pincel_image_wasm_bg.wasm` | Browser recompose p50, per run | Native `image_compose/full_1024_10layers` / `full_4096_10layers` / Pixel `compose_256_four_layers_full` |
  | --------- | --------------------- | --------------------------- | ------------------------------ | -------------------------------------------------------------------------------------------------------- |
  | A: default release (`opt-level = 3`, thin-local LTO, 16 codegen units) | 1 661 981 B | 3 091 544 B | 265, 206, 373, 280 ms | 96.5 / 3060 / 3.89 ms |
  | **B: A + `lto = "fat"`, `codegen-units = 1`** (chosen) | 1 479 684 B | 2 802 714 B | 170, 136, 236, 235 ms | 170.8 / 3229 / 4.03 ms |
  | C: B + `opt-level = "s"`, `opt-level = 3` for `pincel-core`, `pincel-image-core`, `pincel-effects` | 1 600 051 B | 2 950 622 B | 335, 335, 249 ms | 134.1 / 1659 / 1.95 ms |
  | A, scalar (no `+simd128`), for AC-02 | 1 644 734 B | 3 037 677 B | 256, 292 ms | — |

  B is the smallest and composed fastest in every pair of adjacent runs (median ≈ 203 ms against ≈ 272 ms for A), so the editor's wasm, which the runtime budgets are about, decided it. C lost on both counts: `"s"` for the non-hot crates did not shrink the modules (the hot crates dominate after fat LTO) and recomposed slower. The native criterion medians were dominated by load on the shared machine (C, compiled like B for the compose code, measured half of B), so they decided nothing; the brush-dab and Gaussian numbers of the same runs did not separate the candidates (dab draw p50 2.8–3.6 ms, Gaussian full 1.6–1.7 s). `panic = "abort"` was not a candidate: `wasm32-unknown-unknown` aborts on panic already, and the native binaries keep unwinding. No per-package `opt-level` overrides, because every crate keeps the default 3. The browser script's Gaussian step failed in about half of the runs of every candidate, A included (its proxy draw needs ≈ 2.8–3.0 s against the step's 3 s wait), so those runs only have the brush, pan / zoom and recompose numbers.
- 2026-11-06 (spec-build): AC-04 is checked by running every library crate's tests under the release profile, locally and from now on in the release workflow's Rust gate (`cargo test --release`), not in the PR gate: fat LTO puts each test binary through a whole-program link (≈ 12 min locally against ≈ 1 min in the dev profile), which would break the 60 s budget of AGENTS.md §7.5. It passed at once — nothing in the profile changes behaviour; `panic = "abort"` was not chosen (see the AC-01 entry), so no test that expects unwinding is affected.
- 2026-11-06 (acceptance fixes): The SIMD equivalence's Image battery covered 6 of the 11 non-normal blend modes; it now runs all of them. The list is an object literal that `satisfies Record<Exclude<BlendMode, 'normal'>, true>` (`BlendMode` from `ui/src/modes/image/core/wasm.ts`), so svelte-check fails when the type gains a mode the battery lacks (red run: dropping `exclusion` gave "Property 'exclusion' is missing"); because the core maps an unknown mode string to normal without an error, the battery also reads each mode back from `get_document_info` before composing. No runtime list was exported from `pincel-image-wasm` for it, to keep its public API unchanged. The Pixel battery keeps `blendModeNames()`. With all 11 modes the SIMD and scalar builds still matched byte for byte, so no kernel needed a fix.
- 2026-11-06 (acceptance fixes): AC-01's profile is pinned by `ui/e2e/release-profile.spec.ts`, a Node-side spec that parses the root `Cargo.toml` itself (no TOML dependency): `[profile.release]` must hold exactly `lto = "fat"` and `codegen-units = 1`, set neither `opt-level` nor `panic` (also not as `[profile] release.*` keys), and have no `[profile.release.*]` tables (package or build overrides). Changing the profile now means a new measurement, a new entry here and this test in the same commit. Red run: all three tests failed against a temporary `lto = "thin"`, `opt-level = "s"` and `[profile.release.package.pincel-core]`.
- 2026-11-06 (acceptance fixes): The release workflow's release-profile test step listed seven of the nine library crates the AC-04 command names; it now also runs `pincel-agent` and `pincel-mcp`, so CI checks what the evidence states.
