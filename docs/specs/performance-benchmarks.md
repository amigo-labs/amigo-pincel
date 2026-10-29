---
status: todo
created: 2026-10-28
size: L
mode: both
design: [pincel.md §4.1, pincel.md §4.3, pincel.md §10.4, pincel.md §16 (M12)]
depends_on: []
---

# Performance: benchmarks and budgets

## Context

The owner's words: "maximum performance, the thing has to fly". Image mode is far from that today: a native release build composes 1024×1024 with 2 layers in 101 ms and 4096×4096 with 2 layers in 1 262 ms (wasm is typically 1.2–2× slower), and every brush dab recomposes the whole visible image. Pixel mode is in better shape (M12 baselines in STATUS.md), but large sprites at low zoom were never measured.

The legacy plan `23-performance` (removed 2026-10-29, see git history) sets **budgets** and the rule "measure, change, measure": every performance change states before → after on the benchmark it targets, and no speculative tuning happens without a number. This spec is its step 0 — the benchmarks, the in-browser measurement and the regression guard every other `performance-*` spec proves its gain against. It changes no runtime behavior.

## Scope

### In scope

- A criterion benchmark for `pincel-image-core` compose, region compose, brush strokes and a blur
- A Pixel-mode benchmark of a 4096×4096 sprite at the lowest zoom
- A browser performance script, outside the 60 s e2e gate, that measures the budget scenarios in the built app
- A budget check that compares results with the budget table and fails on a regression
- A manual or nightly workflow running benches, script and check
- The measured numbers recorded in this spec's `## Measurements` table

### Out of scope

- The optimizations themselves — because each lives in its own `performance-*` spec and must prove its gain against these numbers
- Performance tests in the PR gate — because CLAUDE.md §7.5 keeps the full test suite under 60 s
- GPU compositing (WebGPU compute) — because it would be a large architecture change (pincel.md §4); the plan says stop and ask, and only if the CPU paths miss the 4K budgets

## Acceptance criteria

- [ ] AC-01 (event): When `cargo bench -p pincel-image-core --bench compose` runs, then it reports a median for each of: full compose of 1024×1024, 2048×2048 and 4096×4096 with 2, 5 and 10 layers; region compose of a 3840×2160, 10-layer document at zoom 1 and at fit zoom; a 100-dab brush stroke on a 3840×2160, 10-layer document; and a Gaussian blur with radius 10 on a 3840×2160 layer.
- [ ] AC-02 (event): When `cargo bench -p pincel-core --bench compose` runs, then it reports, in addition to the existing scenarios, a median for composing a 4096×4096 sprite at zoom 1.
- [ ] AC-03 (event): When the browser performance script runs against the built app, then for each budget scenario under Technical notes it reports p50 and p95 frame time, and for the painting scenarios also p50 and p95 dab latency (pointer event → pixels presented).
- [ ] AC-04 (error): If, in budget mode (on the reference machine), a measured value exceeds a budget that was met before by more than 10 %, then the budget check exits non-zero and names the scenario, the budget and the measured value; within 10 % it exits zero. A budget that has never been met is reported as `pending` with its measured value, and does not fail the check; it blocks from the first run that meets it.
- [ ] AC-05 (always): The PR CI gate runs neither the criterion benchmarks nor the browser performance script.
- [ ] AC-06 (event, manual: the workflow runs only on its schedule or on manual dispatch): When the performance workflow runs — triggered by manual dispatch (`workflow_dispatch`) or by a nightly schedule on `main` — then it runs the criterion benchmarks, the browser performance script and the budget check, and fails when the budget check fails; the nightly run is skipped when `main` has no new commit since the last run.
- [ ] AC-07 (always, manual: documentation): This spec's `## Measurements` section holds a table with the latest median per benchmark and the p50/p95 per browser scenario, with the date and the machine they were measured on.
- [ ] AC-08 (error): If, in baseline mode (on a CI runner), a measured value is more than 10 % worse than the last recorded value for that runner, then the budget check exits non-zero and names the scenario, the baseline and the measured value; in baseline mode the budgets themselves are not checked.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

