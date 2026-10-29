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

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- A `JobRunner` with `run(job) -> Promise<result>`: web → Web Worker + wasm (SIMD, threads if ever accepted); desktop → a Tauri command into native Rust with rayon (all cores) and AVX2 runtime dispatch (`is_x86_feature_detected!("avx2")` with a scalar or SSE2 fallback). Apple Silicon / ARM use 128-bit NEON either way.
- Both backends call the same `pincel-effects` / codec functions; an equivalence test runs one job on both (AC-01). Expected AVX2 gain over SSE2 ≈ 1.3–2× in the kernel, less once memory-bound.
- Tauri commands are thin wrappers (CLAUDE.md §5.5) in `src-tauri/src/`; CI checks `pincel-tauri` in its own job.
- Measure where the transfer cost (document or layer bytes in, result out) crosses over and record the size under Decisions.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| CPU without AVX2 | Fallback path, byte-identical result (AC-02) |
| Job just below the crossover size | Runs in the Web Worker |
| Native job fails or the command is unavailable | Open question (see below) |
| ARM / Apple Silicon desktop | Native backend without AVX2, byte-identical result |

## Open questions

- `rayon` is a new dependency of the native backend — approve it, and write the Decision Log entry the plan asks for when this step starts (one interactive engine (wasm) in both builds, a native job backend on desktop, and why the document does not move native).
- If a native job fails, should the job fall back to the Web Worker, or show an error — and what does the user see?

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |

## Decisions

- Interactive editing stays in wasm in both builds; only heavy one-shot jobs get a native backend — per-frame IPC would be slower than in-webview wasm (plan, 2026-10-28).
- AVX2 only through runtime detection with a fallback, never through a blanket target CPU — older x86 CPUs stay supported (plan).
