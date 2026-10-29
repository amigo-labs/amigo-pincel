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
- [ ] AC-04 (error): If a measured value exceeds its budget by more than 10 %, then the budget check exits non-zero and names the scenario, the budget and the measured value; within 10 % it exits zero.
- [ ] AC-05 (always): The PR CI gate runs neither the criterion benchmarks nor the browser performance script.
- [ ] AC-06 (event, manual: the workflow runs only on its schedule or on manual dispatch): When the performance workflow runs, then it runs the criterion benchmarks, the browser performance script and the budget check, and fails when the budget check fails.
- [ ] AC-07 (always, manual: documentation): This spec's `## Measurements` section holds a table with the latest median per benchmark and the p50/p95 per browser scenario, with the date and the machine they were measured on.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

**Budgets** (the plan's table; "today" as of 2026-10-28). The owning spec is the one whose change is expected to meet the budget.

| Scenario | Budget | Today | Owning spec |
| -------- | ------ | ----- | ----------- |
| Brush dab → pixels on screen, Image, 3840×2160, 10 layers, any zoom | ≤ 16 ms (60 fps) | far over | `performance-image-cache` |
| Pan / zoom, Image 4K 10 layers | 60 fps, no recompose while panning | to measure | `performance-image-cache` |
| Full recompose, Image 4K 10 layers (wasm, mid laptop) | ≤ 100 ms, off the input path | ≈ 4 s native, extrapolated | `performance-image-kernel` (≥ 10×), `performance-build-simd` |
| Pixel: pencil drag on 256×256 at zoom 32 | 60 fps (M12 target), dab latency ≤ 8 ms | per-pixel wasm calls | `performance-input-path` |
| Open a 4K PNG | ≤ 1 s to first paint | to measure | none (see Open questions) |
| Effect preview (Gaussian r = 10) on 4K | ≤ 500 ms, UI never blocks | main thread | `performance-background-jobs` |
| Autosave and export of a 4K multi-layer doc | zero UI jank (no frame > 50 ms) | ORA encode on main thread every 30 s | `performance-background-jobs` |
| Cold start (no cache) → start screen interactive | ≤ 1.5 s mid laptop / 4G; warm ≤ 0.5 s | to measure | `performance-build-simd` |
| Memory, 4K × 10 layers with 50 undo steps | ≤ 1 GB | ≈ 330 MB per structural undo step | `performance-image-undo-memory` |

**Measured today** (native release, dev container): Image full compose 1024²×2 = 101 ms, 2048²×2 = 322 ms, 2048²×5 = 825 ms, 4096²×2 = 1 262 ms. Pixel compose 256² ≈ 0.13 ms (1 layer), 1.4 ms (4 layers) — STATUS.md "M12 baselines".

**Pieces.** Image bench `crates/pincel-image-core/benches/compose.rs` (new; mirror `crates/pincel-core/benches/compose.rs`, `criterion` is already a workspace dev-dependency). Browser script in its own directory, e.g. `ui/e2e-perf/`, not part of `pnpm test:e2e`; it uses the Frame-Time Probe (View ▸ Frame-Time Probe, i18n key `menu.probe`) and `performance.now()` around drag sequences. Pixel zoom minimum is 1 (`ui/src/modes/pixel/view-ops.ts`).

**Docs.** The plan adds a pincel.md §4.x "Performance budgets" with the table above and a Decision Log entry "budgets are contracts; regressions block" (see Open questions).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A scenario has no budget yet ("to measure") | The script reports it; the check skips it until a budget exists |
| A measured value exactly 10 % over budget | Check passes; more than 10 % fails (AC-04) |
| The perf script runs against a dev build | Not supported: like the e2e suite (CLAUDE.md §7.4), it runs against the built bundle |
| A bench scenario's median regresses but stays within budget | The check passes; the before → after number shows in this spec's `## Measurements` table |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Region compose, stroke and blur benches use 3840×2160 with 10 layers | The budget table defines 4K as 3840×2160 with 10 layers | Low |
| "Dab latency" is the time from the pointer event to the frame that shows its pixels | The budget reads "brush dab → pixels on screen" | Low |

## Open questions

- Which machine and browser is the "mid laptop" reference for the in-browser budgets, and which network profile is "4G" for the cold-start budget?
- What does the regression guard compare against before a budget is met for the first time — the budget (red from day one) or the last recorded numbers? And on CI runners, which are not the reference machine, does it compare with the budgets or with runner-specific baselines?
- Nightly schedule, manual dispatch, or both for the performance workflow?
- The Decision Log entry "budgets are contracts; regressions block" and the new pincel.md §4.x "Performance budgets" are not written yet — confirm and write them with this spec.
- No step of the plan targets "open a 4K PNG ≤ 1 s to first paint" — which spec owns meeting it?

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |

## Decisions

- Measure, change, measure: every performance commit states before → after on the benchmark it targets; no speculative tuning without a number (plan, 2026-10-28).
- Benchmarks and the browser script stay out of the 60 s PR gate; any unavoidable slow test is `#[ignore]` with a linked issue (CLAUDE.md §7.5).
- A budget is exceeded when the measured value is more than 10 % over it (plan, "Regression guard").
