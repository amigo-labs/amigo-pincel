---
status: todo
created: 2026-10-29
size: M
mode: both
design: [pincel.md §10.2, pincel.md §11.2, pincel.md §9.2, pincel.md §10.3]
---

# File access across hosts

## Context

Pincel runs in three hosts and must open and save documents in each: Chromium-based browsers with the File System Access API (pick a file, save back in place), other browsers without it (`<input type="file">` to open, a download to save), and the Tauri desktop app (native OS dialogs, reading and writing by path). One file layer hides the differences from both editors. On top of it sit the recent-files registry (re-open without a picker), opening by drag and drop on the start screen, and sizing a new image from the image on the clipboard.

Each mode saves its own project format — `.aseprite` in Pixel mode, OpenRaster (`.ora`) in Image mode (Decision Log 2026-10-11). This spec records the behavior as built on 2026-10-29.

## Scope

### In scope

- Open: FSA picker, `<input type="file">` fallback, Tauri native open dialog; accepted file types; cancel
- Save / Save As of the project file: in-place save through an FSA handle or a Tauri path, save picker, download fallback; cancel
- Save labels depending on in-place save support
- Recent-files registry: recording, order, cap, File ▸ Open Recent, the start screen's Recent list, re-open with permission check
- Opening a file by dropping it on the start screen
- New image size taken from an image on the clipboard (start screen card and New dialog)

### Out of scope

- Which editor a file opens in — because `app-shell` owns file → mode routing.
- What the codecs read and write (`.aseprite`, `.ora`, raster decode) — because `aseprite-round-trip` and `image-files` own them.
- Exports (PNG, sprite sheet, JPEG / WebP, `.gpl`) — because `pixel-export`, `image-files` and `pixel-palette` own them.
- Saving back to the original raster format — because it is the planned `save-original-format` spec.
- Pasting clipboard images into a document — because it is the planned `copy-cut-paste` spec.
- OS file association, open-file events and the native Open Recent submenu — because `desktop-app` owns the Tauri host integration.
- Autosave snapshots — because `autosave-recovery` owns them.

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses Open… and the host supports the File System Access API, then a file picker restricted to the supported document types opens (`.aseprite`, `.ase`, `.ora`, `.png`, `.jpg`, `.jpeg`, `.webp`, `.bmp`, `.gif`, `.tif`, `.tiff`).
- [ ] AC-02 (state): While the browser has no File System Access API, Open… uses the browser's file chooser with the same accepted extensions, and the chosen file opens.
- [ ] AC-03 (event): When the user cancels an open picker or dialog, then nothing changes and no error is shown.
- [ ] AC-04 (state): While the browser has no File System Access save picker, Save downloads the project file under the document's name (`.aseprite` in Pixel mode, `.ora` in Image mode) and the document then reads as saved.
- [ ] AC-05 (state): While a document was opened from, or already saved to, a file through the File System Access API or the desktop app, Save writes back to that file without a picker; Save As always asks for a location.
- [ ] AC-06 (event): When the user saves a never-saved document and a save picker is available, then the picker is offered with the document's name and the project type, and later Saves write to the chosen file.
- [ ] AC-07 (error): If writing back to the remembered file needs permission and the user denies it, then Save falls back to asking for a location.
- [ ] AC-08 (state): While in-place save is unavailable (no File System Access API, not the desktop app), Pixel mode's File menu labels Save as "Save (download)" and lists no "Save As…".
- [ ] AC-09 (state, manual: needs the desktop host): While running in the desktop app, Open… shows the OS open dialog with filters "All supported files", "Aseprite sprite", "OpenRaster image" and "Images", and Save / Save As show the OS save dialog filtered to the mode's project format; files are read and written by their path.
- [ ] AC-10 (event): When a document is opened or saved through a file handle (File System Access API) or a path (desktop app), then it is recorded in the recent-files registry with its name and mode; saving the same document again, or re-opening it through Pixel mode's File ▸ Open Recent, moves its entry to the top instead of adding a second one.
- [ ] AC-11 (always): The recent-files registry holds at most 8 entries, newest first; recording a ninth drops the oldest.
- [ ] AC-12 (state): While the registry is available but empty, File ▸ Open Recent is listed in both modes and disabled; while it is unavailable (neither File System Access API nor desktop app, or IndexedDB unavailable), the entry is absent.
- [ ] AC-13 (event): When the start screen shows and the registry holds entries, then a "Recent" list shows each file's name and its mode (Pixel / Image); activating an entry re-opens that file in the editor of its format.
- [ ] AC-14 (error): If re-opening a recent file needs read permission and the user denies it, then an error notice "<name>: permission denied" appears and nothing opens.
- [ ] AC-15 (event): When the user drops a file anywhere on the start screen, then it opens as if picked with Open…; while a file is dragged over the screen, the screen shows a highlighted drop state.
- [ ] AC-16 (error): If reading a picked, dropped or recent file fails, then a dismissable error notice names the failure and the start screen or open document stays as it was.
- [ ] AC-17 (event): When the New image card or the New dialog (in Image mode) opens and the clipboard holds an image the browser lets Pincel read without prompting, then Width and Height are prefilled with that image's size; the New pixel sprite card keeps 64 × 64.
- [ ] AC-18 (error): If the clipboard cannot be read without prompting or holds no image, then the New image fields keep 800 × 600.
- [ ] AC-19 (event): When the user has already changed Width or Height before the clipboard read finishes, then the typed values are kept.

