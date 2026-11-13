---
status: todo
created: 2026-10-28
size: L
mode: image
design: [pincel.md §10.3, pincel.md §10.4, pincel.md §15 (2026-10-12 autosave session), pincel.md §15 (2026-10-11 one effect wire format)]
depends_on: [performance-benchmarks]
---

# Performance: heavy jobs off the main thread

## Context

Everything in the editor runs on the main thread today: compose, effects, and the PNG / ORA / JPEG / WebP encoding for export and for autosave. Autosave encodes an ORA every 30 s, so on a large multi-layer image the UI freezes every 30 s, and an effect preview on a 4K image blocks input until it is done.

Step 4 of the legacy plan `23-performance` (removed 2026-10-29, see git history) moves these heavy one-shot jobs into a Web Worker with its own wasm instance. It owns two budgets from `performance-benchmarks`: autosave and export of a 4K multi-layer document cause no frame over 50 ms, and an effect preview (Gaussian r = 10) on 4K finishes in at most 500 ms without blocking the UI. Interactive painting stays on the main thread, because it needs the document.

## Scope

### In scope

- Autosave encoding and export encoding (PNG, ORA, JPEG, WebP) in a Web Worker
- Effect preview and effect apply at full resolution in the worker, with a fast reduced-resolution preview first and cancellation of stale requests
- Image mode; every acceptance criterion below is about Image mode

### Out of scope

- Interactive painting, layers and the viewport — because they need the document, which stays on the main thread
- wasm threads (`SharedArrayBuffer`, COOP/COEP) — because the worker needs no cross-origin isolation; threads are a separate stop-and-ask decision after the other steps are measured
- The native desktop backend for the same jobs — because `performance-desktop-jobs` adds it behind this interface (that spec was dropped 2026-11-13: no crossover, so this Web Worker is the only backend in both builds)
- Pixel mode's autosave (`.aseprite`), exports and effects — they stay on the main thread until a Pixel budget of pincel.md §4.5 misses, because Pixel sprites are small, their encodes take milliseconds, and a Pixel worker would need a second wasm instance (`pincel-wasm`) and a Pixel document snapshot; a missed Pixel budget reopens this as its own spec (owner, 2026-11-05)

## Acceptance criteria

