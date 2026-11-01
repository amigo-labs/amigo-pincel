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
- Dropping files onto an open editor: add as layer or open as document, with a remembered choice (added 2026-10-30)
- File drops in the desktop app (added 2026-10-30)
- New image size taken from an image on the clipboard (start screen card and New dialog)

### Out of scope

- Which editor a file opens in — because `app-shell` owns file → mode routing.
- What the codecs read and write (`.aseprite`, `.ora`, raster decode) — because `aseprite-round-trip` and `image-files` own them.
- Exports (PNG, sprite sheet, JPEG / WebP, `.gpl`) — because `pixel-export`, `image-files` and `pixel-palette` own them.
- Saving back to the original raster format — because it is the planned `save-original-format` spec.
- Pasting clipboard images into a document — because the `copy-cut-paste` spec owns it; a dropped layer reuses its placement rules.
- Placing a dropped layer at the drop position — because a paste lands at the viewport top-left (`copy-cut-paste` AC-09) and the drop reuses that path; drop-at-cursor is a later refinement.
- PWA file handlers, `launchQueue` and share targets — because they are OS integration of the installed web app, a separate feature (backlog).
- OS file association, open-file events and the native Open Recent submenu — because `desktop-app` owns the Tauri host integration; file drops onto the window belong here (AC-29), because they share the drop path of the browser.
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
- [ ] AC-20 (event): When the user drops only raster image files (PNG, JPEG, WebP, BMP, GIF, TIFF; the format told by content, else by extension, as for Open…) anywhere on an open editor whose document is loaded, then a dialog asks "Add as layer", "Open as document" or "Cancel" and offers a checkbox "Don't ask again"; Cancel changes nothing and remembers nothing.
- [ ] AC-21 (event): When the user chooses "Add as layer", then each dropped file becomes a new layer above the active layer, the first dropped file lowest and the last on top, placed and sized as a paste of another image (`copy-cut-paste` AC-09, AC-10: for each file larger than the canvas the larger-image dialog asks, and its Cancel skips only that file), one undo step per file.
- [ ] AC-22 (state): While the open editor is in Pixel mode and any dropped raster file is not a PNG, "Add as layer" is disabled for the whole drop and the dialog shows "Pixel mode adds PNG files only".
- [ ] AC-23 (event): When the user chooses "Open as document", then the first dropped file opens as a dropped file opens on the start screen (AC-15), after the unsaved-changes guard of File ▸ Open…; cancelling the guard changes nothing; when more than one file was dropped, a notice says that only the first was opened.
- [ ] AC-24 (event): When a drop onto an open editor contains an `.aseprite`, `.ase` or `.ora` file, then the first of them opens as a document as in AC-23, without the dialog; the other files of that drop are not added, and a notice says that only that file was opened.
- [ ] AC-25 (event): When the user confirms the dialog with "Add as layer" or "Open as document" while "Don't ask again" is ticked, then later raster drops run that action without the dialog, also after a restart; File ▸ Dropped Images in both modes offers "Ask", "Add as Layer" and "Open as Document", marks the current choice and changes it.
- [ ] AC-26 (error): If a dropped file is in no supported format, or reading or decoding it fails, then a notice names it and nothing is done with it; the other supported files of the same drop proceed, and layers already added stay.
- [ ] AC-27 (state): While files are dragged over an open editor whose document is loaded and no modal dialog is open, the editor shows an overlay reading "Drop to add or open"; it disappears when the drag leaves the window or ends.
- [ ] AC-28 (state): While the remembered choice is "Add as Layer" and "Add as layer" is not possible for a drop (AC-22), the dialog shows instead of the remembered action.
- [ ] AC-29 (event, manual: needs the desktop app): When the user drops files onto the desktop app's start screen or an open editor, then they behave as in the browser (AC-15, AC-20 – AC-28), and a document opened by the drop is recorded in the recent files and saves as a document opened by path does (AC-05; a raster file follows `save-original-format`).

## Technical notes

Hints, not contracts — paths drift; grep for the names.

