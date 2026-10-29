---
status: todo
created: 2026-10-29
size: M
mode: both
design: [pincel.md §10.1, pincel.md §10.3]
---

# Autosave, recovery, preferences and offline PWA

## Context

A browser tab can crash, be closed or be reloaded with unsaved work in it. Pincel snapshots an edited, unsaved document into IndexedDB every 30 seconds and, the next time an editor of that mode opens in a new page load, offers the snapshots in a Recovery dialog. Both modes run the same autosave session; Pixel mode stores `.aseprite` bytes, Image mode OpenRaster bytes.

IndexedDB also holds the user's preferences (language, panel layout, startup document, last colors, desktop file-association prompt), so the UI never touches `localStorage` (AGENTS.md §9). The web build is an installable PWA whose service worker precaches the app shell and both wasm cores so the app loads offline; documents are never cached by the service worker. This spec records the behavior as built on 2026-10-29.

## Scope

### In scope

- Periodic autosave of unsaved documents in both modes; when snapshots are written and dropped
- Recovery dialog: listing, Recover, Discard, Not now; per-row errors
- Preference persistence infrastructure (IndexedDB `prefs` store) and the Pixel-mode last foreground / background colors
- Behavior without IndexedDB
- Service worker precache, offline start, web app manifest

### Out of scope

- The individual prefs' features: language (`i18n`), panel layout and startup document (`app-shell`), recent files (`file-access`), file-association prompt (`desktop-app`) — because each owning spec covers its own behavior; this spec covers only last colors and the shared store.
- Autosave in the desktop app beyond what the shared web code does — because `desktop-app` owns the Tauri host.
- Caching documents in the service worker — because documents live in IndexedDB (pincel.md §10.1).
- Multi-tab coordination of autosave — because no such behavior exists today.

## Acceptance criteria

- [ ] AC-01 (state): While a document has edits that are not saved, the editor writes a snapshot of it to browser storage every 30 seconds, in both modes; a reload after that offers it for recovery.
- [ ] AC-02 (always): A document whose undo depth equals its depth at the last save or open is never snapshotted, and a document that did not change since its last snapshot is not written again.
- [ ] AC-03 (event): When the user saves a document, then its snapshot is deleted and it is no longer offered for recovery.
- [ ] AC-04 (event): When an editor of a mode mounts for the first time in a page load and snapshots of that mode exist, then the "Recover unsaved work?" dialog lists each document's latest snapshot with its name, time and size, newest first; snapshots of the other mode are not listed.
- [ ] AC-05 (event): When the user clicks Recover on a listed snapshot, then that document replaces the open one, the dialog closes and the document reads as unsaved (unsaved marker shown).
- [ ] AC-06 (state): While a recovered document has not been saved, its snapshot stays, so closing the tab right after recovering loses nothing.
- [ ] AC-07 (event): When the user clicks Discard on a listed snapshot, then that snapshot is deleted for good; discarding the last listed snapshot closes the dialog.
- [ ] AC-08 (event): When the user clicks "Not now", then the dialog closes, is not shown again for the rest of the page load (also not after File ▸ New…), and the snapshots are offered again after the next reload.
- [ ] AC-09 (event): When the user leaves an edited document through "Discard changes" (File ▸ New… or Open…), then its snapshot is deleted and it is offered neither in this page load nor after a reload.
- [ ] AC-10 (error): If recovering a snapshot fails (missing or unreadable bytes), then the error is shown on that snapshot's row and the dialog stays open.
- [ ] AC-11 (error): If writing a snapshot fails, then the status line (Pixel) or error notice (Image) reports "Autosave failed: …" and editing continues.
- [ ] AC-12 (event): When the user changes the foreground or background color in Pixel mode and reloads, then a new Pixel document starts with those colors.
- [ ] AC-13 (error): If IndexedDB is unavailable, then the app still starts and both editors create, edit and save documents; autosave, recovery, recent files and stored preferences are silently skipped.
- [ ] AC-14 (event): When the web app has been loaded once and the network then goes offline, then reloading it shows the start screen and both editors open new documents.
- [ ] AC-15 (always): The web build serves a web app manifest named "Pincel" with `display: standalone`, whose start URL and scope are the app's base path (`/app/` on the web).

## Technical notes

Hints, not contracts — paths drift; grep for the names.