## Technical notes

Hints, not contracts — paths drift; grep for the names.

- File layer `ui/src/lib/fs/index.ts`: `pickAndOpen(kind: PickKind = 'document')`, `saveBytes(bytes, target, { forceAs, format: 'aseprite' | 'ora' })`, `saveExport(...)`, `hasFsAccess()`, `ensureReadPermission()`, `ensureReadWritePermission()`, `sniffFormat()`, `formatFromName()`, `mimeFromName()`.
- `OpenedFile { name, bytes, mime, handle: FileSystemFileHandle | null, path: string | null }`; `SaveTarget { name, handle, path }`. `saveBytes` order: in-place via `handle.createWritable` (after `ensureReadWritePermission`) → `showSaveFilePicker` → Blob + anchor download. A cancelled picker (`AbortError`) returns the old target.
- Input fallback (`openViaInput`): settles `null` on the `cancel` event, or 300 ms after window focus returns without a `change`.
- Tauri: `@tauri-apps/plugin-dialog` `open` / `save` + Rust commands `read_file_bytes(path)` / `write_file_bytes(path, bytes)` (`src-tauri/src/main.rs`).
- Recents `ui/src/lib/idb/recent-files.ts`: object store `recent_files` (keyPath `id`, index `by_openedAt`), `MAX_RECENTS = 8`, `RecentFile { id, name, handle, path, addedAt, openedAt, mode }`; `id` is the editor's autosave `docId`. Rows without `mode` read back as `'pixel'`, without `path` as `null`. Availability gate: `(hasFsAccess() || isTauri()) && isIdbAvailable()`. Menu entry: `recentMenuItems()` in `ui/src/lib/components/layout/recent-menu.ts`.
- Recording: Pixel `recordRecent()` in `ui/src/modes/pixel/file-ops.ts`; Image `recordRecent()` in `ui/src/modes/image/ImageEditor.svelte`. Only targets with a handle or a path are recorded.
- Drop: `StartScreen.svelte` `onDragOver` / `onDrop` (first file only) → `App.svelte` `onDropFile`.
- Clipboard sizing: `clipboardImage({ onlyIfAllowed: true })` and `imageSize()` in `ui/src/lib/clipboard.ts`; `clipboardDocSize(mode)` in `ui/src/lib/shell/new-doc.ts` (returns `null` for Pixel mode).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| PNG opened in Pixel mode, then Save | Never writes to the PNG: the save picker (or download) uses the PNG's name with `.aseprite` |
| Clipboard image larger than the Image-mode limits | Prefilled size is clamped like a typed size |
| Recent entry recorded before the two-mode shell (no `mode`) | Listed as Pixel |
| Recent entry without handle and without path | Error notice "<name>: no handle / path" |
| Several files dropped at once | Only the first opens |
| Same file opened again through Open…, the start screen's Recent list or in Image mode | A second registry entry is added (entries are keyed by the editor session, not by the file) |
| Browser exposes the open picker but handles lack `createWritable` | Save falls through to the save picker or download |
| Firefox / Safari (no `clipboard-read` permission query) | Clipboard not read for sizing; defaults stay |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | | open | |
| AC-02 | `ui/e2e/ora.spec.ts › image mode: Save writes an OpenRaster file that reopens with its layers` | built | |
| AC-03 | | open | |
| AC-04 | | open | |
| AC-05 | | open | |
| AC-06 | | open | |
| AC-07 | | open | |
| AC-08 | | open | |
| AC-09 | | open | |
| AC-10 | | open | |
| AC-11 | | open | |
| AC-12 | | open | |
| AC-13 | | open | |
| AC-14 | | open | |
| AC-15 | | open | |
| AC-16 | | open | |
| AC-17 | `ui/e2e/clipboard-new.spec.ts › the New image card takes the clipboard image size`; `ui/e2e/clipboard-new.spec.ts › File ▸ New… takes the clipboard image size` | built | |
| AC-18 | `ui/e2e/smoke.spec.ts › start screen: the New cards open in place, one at a time` | built | |
| AC-19 | | open | |

Partial coverage (status stays `open`): AC-04 — `ora.spec.ts › image mode: Save writes an OpenRaster file that reopens with its layers` covers Image mode (`image.ora`, unsaved marker cleared); the Pixel `.aseprite` download is untested. AC-12 — the empty / disabled half is covered by `recents.spec.ts › pixel mode: File lists Open Recent, disabled while empty` and `› image mode: …`; the absent-when-unavailable half is not.

## Decisions

- 2026-10-29: Extracted from the code as built (reverse engineering); ACs describe current behavior.
