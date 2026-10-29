---
status: todo
created: 2026-10-28
size: L
mode: both
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
- Both modes, wherever these jobs exist

### Out of scope

- Interactive painting, layers and the viewport — because they need the document, which stays on the main thread
- wasm threads (`SharedArrayBuffer`, COOP/COEP) — because the worker needs no cross-origin isolation; threads are a separate stop-and-ask decision after the other steps are measured
- The native desktop backend for the same jobs — because `performance-desktop-jobs` adds it behind this interface

## Acceptance criteria

- [ ] AC-01 (always): Files written by export and autosave are byte-identical to those the main-thread encoding writes for the same document and settings.
- [ ] AC-02 (always): A document recovered from an autosave written in the worker equals the document at the moment that autosave started.
- [ ] AC-03 (state): While an autosave or export is encoding, the user can keep painting; strokes made meanwhile stay in the document and are not part of the file being written.
- [ ] AC-04 (state, manual: the budget is defined for the reference laptop): While autosave or export encodes a 3840×2160, 10-layer document, the browser performance script measures no frame longer than 50 ms.
- [ ] AC-05 (event, manual: the budget is defined for the reference laptop): When the Gaussian blur (radius 10) preview is requested on a 3840×2160 layer, then the full-resolution preview is shown within 500 ms and no frame in between is longer than 50 ms.
- [ ] AC-06 (event): When effect parameters change while a full-resolution preview is still computing, then only the result for the latest parameters is ever shown; a result for older parameters never replaces a newer one.
- [ ] AC-07 (event): When the user applies an effect, then the layer pixels are byte-identical to those of applying the same effect with the same parameters on the main thread.
- [ ] AC-08 (event): When the user cancels the effect dialog while a preview is computing, then the document is unchanged and the pending result is discarded.

## Technical notes

Hints from the legacy plan (2026-10-28); paths drift.

- Autosave: `ui/src/lib/idb/autosave-session.svelte.ts` (`AUTOSAVE_INTERVAL_MS = 30_000`), one session for both editors (Decision Log 2026-10-12). Export through `ui/src/lib/fs`. Effects: one wire format for both modes (`apply_effect` / `preview_effect`, `pincel_effects::EffectSpec`, Decision Log 2026-10-11); effect dialogs in `ui/src/lib/components/`.
- The worker holds its own wasm instance; the main thread hands it document bytes (ORA) or layer buffers as transferable `ArrayBuffer`s. No cross-origin isolation needed.
- Effect previews: a downscaled proxy first (instant), then the full-resolution result from the worker; cancel stale requests when parameters change.
- pincel.md §10.4 sketches Phase-2 workers via `comlink` with `SharedArrayBuffer` (COOP/COEP); the plan's design needs neither (see Open questions).
- Existing e2e: `ui/e2e/export.spec.ts`, `recovery.spec.ts`, `effects.spec.ts`, `ora.spec.ts` stay green.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Autosave timer fires while the previous autosave is still encoding | Open question (see below) |
| The document is closed while an export is encoding | Open question (see below) |
| Effect slider dragged continuously | Only the latest parameters' full result is shown (AC-06) |
| Tiny document (e.g. 16×16) | Same results; the job may finish within one frame |
| Export started twice in a row | Two files, each the document state at its own start |

## Open questions

- The Decision Log entry for the worker architecture is not written yet: message protocol, what stays on the main thread, and whether to follow pincel.md §10.4 (`comlink`, a new dependency, and `SharedArrayBuffer`) or plain `postMessage` with transferables as the plan proposes — confirm and write it with this spec.
- What happens if the worker fails or crashes during autosave, export or an effect preview: retry, fall back to the main thread, or show an error — and what does the user see?
- What happens when an autosave is due while the previous one is still encoding, and when the document is closed while an export is encoding?
- What resolution is the "instant" reduced-resolution effect preview, and is there a time it must appear within?

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
| AC-08 |      | open   |        |

## Decisions

- Heavy one-shot jobs (encoding, effects) leave the main thread; interactive painting stays, because it needs the document (plan, 2026-10-28).
- The worker needs no cross-origin isolation: data moves as transferable buffers (plan).