- Session `ui/src/lib/idb/autosave-session.svelte.ts`: `AUTOSAVE_INTERVAL_MS = 30_000`; `class AutosaveSession` with `start()`, `tick()`, `opened(id?)`, `saved()`, `recover(meta)`, `discard(id)`, `dismiss()`, `close()`; module-level `probedModes` makes the probe run once per mode per page load. Editors supply an `AutosaveHost { mode, depth, savedDepth, busy, name, encode, open, report }` — Pixel in `ui/src/modes/pixel/file-ops.ts` (`initAutosave`), Image in `ui/src/modes/image/ImageEditor.svelte`. A recovered Pixel document sets `ed.lastSavedUndoDepth = -1`; Image calls `markUnsaved()`.
- `close()` on editor unmount deletes the open document's snapshots; a page unload never unmounts, so a crash or closed tab keeps them.
- Storage `ui/src/lib/idb/db.ts`: database `pincel`, `DB_VERSION = 2`, stores `prefs` (keyPath `key`), `recent_files` (keyPath `id`, index `by_openedAt`), `autosave_snapshots` (keyPath `['docId', 'ts']`, `{ docId, ts, bytes }`), `autosave_meta` (keyPath `['docId', 'ts']`, `{ docId, ts, name, byteLength, mode? }`). The v1 → v2 upgrade backfills `autosave_meta`. One snapshot per document: `writeSnapshot` deletes the `docId` range before its puts in one transaction. The probe reads only `autosave_meta`; bytes load on Recover.
- `ui/src/lib/idb/autosave.ts`: `writeSnapshot`, `latestSnapshot`, `listLatestSnapshots(mode?)`, `removeSnapshots`; rows without `mode` count as `'pixel'`.
- Dialog `ui/src/lib/components/RecoveryDialog.svelte` (presentational; title key `recovery.title`).
- Prefs `ui/src/lib/idb/prefs.ts`: `getPref`, `setPref`, `removePref`. Keys in use: `language`, `layout:pixel`, `layout:image`, `autoNew`, `lastColor`, `lastBgColor` (Pixel, `#rrggbb` validated on load), `fileAssocPromptShown`.
- Service worker `ui/src/sw.ts`: `precacheAndRoute(self.__WB_MANIFEST)`, `skipWaiting()` on install, `clients.claim()` on activate. Config `ui/vite.config.ts`: `VitePWA({ strategies: 'injectManifest', registerType: 'autoUpdate', injectRegister: 'auto', … })`, `globPatterns: ['**/*.{js,css,html,wasm,svg,webmanifest,ttf}']`, `maximumFileSizeToCacheInBytes: 16 * 1024 * 1024`; `base` is `/app/` on the web and `/` under Tauri.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| New / Open swaps the document while a snapshot write is in flight | The new document's baseline is not overwritten by the stale write |
| A file operation (save, open) is running at tick time | The tick is skipped |
| Recover clicked twice quickly | Only the first runs |
| Stored color pref is not a `#rrggbb` string | Ignored; default colors stay |
| Two snapshot rows for one document left by a partial write | Only the newest is listed |
| Second editor mount of the same mode in a page load (after New / Open) | No Recovery dialog |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/recovery.spec.ts › pixel: an autosaved edit is offered and recovered after a reload`; `ui/e2e/recovery.spec.ts › image: an autosaved edit is offered and recovered after a reload` | built | |
| AC-02 | | open | |
| AC-03 | | open | |
| AC-04 | | open | |
| AC-05 | `ui/e2e/recovery.spec.ts › pixel: an autosaved edit is offered and recovered after a reload`; `ui/e2e/recovery.spec.ts › image: an autosaved edit is offered and recovered after a reload` | built | |
| AC-06 | | open | |
| AC-07 | | open | |
| AC-08 | | open | |
| AC-09 | `ui/e2e/recovery.spec.ts › a discarded document is not offered again, on New or after a reload` | built | |
| AC-10 | | open | |
| AC-11 | | open | |
| AC-12 | | open | |
| AC-13 | | open | |
| AC-14 | | open | |
| AC-15 | | open | |

Partial coverage (status stays `open`): AC-04 — the dialog title and Recover button are asserted by the recovery tests, the listed name / time / size, the ordering and the mode filter are not. AC-07 — `recovery.spec.ts › discarding the last snapshot closes the dialog` asserts the dialog closing, not that the snapshot is gone after a reload. AC-08 — `recovery.spec.ts › "Not now" keeps the dialog away for the rest of the page load` covers the page-load half; re-offering after a reload is untested. AC-09 is marked `built` on the File ▸ New… path in Pixel mode; the Open… path and Image mode share the mechanism but have no test of their own.

## Decisions

- 2026-10-29: Extracted from the code as built (reverse engineering); ACs describe current behavior.
