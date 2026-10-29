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
| AC-01 | `cargo bench -p pincel-image-core --bench compose` — groups `image_compose` (`full_{1024,2048,4096}_{2,5,10}layers`), `image_region` (`zoom1_4k_10layers`, `fit_4k_10layers`), `image_stroke/brush_100_dabs_4k_10layers`, `image_effect/gaussian_r10_4k` in `crates/pincel-image-core/benches/compose.rs` | built |        |
| AC-02 | `cargo bench -p pincel-core --bench compose` — `compose/compose_4096_single_layer_zoom_1` in `crates/pincel-core/benches/compose.rs` | built |        |
| AC-03 | `pnpm perf` (`ui/playwright.perf.config.ts`, `ui/e2e-perf/scenarios.perf.ts`): 5 tests covering the 9 budget scenarios — every scenario records `frame-p50` / `frame-p95` / `frame-max`, `image-brush-4k` and `pixel-pencil-256-z32` also `dab-latency-p50` / `-p95`; output `ui/e2e-perf/results/browser.json` | built |        |
| AC-04 | `ui/e2e/perf-budget-check.spec.ts` › "budget mode (AC-04)" (5 tests: over-budget fails naming scenario / budget / value, ≤ 10 % incl. exactly 10 % passes, pending until first met then blocking, CLI `--write` records met, no-budget skipped) | built |        |
| AC-05 | `ui/e2e/perf-workflow.spec.ts` › "the PR gate runs neither the benchmarks nor the browser performance script (AC-05)" | built |        |
| AC-06 | `ui/e2e/perf-workflow.spec.ts` › "the performance workflow runs benches, browser script and budget check on dispatch and nightly (AC-06)" (static check of `.github/workflows/perf.yml`); manual: a dispatched run is still to be checked | built |        |
| AC-07 |      | open   |        |
| AC-08 | `ui/e2e/perf-budget-check.spec.ts` › "baseline mode (AC-08)" (4 tests: > 10 % worse fails naming scenario / baseline / value, exactly 10 % passes and becomes the baseline, budgets not checked, first run on a new runner records and passes) | built |        |

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
- 2026-10-29 (spec-build): The budget check is `ui/e2e-perf/check-budgets.ts`, run directly by Node's type stripping (`pnpm perf:check`, Node ≥ 22.18) — no new dependency, and it stays under the UI's strict TypeScript and lint. Its unit tests are `ui/e2e/perf-budget-check.spec.ts`: pure Node, no browser, so they join the PR e2e run; AC-05 only keeps the benchmarks and the browser script out.
- 2026-10-29 (spec-build): Frame-rate budgets ("60 fps") check the p95 frame time against 17 ms and latency budgets the p95 latency — p95 is what a user feels as stutter, the p50 is reported alongside, and rAF timestamps at 60 Hz jitter between 16.6 and 16.8 ms, so 16.7 ms itself would never be met reliably.
- 2026-10-29 (spec-build): Budget mode keeps "first met" dates in `ui/e2e-perf/budgets-met.json`; `--write` updates it and the reference-machine run commits it. Baseline mode keeps the runner's last values in a file the workflow caches per runner; a run with a regression never overwrites it, so the regression keeps failing until fixed.
- 2026-10-29 (spec-build): AC-05's test was green on its first run — the PR gate never ran benches, and `pnpm test:e2e` only collects `ui/e2e`. The test pins that state so a later CI change cannot pull the benchmarks or `ui/e2e-perf` into the gate.
- 2026-10-29 (spec-build): The nightly skip compares `github.sha` with the head SHA of the last completed scheduled run (`gh run list`); the runner baseline lives in the actions cache keyed by OS and architecture (restored by prefix, saved only after a passing check), so a dispatched run on a PR branch reads `main`'s baseline but cannot overwrite it.
- 2026-10-29 (spec-build): The browser script instruments the page from the test (`ui/e2e-perf/probe.ts`, an init script): rAF timestamps give frame times; draws are `putImageData` / `drawImage` on the editor's render canvas and WebGPU `writeTexture`; dab latency is a pointer event's `timeStamp` to the first frame that starts after the first draw following it. The app code is unchanged and still never reads canvas pixels.
- 2026-10-29 (spec-build): Scenario operationalisations — recompose: toggling a layer's visibility at fit zoom (pointer-down → first draw); pan / zoom: middle-button drag plus wheel; effect: proxy = radius input → first preview draw, full = OK → first draw; autosave / export: Save (the ORA encoder autosave uses) plus Export PNG, longest frame; open: file input `change` → first frame after the canvas's `data-ready`; cold start: Lighthouse Slow 4G values via CDP, HTTP cache disabled, warm = second load with a filled cache (service workers blocked, as in e2e); memory: 40 short strokes, 5 Invert Colors, 3 Merge Down, 2 pastes of 1024² after adding 9 layers, largest `WebAssembly.Memory`. A pixel drag moves one sprite pixel (32 CSS px) per event — an event inside the same pixel paints nothing and has no frame to wait for.
- 2026-10-29 (spec-build): AC-03 had no meaningful red run: the script is new tooling whose absence is the red state (`playwright test -c playwright.perf.config.ts` found nothing). The first real run failed on a scenario bug (a paste slower than the 5 s assertion timeout), fixed before this commit.