**Budgets** (the plan's table; "today" as of 2026-10-28). The owning spec is the one whose change is expected to meet the budget. **Reference machine ("mid laptop"):** a 4-core / 8-thread x86 laptop of the 2021 class (i5-1135G7 / Ryzen 5 5500U class), 16 GB RAM, on the "balanced" power plan, running current stable Chrome. **"4G":** Lighthouse's Slow 4G throttling — 150 ms RTT, 1.6 Mbps down, 750 kbps up.

| Scenario | Budget | Today | Owning spec |
| -------- | ------ | ----- | ----------- |
| Brush dab → pixels on screen, Image, 3840×2160, 10 layers, any zoom | ≤ 16 ms (60 fps) | far over | `performance-image-cache` |
| Pan / zoom, Image 4K 10 layers | 60 fps, no recompose while panning | to measure | `performance-image-cache` |
| Full recompose, Image 4K 10 layers (wasm, mid laptop) | ≤ 100 ms, off the input path | ≈ 4 s native, extrapolated | `performance-image-kernel` (≥ 10×), `performance-build-simd` |
| Pixel: pencil drag on 256×256 at zoom 32 | 60 fps (M12 target), dab latency ≤ 8 ms | per-pixel wasm calls | `performance-input-path` |
| Open a 4K PNG | ≤ 1 s to first paint | to measure | measure first; a new spec `performance-image-open` if missed |
| Effect preview (Gaussian r = 10) on 4K | proxy ≤ 100 ms, full result ≤ 500 ms, UI never blocks | main thread | `performance-background-jobs` |
| Autosave and export of a 4K multi-layer doc | zero UI jank (no frame > 50 ms) | ORA encode on main thread every 30 s | `performance-background-jobs` |
| Cold start (no cache) → start screen interactive | ≤ 1.5 s mid laptop / 4G; warm ≤ 0.5 s | to measure | `performance-build-simd` |
| Memory, 4K × 10 layers with 50 undo steps (40 strokes ≤ 512 px, 5 whole-layer adjustments / effects, 3 merge-downs, 2 pastes of 1024²), measured as `WebAssembly.Memory` `buffer.byteLength` | ≤ 1 GB | ≈ 330 MB per structural undo step | `performance-image-undo-memory` |

**Measured today** (native release, dev container): Image full compose 1024²×2 = 101 ms, 2048²×2 = 322 ms, 2048²×5 = 825 ms, 4096²×2 = 1 262 ms. Pixel compose 256² ≈ 0.13 ms (1 layer), 1.4 ms (4 layers) — STATUS.md "M12 baselines".

**Pieces.** Image bench `crates/pincel-image-core/benches/compose.rs` (new; mirror `crates/pincel-core/benches/compose.rs`, `criterion` is already a workspace dev-dependency). Browser script in its own directory, e.g. `ui/e2e-perf/`, not part of `pnpm test:e2e`; it uses the Frame-Time Probe (View ▸ Frame-Time Probe, i18n key `menu.probe`) and `performance.now()` around drag sequences. Pixel zoom minimum is 1 (`ui/src/modes/pixel/view-ops.ts`).

**Docs.** pincel.md §4.5 "Performance budgets" holds the table above; Decision Log 2026-10-29 makes the budgets contracts.

**Throttling presets.** Network: Chrome DevTools / Lighthouse "Slow 4G" (150 ms RTT, 1.6 Mbps down, 750 kbps up). CPU: none — the reference machine is the real hardware; on CI runners only baseline mode applies.

**Regression guard modes.** Baseline mode (CI runners): compare with the last recorded numbers of the same runner, +10 % fails (AC-08). Budget mode (reference machine): compare with the budgets; `pending` until a budget is first met, then blocking (AC-04).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A scenario has no budget yet ("to measure") | The script reports it; the check skips it until a budget exists |
| A budget is not met yet (budget mode) | Reported as `pending` with the measured value; the check stays green (AC-04) |
| First CI run on a new runner (no baseline yet) | The run records the baseline; nothing to compare, the check passes |
| A measured value exactly 10 % over budget | Check passes; more than 10 % fails (AC-04) |
| The perf script runs against a dev build | Not supported: like the e2e suite (CLAUDE.md §7.4), it runs against the built bundle |
| A bench scenario's median regresses but stays within budget | The check passes; the before → after number shows in this spec's `## Measurements` table |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Region compose, stroke and blur benches use 3840×2160 with 10 layers | The budget table defines 4K as 3840×2160 with 10 layers | Low |
| "Dab latency" is the time from the pointer event to the frame that shows its pixels | The budget reads "brush dab → pixels on screen" | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `cargo bench -p pincel-image-core --bench compose` — groups `compose` (`full_{1024,2048,4096}_{2,5,10}layers`), `region` (`zoom1_4k_10layers`, `fit_4k_10layers`), `stroke/brush_100_dabs_4k_10layers`, `effect/gaussian_r10_4k` in `crates/pincel-image-core/benches/compose.rs` | built |        |
| AC-02 | `cargo bench -p pincel-core --bench compose` — `compose/compose_4096_single_layer_zoom_1` in `crates/pincel-core/benches/compose.rs` | built |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |

## Decisions

- Measure, change, measure: every performance commit states before → after on the benchmark it targets; no speculative tuning without a number (plan, 2026-10-28).
- Benchmarks and the browser script stay out of the 60 s PR gate; any unavoidable slow test is `#[ignore]` with a linked issue (CLAUDE.md §7.5).
- A budget is exceeded when the measured value is more than 10 % over it (plan, "Regression guard").
- 2026-10-29: The reference "mid laptop" is a 4-core / 8-thread x86 laptop of the 2021 class (i5-1135G7 / Ryzen 5 5500U class), 16 GB, "balanced" power, current stable Chrome; "4G" is Lighthouse's Slow 4G (150 ms RTT, 1.6 Mbps down, 750 kbps up) — named, reproducible presets make the budgets checkable by anyone (owner accepted the recommendation).
- 2026-10-29: The regression guard compares CI runs with runner-specific baselines (+10 % fails) and reference-machine runs with the budgets, where a never-met budget is `pending` and blocks once met — runners aren't the reference machine, and a check that is red from day one gets ignored (owner accepted the recommendation).
- 2026-10-29: The performance workflow runs on manual dispatch and nightly on `main`, skipping nights without a new commit — nightly catches regressions without anyone remembering, and dispatch proves a single PR (owner accepted the recommendation).
- 2026-10-29: The budgets become pincel.md §4.5 "Performance budgets" and are contracts: every performance change states before → after, a budget once met may not regress by more than 10 %, benchmarks run outside the 60 s PR gate (Decision Log 2026-10-29) — the other `performance-*` specs all cite these budgets (owner accepted the recommendation).
- 2026-10-29: "Open a 4K PNG ≤ 1 s to first paint" has no owner yet: this spec measures it, and a new spec `performance-image-open` takes it only if the measurement misses the budget — no speculative tuning without a number, and decode plus the cached composite may already meet it (owner accepted the recommendation).
- 2026-10-29 (spec-build): `pincel-image-core` gains `criterion` and `pincel-effects` as dev-dependencies — both are workspace crates already in `Cargo.lock`, and AC-01 needs the Gaussian blur inside this crate's bench.
- 2026-10-29 (spec-build): The 100-dab stroke bench applies 100 single-point Pencil strokes (one dab each, size 20, one per pointer event as the UI sends them) through `Command::apply` without an undo stack — exactly 100 dabs, and the measured cost is rasterize + apply, not history growth.
- 2026-10-29 (spec-build): The region benches render into a 1920×1080 viewport: at zoom 1 a centered 1920×1080 rect, at fit zoom the whole 3840×2160 canvas scaled bilinearly to 1920×1080, as `composite_region` does — a full-HD window is the common desktop case.