- [ ] AC-01 (always): Image-mode files written by export and autosave are byte-identical to those the main-thread encoding writes for the same document and settings.
- [ ] AC-02 (always): An Image document recovered from an autosave written in the worker equals the document at the moment that autosave started.
- [ ] AC-03 (state): While an Image-mode autosave or export is encoding, the user can keep painting; strokes made meanwhile stay in the document and are not part of the file being written.
- [ ] AC-04 (state, manual: the budget is defined for the reference laptop): While autosave or export encodes a 3840×2160, 10-layer document, the browser performance script measures no frame longer than 50 ms.
- [ ] AC-05 (event, manual: the budget is defined for the reference laptop): When the Gaussian blur (radius 10) preview is requested on a 3840×2160 layer, then a reduced-resolution proxy (the canvas's on-screen size, at most 1024 px on the longest side) is shown within 100 ms, the full-resolution preview replaces it within 500 ms, and no frame in between is longer than 50 ms.
- [ ] AC-06 (event): When Image-mode effect parameters change while a full-resolution preview is still computing, then only the result for the latest parameters is ever shown; a result for older parameters never replaces a newer one.
- [ ] AC-07 (event): When the user applies an effect in Image mode, then the layer pixels are byte-identical to those of applying the same effect with the same parameters on the main thread.
- [ ] AC-08 (event): When the user cancels an Image-mode effect dialog while a preview is computing, then the document is unchanged and the pending result is discarded.
- [ ] AC-09 (error): If the worker fails or crashes during an Image-mode autosave, export or effect job, then the worker is restarted once and the job retried; if it fails again, the job runs on the main thread as before this feature and the failure is logged to the console. The user sees an error (the existing `status.failed` message) only if the main-thread run fails too.
- [ ] AC-10 (state): While an Image-mode autosave is encoding, no second autosave starts; an autosave tick that falls due meanwhile is skipped, and the next tick saves the latest document state.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- Autosave: `ui/src/lib/idb/autosave-session.svelte.ts` (`AUTOSAVE_INTERVAL_MS = 30_000`), one session for both editors (Decision Log 2026-10-12). Export through `ui/src/lib/fs`. Effects: one wire format for both modes (`apply_effect` / `preview_effect`, `pincel_effects::EffectSpec`, Decision Log 2026-10-11); effect dialogs in `ui/src/lib/components/`.
- The worker holds its own wasm instance; the main thread hands it document bytes (ORA) or layer buffers as transferable `ArrayBuffer`s. No cross-origin isolation needed.
- Effect previews: a downscaled proxy first, rendered on the main thread at the canvas's on-screen size capped at 1024 px on the longest side (≤ 100 ms, AC-05), then the full-resolution result from the worker; cancel stale requests when parameters change. The 100 ms proxy deadline sits next to the 500 ms full-preview budget of `performance-benchmarks`.
- Message protocol: plain `postMessage` with typed request / response messages carrying a job id; data moves as transferable `ArrayBuffer`s. No `comlink`, no `SharedArrayBuffer`, no COOP/COEP. This supersedes the `comlink` / `SharedArrayBuffer` sketch in pincel.md §10.4 (Decision Log 2026-10-29).
- Existing e2e: `ui/e2e/export.spec.ts`, `recovery.spec.ts`, `effects.spec.ts`, `ora.spec.ts` stay green.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Autosave timer fires while the previous autosave is still encoding | The tick is skipped; the next tick saves the latest state (AC-10) |
| The document is closed while an export is encoding | The export finishes writing the bytes captured at its start; a pending autosave of the closed document is discarded |
| Worker crashes twice in a row during an export | The export runs on the main thread (with a stutter) and writes the same file (AC-09) |
| Effect slider dragged continuously | Only the latest parameters' full result is shown (AC-06) |
| Tiny document (e.g. 16×16) | Same results; the job may finish within one frame |
| Export started twice in a row | Two files, each the document state at its own start |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | Image mode: `ui/e2e/background-jobs.spec.ts` › "PNG export from the worker is byte-identical to the main-thread encoding (AC-01, AC-09)", "WebP export …"; `crates/pincel-image-core/src/codec/snapshot.rs` › `snapshot_round_trip_encodes_byte_identical_ora`, `snapshot_round_trip_restores_the_document` | built |        |
| AC-02 | Image mode: `ui/e2e/background-jobs.spec.ts` › "a recovered worker autosave is the document as the autosave started (AC-02)" | built |        |
| AC-03 | Image mode: `ui/e2e/background-jobs.spec.ts` › "a stroke made while an export encodes stays in the document and out of the file (AC-03)" | built |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 | `ui/e2e/latest-only.spec.ts` › "a result for older parameters never replaces a newer one (AC-06)", "only the newest request’s failure is reported" | built |        |
| AC-07 | `crates/pincel-image-wasm/src/lib.rs` › `effect_region_tests::apply_effect_region_matches_apply_effect`; `ui/e2e/background-jobs.spec.ts` › "an effect applied in the worker is byte-identical to the main-thread apply (AC-07, AC-09)" | built |        |
| AC-08 | `ui/e2e/latest-only.spec.ts` › "cancel discards the running result and the waiting request (AC-08)"; `ui/e2e/background-jobs.spec.ts` › "cancelling the panel while a preview computes leaves the document unchanged (AC-08)" | built |        |
| AC-09 | Encode jobs: `ui/e2e/background-jobs.spec.ts` › "PNG export …", "WebP export … (AC-01, AC-09)" (worker script blocked: retried, encoded on the main thread, logged, no error shown). Effect jobs: "an effect applied in the worker is byte-identical to the main-thread apply (AC-07, AC-09)"; `ui/e2e/effect-panel-failure.spec.ts` › "image mode › a failing preview keeps the panel open with its values (AC-16)" (the error shows only when the main-thread run fails too) | built |        |
| AC-10 | `ui/e2e/background-jobs.spec.ts` › "an autosave tick due while one is still encoding is skipped (AC-10)" | built |        |

## Decisions

- Heavy one-shot jobs (encoding, effects) leave the main thread; interactive painting stays, because it needs the document (plan, 2026-10-28).
- The worker needs no cross-origin isolation: data moves as transferable buffers (plan).
- 2026-10-29: Heavy one-shot jobs run in one dedicated Web Worker with its own wasm instance, talking through typed `postMessage` requests / responses with job ids and transferable `ArrayBuffer`s; painting, layers and the viewport stay on the main thread; this supersedes the `comlink` / `SharedArrayBuffer` sketch of pincel.md §10.4 (Decision Log 2026-10-29) — no new dependency and no cross-origin isolation, and the protocol is a handful of message types (owner accepted the recommendation).
- 2026-10-29: A failed worker job is retried once in a restarted worker, then run on the main thread with a console log; the user sees an error only if that fails too — autosave and export must be correct even at the cost of a stutter, and a silent fallback is what the user needs (owner accepted the recommendation).
- 2026-10-29: At most one autosave is in flight and a tick due meanwhile is skipped; an export captures its bytes at start and finishes even if the document closes; a pending autosave of a closed document is discarded — no queue build-up, and every export is the state at its start (owner accepted the recommendation).
- 2026-11-05 (spec-build): The page hands the worker a **document snapshot**: new wasm API `snapshot_document(handle)` / `open_snapshot(bytes)` (`pincel_image_core::codec::snapshot`, uncompressed: JSON header plus raw layer and selection bytes). The snapshot is taken synchronously when the job starts, so edits made while the worker encodes are neither lost nor in the file (AC-03), and the worker encodes with the same `encodeHandle` function as the main-thread fallback (`ui/src/modes/image/jobs/encode.ts`), so the bytes are identical (AC-01). The snapshot is posted as a structured clone, not transferred, so a retry or the fallback can still use it.
- 2026-11-05 (spec-build): The job protocol is `ui/src/lib/jobs/runner.ts` (`jobRunner` on the page, `serveJobs` in the worker): `{ id, req }` requests, `{ id, ok, res | error }` responses, results transferred. A failed job (an error answer, a worker `error` event, an unreadable message) terminates the worker, retries once in a fresh one, then runs the fallback with a `console.error` (AC-09). Save, Save As, Export (PNG / JPEG / WebP) and the Image autosave go through `encodeInBackground`; a save marks the undo depth at the job's start as saved.
- 2026-11-05 (spec-build): Measured on the reference machine (release wasm, `pnpm perf`, back to back): `image-autosave-export-4k` longest frame 1766.6 / 1766.6 ms → 300.0 / 416.7 ms, frame p95 16.8 ms both. AC-04's 50 ms budget is not met yet: the main thread still copies the 4K, 10-layer document (≈ 330 MB) into the snapshot and the structured clone. Candidates for a follow-up: copy layer buffers straight into transferable `ArrayBuffer`s instead of one snapshot `Vec`, or snapshot per layer across frames.
- 2026-11-05 (spec-build): Red runs — AC-10's test wrote two snapshots with the skip removed from `AutosaveSession.tick`; AC-01 / AC-09 / AC-02 / AC-03 tests are new with the worker (before it, no worker ran and nothing was logged).
- 2026-11-05 (fix): The job worker instantiates the core the page has already compiled (`compiledCore()`, sent as the runner's `JobInit` message ahead of the first job) instead of fetching and compiling the 12 MB core itself. The worker's own cold start had put 1–2.5 s between an autosave tick and its IndexedDB write under e2e load, so the recovery tests' reload 500 ms after the tick lost the snapshot. Without a compiled module (or if instantiating it fails) the worker still loads the core itself. First autosave latency (tick → `autosave_meta` row, 4 e2e workers): 945–2566 ms → 73–95 ms. Regression test: `ui/e2e/background-jobs.spec.ts` › "the job worker reuses the page’s compiled core instead of fetching it again".
- 2026-11-05 (owner): Pixel-mode autosave and exports stay on the main thread until a Pixel budget misses; the spec covers Image mode only (Scope). Pixel encodes take milliseconds on sprites, and a Pixel worker costs a second wasm instance and a document snapshot. Settles the Open question of 2026-11-05.
- 2026-11-06 (spec-build): Effect jobs run in the same job worker as the encodes, on a document copy the worker keeps for one effect panel session: the runner's new `JobSetup` (`ui/src/lib/jobs/runner.ts`) loads a `snapshotDocument` copy into a worker that lacks it, ahead of the session's first job and again after a restart, and `release` frees it when the panel closes. The page copies the document once per session, not per preview; the panel keeps the editor inert, so the copy stays the document's state.
- 2026-11-06 (spec-build): Apply is split in two (new wasm API `effect_region_bytes` / `apply_effect_region`): the worker computes the effect's region on its copy and the page commits it as the same `SetPixels` undo step `apply_effect` makes — the same `effect_region` function on byte-identical data, so the pixels match the main-thread apply (AC-07). A panel cancelled or replaced while the region computes commits nothing (AC-08).
- 2026-11-06 (spec-build): Previews go through `latestOnly` (`ui/src/lib/jobs/latest.ts`): at most one preview job runs, a request made meanwhile waits and replaces any earlier waiting one, and a result or error is shown only if no newer request or cancel came after it (AC-06, AC-08). The scheduler is rune-free and tested from a Node-side Playwright spec, because Playwright cannot time a worker result against a slider deterministically.
- 2026-11-06 (spec-build): `effect-panel-failure.spec.ts` forced its preview failure by patching the page's wasm exports; previews now run in the worker, which an init script cannot reach, so the spec blocks the worker script and the preview fails on its main-thread fallback — the path where AC-09 shows the error. Its patch also accepts the `Instance` that `WebAssembly.instantiate` returns for the page's precompiled module.
- 2026-11-06 (spec-build): Red runs — AC-07 / AC-09's e2e test failed on the main-thread effect path ("the effect fallback is logged": no worker ran for the effect); the `latestOnly` tests for AC-06 / AC-08 failed against a version that showed every result; `apply_effect_region_matches_apply_effect` failed against an `effect_region_bytes` stub. The AC-08 e2e test passed on the main-thread path too (a synchronous preview cannot outlive a cancel), kept as a pin.
- 2026-11-06 (spec-build): Not built yet: AC-05's reduced-resolution proxy. The first preview of a session also copies the document on the main thread (the same ≈ 300 ms on a 4K, 10-layer document as AC-04's snapshot), so AC-05's "no frame longer than 50 ms" waits on the same snapshot follow-up as AC-04.
- 2026-10-29: The instant effect preview is a main-thread proxy at the canvas's on-screen size, capped at 1024 px on the longest side, shown within 100 ms on the reference laptop and replaced by the full-resolution result — anything finer than screen resolution isn't visible, and 100 ms reads as immediate (owner accepted the recommendation).
- 2026-11-12 (perf): AC-05's full-resolution Gaussian preview was compute-bound (≈ 2.8 s in the wasm worker on a 3840×2160 layer): `convolve_axis` and `convolve_3x3` (`crates/pincel-effects`) clamped and bounds-checked every tap of every pixel. Both now compute one output row at a time (`axis_row`, `conv3x3_row`); only pixels whose taps leave the image clamp, and the rest add contiguous runs of floats tap by tap into a 32-float accumulator tile, which vectorises. Each output float still starts at 0.0 and adds `src * weight` in kernel order, so every effect's output stays bit-identical (pinned by `convolve_axis_matches_reference_bit_for_bit` and `convolve_3x3_matches_reference_bit_for_bit` against the frozen old functions). Native criterion bench on the reference laptop, `image_effect/gaussian_r10_4k` median: 1372.8 ms → 375.2 ms, 322.4 ms in a second run on the final per-row code (another checkout ran tests on the machine meanwhile; much of the remainder is first-touching the fresh 133 MB f32 buffers, ≈ 80–100 ms each natively, which a wasm heap that has grown once does not pay again). In the browser (release bundle, `pnpm perf -g "brush, pan"`, `image-gaussian-4k`, two runs with the new code, then two with the old one on the same machine, so drift between the pairs is not excluded): applying the blur (`full`, OK until the panel closes) 1396 / 1359 ms → 820 / 624 ms, still over AC-05's 500 ms; the first preview draw (`proxy`) 2424 / 2346 ms → 2568 / 1464 ms, within the noise of these runs and dominated by what AC-05's main-thread proxy and the snapshot follow-up of AC-04 address, not by the kernel. The digests of `ui/e2e/job-digests.spec.ts` stay the same with the new kernels in the wasm SIMD build, so the job worker's outputs are byte-identical end to end.
