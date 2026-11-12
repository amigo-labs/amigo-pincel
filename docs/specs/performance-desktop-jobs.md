---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.3, pincel.md §10.4, pincel.md §11.2]
depends_on: [performance-background-jobs]
---

# Performance: native job backend on desktop

## Context

The web build stays a full product. The desktop app can be faster where native code clearly wins: heavy one-shot jobs (full-resolution effects, export and encoding, autosave; later batch export and MCP) can run in the Tauri process on all cores with AVX2, without the cross-origin isolation wasm threads would need. Step 8 of the legacy plan `23-performance` (removed 2026-10-29, see git history) adds a native backend behind the job interface `performance-background-jobs` introduces.

Interactive editing (strokes, layers, viewport) stays in wasm in both builds: moving it into the Tauri process would push every presented frame through IPC (≈ 8 MB per 1080p frame, ≈ 500 MB/s at 60 fps), slower than in-webview wasm and a second rendering path.

## Scope

### In scope

- A native backend for the heavy jobs of `performance-background-jobs` in the desktop app, using all cores
- AVX2 in the native backend with runtime detection and a fallback
- Byte-identical results across both backends
- Keeping small jobs local, below a measured crossover size

### Out of scope

- Moving the document or interactive editing into the Tauri process — because every frame would cross IPC (see Context)
- Images beyond 64 Mpx (native documents or Memory64) — because the plan keeps it out until someone needs it; it is to be recorded as an open question in pincel.md §14
- A blanket `target-cpu=x86-64-v3` — because it would exclude pre-2013 x86 CPUs
- The web build — because it keeps the Web Worker backend unchanged

## Acceptance criteria

- [ ] AC-01 (always): For every job kind (full-resolution effect, export encoding, autosave encoding), the native backend's result is byte-identical to the Web Worker backend's for the same input.
- [ ] AC-02 (always): The native backend's results are byte-identical with and without AVX2 on the same input.
- [ ] AC-03 (option): Where the app runs as the desktop app and a job's input is at least the crossover size recorded under `## Decisions`, the job runs in the native backend; smaller jobs, and every job in the PWA, run in the Web Worker.
- [ ] AC-04 (event, manual: needs the desktop app on the reference machine): When a Gaussian blur (radius 10) is applied to a 3840×2160 layer in the desktop app, then it finishes faster in the native backend than in the Web Worker backend on the same machine.
- [ ] AC-05 (state, manual: needs the desktop app on the reference machine): While a native job runs on a 3840×2160, 10-layer document, no frame is longer than 50 ms.
- [ ] AC-06 (error): If a native job fails or the native command is unavailable, then the job runs in the Web Worker backend instead, without a message to the user, and the failure is logged; the user sees an error only if the Web Worker path fails too, with the same behavior as `performance-background-jobs`.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- A `JobRunner` with `run(job) -> Promise<result>`: web → Web Worker + wasm (SIMD, threads if ever accepted); desktop → a Tauri command into native Rust with rayon (all cores) and AVX2 runtime dispatch (`is_x86_feature_detected!("avx2")` with a scalar or SSE2 fallback). Apple Silicon / ARM use 128-bit NEON either way.
- Both backends call the same `pincel-effects` / codec functions; an equivalence test runs one job on both (AC-01). Expected AVX2 gain over SSE2 ≈ 1.3–2× in the kernel, less once memory-bound.
- `rayon` 1.x (MIT OR Apache-2.0) is an optional dependency behind a `parallel` feature of `pincel-effects` (and of the codec paths in `pincel-image-core`); only `pincel-tauri` enables the feature, so wasm builds never link it and the cores stay free of platform dependencies (Decision Log 2026-10-29).
- Tauri commands are thin wrappers (CLAUDE.md §5.5) in `src-tauri/src/`; CI checks `pincel-tauri` in its own job.
- Measure where the transfer cost (document or layer bytes in, result out) crosses over and record the size under Decisions.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| CPU without AVX2 | Fallback path, byte-identical result (AC-02) |
| Job just below the crossover size | Runs in the Web Worker |
| Native job fails or the command is unavailable | Silent fallback to the Web Worker, byte-identical result; error only if the worker path fails too (AC-06) |
| ARM / Apple Silicon desktop | Native backend without AVX2, byte-identical result |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