- File layer `ui/src/lib/fs/index.ts`: `pickAndOpen(kind: PickKind = 'document')`, `saveBytes(bytes, target, { forceAs, format: 'aseprite' | 'ora' })`, `saveExport(...)`, `hasFsAccess()`, `ensureReadPermission()`, `ensureReadWritePermission()`, `sniffFormat()`, `formatFromName()`, `mimeFromName()`.
- `OpenedFile { name, bytes, mime, handle: FileSystemFileHandle | null, path: string | null }`; `SaveTarget { name, handle, path }`. `saveBytes` order: in-place via `handle.createWritable` (after `ensureReadWritePermission`) → `showSaveFilePicker` → Blob + anchor download. A cancelled picker (`AbortError`) returns the old target.
- Input fallback (`openViaInput`): settles `null` on the `cancel` event, or 300 ms after window focus returns without a `change`.
- Tauri: `@tauri-apps/plugin-dialog` `open` / `save` + Rust commands `read_file_bytes(path)` / `write_file_bytes(path, bytes)` (`src-tauri/src/main.rs`).
- Recents `ui/src/lib/idb/recent-files.ts`: object store `recent_files` (keyPath `id`, index `by_openedAt`), `MAX_RECENTS = 8`, `RecentFile { id, name, handle, path, addedAt, openedAt, mode }`; `id` is the editor's autosave `docId`. Rows without `mode` read back as `'pixel'`, without `path` as `null`. Availability gate: `(hasFsAccess() || isTauri()) && isIdbAvailable()`. Menu entry: `recentMenuItems()` in `ui/src/lib/components/layout/recent-menu.ts`.
- Recording: Pixel `recordRecent()` in `ui/src/modes/pixel/file-ops.ts`; Image `recordRecent()` in `ui/src/modes/image/ImageEditor.svelte`. Only targets with a handle or a path are recorded.
- Drop: `StartScreen.svelte` `onDragOver` / `onDrop` (first file only) → `App.svelte` `onDropFile`. In the Tauri app on Windows the webview's native drag-drop handling (on by default in Tauri 2) can keep HTML5 file drops from reaching the page; Tauri reports drops with paths through `getCurrentWebview().onDragDropEvent` (`@tauri-apps/api/webview`, already a dependency), and `read_file_bytes` reads a path.
- Layer import: Pixel `runPaste` / `beginPaste` (`ui/src/modes/pixel/clipboard-ops.ts`, PNG only — `pincel-core` decodes PNG), Image `beginPaste` (`ImageEditor.svelte`, any raster format the image core decodes). Remembered choice: an IndexedDB pref like `autoNew` (`ui/src/lib/shell/auto-new.svelte.ts`).
- Clipboard sizing: `clipboardImage({ onlyIfAllowed: true })` and `imageSize()` in `ui/src/lib/clipboard.ts`; `clipboardDocSize(mode)` in `ui/src/lib/shell/new-doc.ts` (returns `null` for Pixel mode).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| PNG opened in Pixel mode, then Save | Never writes to the PNG: the save picker (or download) uses the PNG's name with `.aseprite` |
| Clipboard image larger than the Image-mode limits | Prefilled size is clamped like a typed size |
| Recent entry recorded before the two-mode shell (no `mode`) | Listed as Pixel |
| Recent entry without handle and without path | Error notice "<name>: no handle / path" |
| Several files dropped at once on the start screen | Only the first opens |
| A drop onto an open editor while a modal dialog is open, or before its document is loaded | Ignored, no drop overlay — because the dialog owns the input and an unloaded editor has no layer to add to |
| A drag of text or a link (no files) over an open editor | No drop state, nothing happens |
| Raster and project files dropped together onto an open editor | The first project file opens (AC-24) — because a project replaces the document, so adding layers to the old one first would be thrown away |
| A folder, or a zero-byte file, dropped | AC-26: a notice names it |
| The remembered choice cannot be stored (IndexedDB unavailable) | It holds for the session only |
| A dropped file that is larger than the mode's limits as a layer | The paste's larger-image handling (`copy-cut-paste` AC-10 and its `paste.tooLarge` edge case) |
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
| AC-20 | | open | |
| AC-21 | | open | |
| AC-22 | | open | |
| AC-23 | | open | |
| AC-24 | | open | |
| AC-25 | | open | |
| AC-26 | | open | |
| AC-27 | | open | |
| AC-28 | | open | |
| AC-29 | | open | |

Partial coverage (status stays `open`): AC-04 — `ora.spec.ts › image mode: Save writes an OpenRaster file that reopens with its layers` covers Image mode (`image.ora`, unsaved marker cleared); the Pixel `.aseprite` download is untested. AC-12 — the empty / disabled half is covered by `recents.spec.ts › pixel mode: File lists Open Recent, disabled while empty` and `› image mode: …`; the absent-when-unavailable half is not.

## Decisions

- 2026-10-29: Extracted from the code as built (reverse engineering); ACs describe current behavior.
- 2026-10-30: Revision — dropping files onto an open editor (AC-20 – AC-28) and file drops in the desktop app (AC-29), on the owner's request to hand image files to Pincel; the owner chose a dialog with a "Don't ask again" checkbox over a fixed action.
- 2026-10-30: The remembered choice is changed under File ▸ Dropped Images — because Tauri drops carry no modifier keys, so a modifier cannot bring the dialog back, and Pincel has no settings dialog yet.
- 2026-10-30: Pixel mode adds only PNG files as layers (AC-22) — because `pincel-core` decodes PNG only, and decoding through a canvas would read pixels back from it (AGENTS.md §9).
- 2026-10-30: "Open as document" opens only the first file — because the shell holds one document at a time (§9.2).
- 2026-10-30: The desktop app takes drops through Tauri's native drag-drop event with file paths (Decision Log 2026-10-30) — because a path lets Save write back to the dropped file and Open Recent find it again.
- 2026-10-30: Clarification — the dialog with "Don't ask again" is the owner's choice because they want to decide per drop until a habit settles; dialog buttons are written in sentence case, the File ▸ Dropped Images menu items in title case like every menu item.
- 2026-10-30: Browser drops open without a file handle, as on the start screen today, so they save through the picker or a download and are not recorded in the recent files — because the drop path reads `File` objects; `DataTransferItem.getAsFileSystemHandle()` could add a handle later.