- AC-03 / AC-04 / AC-05: the loopback transport works (a `Blob` body from the fetch worker), but on the idle reference machine the SIMD worker is as fast as or faster than the native backend at every size (Decisions, 2026-11-12: Gaussian r 10 at 1024² 57 vs 109 ms, at 3840×2160 592 vs 606 ms; encodes slower natively throughout), so no crossover exists and nothing runs natively (`NATIVE_JOB_MIN_PIXELS` = `{ effect: null, encode: null }`). The 2026-11-11 measurement that found a 1 Mpx crossover for effects ran beside other builds: under CPU load the native backend's many cores win. Options: (a) drop the native backend and AC-02 – AC-05 (the worker stays the only backend); (b) keep it dormant and reword AC-03 – AC-05 for a machine under load or for heavier effects than the blur; (c) cut the transfer — send only the affected layer instead of the whole document, which matters most for multi-layer documents (the 10-layer 4K blur: worker 749 ms, native 8629 ms). Recommendation: (a), or (c) if desktop effect speed on large documents matters.

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/desktop-jobs.spec.ts` › "the Web Worker backend gives the recorded result for every job kind (AC-01)" and `src-tauri/src/native_jobs.rs` › `tests::run_matches_the_web_worker_backend_for_every_job_kind`: both backends against the same digests in `src-tauri/tests/fixtures/jobs/cases.json` (every Image effect's preview and region, every encode format); `crates/pincel-image-core/src/codec/ora.rs` › `to_ora_bytes_marks_every_entry_made_by_unix_on_every_platform`; `clippy.toml` of both cores disallows `std`'s float functions | built |        |
| AC-02 | `crates/pincel-effects/src/blur/mod.rs` › `dispatch_tests::axis_row_gives_the_same_bits_in_the_avx2_build`, `crates/pincel-effects/src/kernel.rs` › `dispatch_tests::row_3x3_gives_the_same_bits_in_the_avx2_build` (`cargo test -p pincel-effects --features parallel`); `src-tauri/src/native_jobs.rs` › `tests::run_matches_the_web_worker_backend_for_every_job_kind` (native with the AVX2 build on an AVX2 CPU against the worker, which has none) | built |        |
| AC-03 | `ui/e2e/native-jobs.spec.ts` › "jobs from the crossover size up run natively in the desktop app only (AC-03)", "without a crossover size every job of that kind runs in the worker (AC-03)"; `ui/e2e/desktop-jobs-routing.spec.ts` › "no job kind has a crossover size (AC-03)", "with the native endpoint reachable, an effect and an encode on a large document run in the worker (AC-03)", "every job in the PWA runs in the worker (AC-03)". Measured on the idle reference machine no size pays off against the SIMD worker (Decisions, 2026-11-12), so no job runs natively — blocked, see Open questions | open |        |
| AC-04 | `ui/e2e-perf/desktop-jobs.ts` (manual, desktop app): measured 2026-11-12 over the loopback endpoint on the idle reference machine, native 606 ms vs worker 592 ms — not met, see Open questions | open |        |
| AC-05 | `ui/e2e-perf/desktop-jobs.ts` (manual, desktop app): measured 2026-11-12, longest frame during a native Gaussian blur on a 3840×2160, 10-layer document 21.7 ms; native encodes of that document 230–264 ms (the main thread copies the 330 MB snapshot to the fetch worker) — not met for encodes, see Open questions | open |        |
| AC-06 | `ui/e2e/native-jobs.spec.ts` › "a failed or missing native job runs in the worker, logged, without an error (AC-06)", "a setup is loaded natively again after a native failure (AC-06)", "the user sees an error only if the worker path fails too (AC-06)" (the `nativeFirst` runner `ui/src/modes/image/jobs/index.ts` wraps the worker with; reached in the app only once a crossover size exists) | built  |        |

## Decisions

- Interactive editing stays in wasm in both builds; only heavy one-shot jobs get a native backend — per-frame IPC would be slower than in-webview wasm (plan, 2026-10-28).
- AVX2 only through runtime detection with a fallback, never through a blanket target CPU — older x86 CPUs stay supported (plan).
- 2026-10-29: `rayon` 1.x is approved as an optional dependency behind a `parallel` feature of `pincel-effects` (and the `pincel-image-core` codec paths), enabled only by `pincel-tauri`; one interactive engine (wasm) in both builds, a native job backend (rayon, AVX2 by runtime detection) on desktop for one-shot jobs, and the document never moves native (Decision Log 2026-10-29) — rayon is the standard std-only data-parallel crate, and the feature keeps the cores free of platform dependencies and the commands thin (owner accepted the recommendation).
- 2026-10-29: A failed native job falls back silently to the Web Worker and is logged; an error shows only if the worker path fails too — the results are byte-identical, and the user cares about the result, not which backend produced it (owner accepted the recommendation).
- 2026-11-11 (spec-build): The native backend is the Tauri command `image_job` (`src-tauri/src/native_jobs.rs`): the job is the UI's `ImageJob` without its snapshot, as JSON in the `pincel-job` request header; the snapshot is the raw request body; the result comes back as raw bytes (`tauri::ipc::Response`). The command is `async` and runs the job with `spawn_blocking`, so neither the webview nor the Tauri main thread waits. Effect sessions keep their document copy natively under the session's key, as the worker does (`load` / `drop`). Every job calls the `pincel_image_core::jobs` function the worker calls; TGA and ICO now go through `jobs::encode` too, so every encode format has one function.
- 2026-11-11 (spec-build): `rayon` runs the rows of the separable and 3×3 convolutions (`pincel-effects`, `par::for_each_row`) and the PNGs of an OpenRaster file (`pincel-image-core`, `par::map`) on all cores. Each row or PNG runs the same code with and without the `parallel` feature and keeps its place, so the bytes do not depend on it.
- 2026-11-11 (spec-build): The routing is `ui/src/lib/jobs/native.ts` (rune-free): `jobBackend(desktop, width, height)` picks the native backend in the desktop app for documents of at least `NATIVE_JOB_MIN_PIXELS` canvas pixels, once per encode and once per effect session; `nativeFirst` wraps the worker runner, so a native failure (an error, a missing command, an answer that is not bytes) logs `image jobs: native job failed, running it in the worker` and runs the same job through the worker runner with its own retry and main-thread fallback (AC-06). A failed native session copy is loaded again on the session's next job.
- 2026-11-11 (spec-build): AC-01's proof is split at shared fixtures, because Playwright cannot run the native backend and a native test cannot run the worker: `ui/e2e/desktop-jobs.spec.ts` runs every job case (every Image effect at its defaults plus Gaussian radius 10, preview and region, and every encode format) in the bundle's real job worker on `src-tauri/tests/fixtures/jobs/doc.snapshot` and compares FNV-1a digests with `cases.json` (`PINCEL_RECORD_JOB_DIGESTS=1` records them); the native side compares its results with the same file. `native_jobs::tests::fixture_snapshot_matches_its_recipe` ties the snapshot to the document recipe in code (`PINCEL_WRITE_JOB_FIXTURES=1` rewrites it).
- 2026-11-11 (spec-build): Fixed on the way: the zip crate stamps the host OS into every OpenRaster entry (DOS on Windows, Unix elsewhere), so a Windows-written ORA differed from the browser's in two bytes per entry; entries are now always marked Unix (`codec::ora::tests::to_ora_bytes_marks_every_entry_made_by_unix_on_every_platform`, red on Windows before the fix). The web output is unchanged.
- 2026-11-11 (spec-build): Red runs — the native comparison failed against a stub `NativeJobs::run` ("not implemented" for every case), then, implemented, only for Drop Shadow and the two ORA cases; the ORA fix removed the ORA cases, Drop Shadow is the float-math open question. `ui/e2e/native-jobs.spec.ts` and `desktop-jobs-routing.spec.ts` ran against a build whose `jobBackend` always answered "worker" and whose `nativeFirst` returned the worker runner: the native-routing and logged-fallback tests failed (with a 1 Mpx placeholder crossover, the routing e2e tests then passed through the mocked bridge, before the measurement removed the crossover); "just below the crossover size runs in the worker" and "the user sees an error only if the worker path fails too" passed there too (that behavior existed), kept as pins.
- 2026-11-11 (spec-build, superseded below by the loopback transport): Measured crossover over Tauri IPC: none. Every job size from 256² to 3840×2160 ran slower natively than in the worker, transfer included (table below), so `NATIVE_JOB_MIN_PIXELS` is `null` and the routing keeps every job in the worker until a faster transport exists (Open questions). `jobBackend` takes the threshold as a parameter, so the routing logic stays tested; the e2e tests that drove native routing through the mocked bridge went with it, and AC-06's fallback is tested on the `nativeFirst` runner itself.
- 2026-11-11 (spec-build): Measurements over Tauri IPC (reference machine Ryzen 7 PRO 6850H, release desktop app with release wasm, `node ui/e2e-perf/desktop-jobs.ts --reps 5`, median wall time in the page including the transfer, longest frame worker / native; other agents' builds ran alongside, so treat the absolute numbers as rough):

  | Scenario | Worker (ms) | Native (ms) | Longest frame worker / native (ms) |
  | --- | ---: | ---: | ---: |
  | Gaussian r 10, 256² | 11.9 | 37.3 | 12.3 / 16.8 |
  | PNG, 256² | 6.3 | 22.6 | 14.8 / 16.7 |
  | ORA, 256² | 14.7 | 24.6 | 17.2 / 16.7 |
  | Gaussian r 10, 512² | 35.1 | 100.4 | 17.1 / 50.3 |
  | PNG, 512² | 13.4 | 76.9 | 18.2 / 36.0 |
  | ORA, 512² | 32.5 | 84.5 | 18.6 / 33.4 |
  | Gaussian r 10, 724² | 72.8 | 226.3 | 18.7 / 66.9 |
  | PNG, 724² | 25.9 | 198.7 | 16.8 / 83.6 |
  | ORA, 724² | 61.1 | 208.9 | 20.7 / 116.9 |
  | Gaussian r 10, 1024² | 158.0 | 394.2 | 20.2 / 166.8 |
  | PNG, 1024² | 44.6 | 347.5 | 23.2 / 133.6 |
  | ORA, 1024² | 74.7 | 272.0 | 24.0 / 167.0 |
  | Gaussian r 10, 1448² | 297.9 | 838.9 | 17.1 / 417.3 |
  | PNG, 1448² | 97.1 | 1031.3 | 16.9 / 450.7 |
  | ORA, 1448² | 198.4 | 1124.1 | 16.9 / 434.0 |
  | Gaussian r 10, 2048² | 1165.8 | 2123.8 | 18.6 / 817.4 |
  | PNG, 2048² | 323.6 | 2663.9 | 16.9 / 1103.5 |
  | ORA, 2048² | 629.7 | 2108.9 | 17.1 / 1210.8 |
  | Gaussian r 10, 3840×2160 (AC-04) | 2784.4 | 5897.3 | 18.9 / 2281.6 |

  The 4K PNG / ORA rows did not finish: the page crashed after the 4K blur. Native compute alone (`cargo test --release`, no transfer, noise content): Gaussian r 10 on 3840×2160 531 ms; PNG 3840×2160 1.89 s; ORA 3840×2160 3.60 s with one layer, 6.98 s with ten (layers on all cores).
- 2026-11-11 (owner): Transport — the native backend takes its data over a loopback HTTP endpoint of the desktop app on `127.0.0.1` with a random bearer token, which the Web Worker `fetch`es (request body the snapshot, response the result), so the main thread never carries the bytes; a small `std::net` server under `src-tauri/` without a new dependency, refusing foreign `Origin`s and `Host`s like the MCP endpoint's DNS-rebinding guard. The crossover size is measured again over it. Settles the open question on Tauri IPC (Decision Log 2026-11-11).
- 2026-11-11 (spec-build, behind the next decision): wasm32 against x86_64-pc-windows-msvc, 200 000 inputs per function, `std` results differed for `exp` 8 285, `powf` 19 580, `ln` 7 865, `tan` 16 150, `atan2` 49 266, `hypot` 22 700, `cos` 60, `sin` 5, and in 74 of compose's 512 sRGB table entries; on the fixture only Drop Shadow differed (5 bytes, by 1–3). The `libm` crate 0.2.16 called natively matched wasm in every case.
- 2026-11-11 (owner): AC-01 — `libm` 0.2 is approved as a dependency of `pincel-effects` and `pincel-image-core`; their transcendental float calls go through it, so native output is bit-identical to wasm (Decision Log 2026-11-11).
- 2026-11-11 (owner): AC-02 — the AVX2 path uses the dispatch crate `multiversion` (MIT OR Apache-2.0), no `unsafe` of our own (Decision Log 2026-11-11).
- 2026-11-11 (spec-build): AC-01 with `libm` — every transcendental call of `pincel-effects` and `pincel-image-core` (25 sites; `sin_cos` as `sinf` and `cosf` separately, as wasm32 computes it) goes through `libm`, and a `clippy.toml` in each crate disallows `std`'s `f32` / `f64` float functions, so a new call fails the clippy gate. Red run: `run_matches_the_web_worker_backend_for_every_job_kind` failed on Windows for Drop Shadow's preview and region before the switch, green after; the worker's digests in `cases.json` stayed the same after rebuilding the wasm, so the browser's output did not change.
- 2026-11-11 (spec-build): AC-02 with `multiversion` 0.9 — the row kernels of the separable and 3×3 convolutions (`axis_row`, `row_3x3`) carry `#[multiversion(targets("x86_64+avx2+fma"))]` behind the `parallel` feature; each calls an `#[inline(always)]` body, which the tests also call directly for the build's own target and compare bit for bit. Rust never fuses a multiply and an add on its own, so the AVX2 / FMA build computes the same bits. The equality tests were green at once (the property holds by construction); their red run is a mutation: a `mul_add` added to the dispatched row only made `axis_row_gives_the_same_bits_in_the_avx2_build` fail. They need `--features parallel`, which the gates do not set; the native comparison test in `pincel-tauri` (CI job `tauri`) covers the AVX2 build end to end.
- 2026-11-11 (spec-build): The loopback transport (`src-tauri/src/job_server.rs`): `std::net` on `127.0.0.1` with a free port, one thread per connection, HTTP/1.1 with `Content-Length` bodies only (keep-alive, `Expect: 100-continue`), a 256-bit bearer token from `RandomState` keys (OS-seeded, the MCP endpoint's approach), and a `403` for any `Host` but `127.0.0.1:<port>` and any `Origin` but the app's own (`http://tauri.localhost`, `https://tauri.localhost`, `tauri://localhost`; a debug build also the Vite dev server). The page asks for the address once (`job_endpoint` command) and hands each job to a small fetch worker (`ui/src/lib/jobs/native-worker.ts`, no wasm); the snapshot is copied to it, so a fallback still has it. The CSP's `connect-src` allows `http://127.0.0.1:*`. The `image_job` IPC command is gone.
- 2026-11-11 (spec-build): WebView2 uploads a `fetch` body given as an `ArrayBuffer` / typed array at about 16 MB/s (32 MB in 2.07 s from a worker to a Node server on loopback; four or more parallel requests no faster), but a `Blob` body at about 200 MB/s (32 MB in 157 ms); downloads run at about 320 MB/s. The fetch worker therefore wraps the snapshot in a `Blob`.
- 2026-11-11 (spec-build): The crossover size is per job kind (`NATIVE_JOB_MIN_PIXELS` in `ui/src/lib/jobs/native.ts`): effect jobs (preview and apply of one panel session) run natively from 1 048 576 canvas pixels (1024²); encodes (save, export, autosave) have none and stay in the worker. Measured over the loopback transport (table below): Gaussian r 10 wins natively from about 512² (31 ms vs 34 ms) and clearly from 1024² (93 ms vs 148 ms; 3840×2160: 539 ms vs 1148 ms), so 1024² keeps a margin for effects cheaper than a blur; PNG and ORA never win (one layer: within ±10 % at every size; ten 4K layers: native 8.3 s / 11.2 s vs 2.0 s / 3.7 s in the worker), because the encoders run on one core either way and the whole snapshot still has to move. One size for both kinds would either send encodes native, slower, or keep effects in the worker, slower; AC-03's "the crossover size recorded" is read per kind.
- 2026-11-11 (spec-build): An effect session's document copy (`load`) moves to the fetch worker as a transfer, not a copy, because `JobSetup.req` makes a new one for every attempt, the worker fallback included; an encode's snapshot is copied, because its main-thread fallback reuses it.
- 2026-11-11 (spec-build): Measurements over the loopback transport with `Blob` uploads (reference machine, release app and release wasm, `node ui/e2e-perf/desktop-jobs.ts --reps 3`, median wall time in the page including the transfer, longest frame worker / native; other agents' builds and e2e runs on the machine, the e2e lock held during the run):

  | Scenario | Worker (ms) | Native (ms) | Longest frame worker / native (ms) |
  | --- | ---: | ---: | ---: |
  | Gaussian r 10, 256² | 8.3 | 16.5 | 10.6 / 14.2 |
  | PNG, 256² | 5.4 | 12.3 | 7.3 / 12.7 |
  | ORA, 256² | 13.4 | 14.9 | 16.1 / 10.5 |
  | Gaussian r 10, 512² | 34.0 | 31.2 | 16.7 / 16.8 |
  | PNG, 512² | 13.5 | 19.1 | 16.6 / 16.7 |
  | ORA, 512² | 26.0 | 27.9 | 16.7 / 16.8 |
  | Gaussian r 10, 724² | 62.5 | 59.5 | 16.8 / 16.8 |
  | PNG, 724² | 22.5 | 33.3 | 16.8 / 16.7 |
  | ORA, 724² | 52.1 | 51.5 | 16.8 / 16.7 |
  | Gaussian r 10, 1024² | 148.3 | 93.4 | 17.0 / 16.9 |
  | PNG, 1024² | 42.9 | 58.3 | 16.8 / 16.8 |
  | ORA, 1024² | 73.9 | 79.7 | 16.8 / 16.8 |
  | Gaussian r 10, 1448² | 255.3 | 154.5 | 17.3 / 16.9 |
  | PNG, 1448² | 80.6 | 109.6 | 17.4 / 16.8 |
  | ORA, 1448² | 142.5 | 151.0 | 17.1 / 17.0 |
  | Gaussian r 10, 2048² | 985.6 | 296.1 | 18.7 / 17.1 |
  | PNG, 2048² | 165.0 | 201.6 | 17.0 / 16.9 |
  | ORA, 2048² | 259.2 | 257.4 | 17.0 / 16.9 |
  | Gaussian r 10, 3840×2160 (AC-04) | 1148.0 | 538.5 | 36.0 / 17.4 |
  | PNG, 3840×2160 | 317.6 | 336.6 | 17.2 / 17.2 |
  | ORA, 3840×2160 | 482.4 | 465.6 | 17.4 / 34.0 |
  | ORA, 3840×2160, 10 layers (1 rep) | 3735.8 | 11178.0 | 250.1 / 282.2 |
  | PNG, 3840×2160, 10 layers (1 rep) | 2018.9 | 8340.2 | 245.2 / 262.7 |

  The 10-layer rows ran alone (the page ran out of memory when they followed the others); their 250 ms frames are the main thread copying the 330 MB snapshot to a worker, in both columns.
- 2026-11-12 (spec-build): the fetch worker and the per-kind routing described above (2026-11-11) were not on any branch, stash or worktree — only their tests, benchmark and these notes were (uncommitted in the old worktree). They were rebuilt from the tests: `ui/src/lib/jobs/native-worker.ts` (no wasm; posts `{ id, url, token, job, body }` to the endpoint with the body as a `Blob`), `nativeFetcher` in `ui/src/lib/jobs/native.ts` (one fetch worker, started at the first native job, no retries — a failure goes straight to `nativeFirst`'s worker fallback with the endpoint's status in the logged error), and `runNative` in `ui/src/modes/image/jobs/native.ts` (an effect session's copy moves to the fetch worker, an encode's snapshot is copied). The taken-over routing and fallback tests failed to load against the source before the rebuild (no per-kind thresholds) and passed after it.
- 2026-11-12 (spec-build): measured again on the idle reference machine (Ryzen 7 PRO 6850H, release app and release wasm, `node ui/e2e-perf/desktop-jobs.ts --reps 3`, median incl. transfer, worker / native ms): Gaussian r 10 256² 3.7 / 18.1, 512² 12.9 / 48.6, 724² 24.1 / 60.7, 1024² 57.3 / 109.0, 1448² 96.9 / 155.4, 2048² 254.0 / 274.9, 3840×2160 592.1 / 606.0, 3840×2160 with 10 layers 749.0 / 8628.8; PNG 3840×2160 316.1 / 488.0; ORA 3840×2160 520.6 / 654.6; 10-layer PNG / ORA 1877 / 9606 and 3651 / 9336. Longest frames stayed at 17–34 ms except the 10-layer encodes (230–268 ms in both columns: the snapshot copy to a worker). The worker is faster than in the 2026-11-11 table (1024² blur 57 vs 148 ms), which ran beside other builds; no size pays off natively, so `NATIVE_JOB_MIN_PIXELS` is `{ effect: null, encode: null }` and the routing tests that drove native effect jobs through the stand-in endpoint went with the 1 Mpx crossover; the in-app test now checks that a reachable endpoint gets no job (Open questions).
- 2026-11-12 (spec-build): `ui/e2e/desktop-jobs.spec.ts` read the worker script from `ui/dist/app/assets/`, the bundle path before the editor moved to the root (`pincel.md` §15, 2026-11-11); it reads `ui/dist/assets/` now.
