---
status: todo
created: 2026-10-29
size: M
mode: shell
design: [pincel.md §9.1, pincel.md §9.2, pincel.md §9.4]
---

# App shell and shared editor frame

## Context

Pincel is two editors in one app: Pixel mode (`pincel-core`, `.aseprite`) and Image mode (`pincel-image-core`, OpenRaster and raster images). A thin shell decides which one runs: it owns the start screen, the New-document dialog, routing an opened file to the editor its format belongs to, and the unsaved-changes guard on its own entry points. It mounts exactly one editor per session, so the two keyboard maps never overlap and a session only downloads the wasm core it uses.

Both editors render through one shared frame: top bar with menus described as data, tool rail, options bar, canvas, docked / drawer / bottom-sheet panels and a status bar, restructured by window width. One keyboard map serves both modes.

Already decided: "Two cores, one shell" (Decision Log 2026-09-06); "Open like this on startup" (Decision Log 2026-10-28). This spec records the behavior as built on 2026-10-29.

## Scope

### In scope

- Start screen: New pixel sprite / New image cards (accordion with size and Create), Open…, About, language picker
- New-document dialog (File ▸ New… in an editor) with mode choice and size
- Size defaults and limits per mode; "Open like this on startup"
- File → mode routing by content signature with extension fallback, including files handed over by the Pixel editor
- One mounted editor per session; editor loading and load-failure states
- Unsaved-changes guard on New / Open / Open Recent and on leaving the page
- Shared editor frame: top bar (mode badge, menus, document name, unsaved marker, panels toggle), tool rail, options bar, panels, status bar; responsive layout (desktop / tablet / phone); persisted panel layout per mode
- Shared keyboard map: tool-group keys with variant cycling, tooltips with key hints, dialogs holding the keyboard
- Help ▸ About Pincel and the start screen's About

### Out of scope

- How files are picked, read and written (FSA, download fallback, Tauri dialogs, recents, drag & drop, clipboard sizing) — because `file-access` owns it.
- Autosave and the Recovery dialog — because `autosave-recovery` owns it.
- UI language and the language picker's behavior — because `i18n` owns it.
- The on-screen Shift / Alt toggles — because `pen-input` owns touch and pen input.
- Native macOS menu, file association, open-file events and the updater entries in Help — because `desktop-app` owns the Tauri host.
- What each editor's menus, tools and panels do — because the per-mode feature specs own them.
- A keyboard shortcut overview / cheat sheet — because it is the planned `shortcut-overview` spec.

## Acceptance criteria

- [ ] AC-01 (always): The start screen offers "New pixel sprite", "New image", "Open…" and "About Pincel"; no editor is mounted while it shows.
- [ ] AC-02 (event): When the user activates a New card on the start screen, then that card expands in place with Width / Height prefilled (Pixel 64 × 64, Image 800 × 600) and Create, any other expanded card collapses, no dialog opens, and with a fine pointer the Width field has focus.
- [ ] AC-03 (event): When the user presses Escape while a New card is expanded, then the card collapses and its header button has focus.
- [ ] AC-04 (event): When the user submits the New image card, then the Image editor opens with a document of exactly the typed width and height.
- [ ] AC-05 (event): When the user submits the New pixel sprite card, then the Pixel editor opens with a sprite of exactly the typed width and height.
- [ ] AC-06 (error): If a typed size is empty or not a number, then the mode's default size is used; if it is outside the mode's limits, then it is clamped (Pixel: 1–4096 per edge; Image: 1–16384 per edge and at most 8192 × 8192 px in area, scaling both edges down together).
- [ ] AC-07 (option): Where "Open like this on startup" is ticked on a New card or in the New dialog, the next app launch skips the start screen and opens an empty document of that mode and size; at most one mode is remembered (ticking one card clears the other), and unticking it in File ▸ New… brings the start screen back on the next launch.
- [ ] AC-08 (event): When the user chooses File ▸ New… in an editor, then the "New document" dialog opens with the current editor's mode preselected and that mode's default size; switching the mode resets the size to the new mode's default; Create mounts a fresh editor of the chosen mode; Cancel or Escape closes the dialog and leaves the open document unchanged.
- [ ] AC-09 (event): When a file is opened from the shell (start screen Open…, File ▸ Open… in Image mode, Open Recent, drop), then an Aseprite sprite opens in the Pixel editor and an OpenRaster, PNG, JPEG, WebP, BMP, GIF or TIFF file opens in the Image editor.
- [ ] AC-10 (always): A file's format is decided by its leading bytes; the file-name extension decides only when no known signature matches (a PNG named `x.aseprite` opens in Image mode).
- [ ] AC-11 (event): When the user opens a file through the Pixel editor's own File ▸ Open…, then a sprite or PNG stays in Pixel mode (a PNG becomes a one-layer sprite) and any other supported format is handed to the Image editor.
- [ ] AC-12 (error): If the opened file is neither a known signature nor a known extension, then a dismissable error notice names the file and the start screen or open document stays as it was.
- [ ] AC-13 (always): Exactly one editor is mounted at a time, and a session in one mode never downloads the other mode's wasm module.
- [ ] AC-14 (state): While the open document has unsaved changes, File ▸ New…, File ▸ Open… and Open Recent first ask "Discard changes" / "Cancel"; Discard proceeds with the action, Cancel keeps the document unchanged.
- [ ] AC-15 (state): While the open document has unsaved changes, closing or reloading the page triggers the browser's leave-page confirmation; with no unsaved changes it does not.
- [ ] AC-16 (always): The editor top bar shows the mode badge, the document name and, while there are unsaved changes, an unsaved marker whose tooltip reads "<name> — unsaved changes"; after a successful save the marker disappears.
- [ ] AC-17 (state): While the window is at least 1024 px wide, menus are a menu bar, tools a vertical rail and panels a docked sidebar; at 640–1023 px the panels are a drawer over the canvas toggled from the top bar; below 640 px every menu sits behind one ☰ sheet, tools are a horizontal strip at the bottom and panels open as a bottom sheet with one tab per panel.
- [ ] AC-18 (event): When the user collapses a panel (or hides the docked sidebar), then that state is restored the next time an editor of the same mode is mounted, also after a reload.
- [ ] AC-19 (always): The same tool key selects the same tool group in Pixel and Image mode, repeated presses cycle the group's variants, the options bar offers the group's variants as a choice, D resets the colors to black / white and X swaps them; tool-rail tooltips name the group's key (e.g. "Pencil (B)").
- [ ] AC-20 (state): While a text field has focus, or the About or New document dialog is open, tool keys, undo and paste do not reach the editor.
- [ ] AC-21 (event): When the user chooses Help ▸ About Pincel in either editor, or About Pincel on the start screen, then the About dialog opens and names the version; Close or Escape dismisses it.

## Technical notes

Hints, not contracts — paths drift; grep for the names.

- Shell: `ui/src/App.svelte` (start screen / editor slot, `guard()`, `openFile()`, `openRecent()`, `onDropFile()`, shell error toast). Editors load lazily: `loaders: Record<EditorMode, () => Promise<EditorModule>>` with `import('./modes/pixel/PixelEditor.svelte')` / `import('./modes/image/ImageEditor.svelte')`, wrapped in `{#key session.seq}`.
- Session store `ui/src/lib/shell/session.svelte.ts`: `session = $state({ mode: 'start' | 'pixel' | 'image', seq, pending, dirty })`; `detectMode(format)` returns `'pixel'` only for `'aseprite'`; `launch()` bumps `seq` to remount.
- Shell ↔ editor contract `ui/src/lib/shell/types.ts`: `EditorProps { initialFile, initialNew, onRequestNew, onRequestOpen, onOpenForeign, onDirtyChange, onOpenRecent, registerMenuHandlers }`, `NewDocParams { mode, width, height }`.
- Sizes `ui/src/lib/shell/new-doc.ts`: `NEW_DOC_LIMITS = { pixel: { max: 4096, width: 64, height: 64 }, image: { max: MAX_CANVAS_DIM, width: 800, height: 600 } }`; Image clamp is `clampCanvasSize` (`MAX_CANVAS_DIM = 16384`, `MAX_CANVAS_AREA = 8192 * 8192`).
- Startup document `ui/src/lib/shell/auto-new.svelte.ts`: IndexedDB pref `autoNew` = `{ mode, width, height }`, re-clamped on load.
- Format detection `ui/src/lib/fs/index.ts`: `sniffFormat(bytes)` (Aseprite magic `0xA5E0` at offset 4, ORA zip with stored `mimetype`, PNG, JPEG, RIFF/WEBP, BM, GIF8, TIFF II/MM) then `formatFromName(name)`.
- Frame `ui/src/lib/components/layout/`: `EditorFrame.svelte`, `MenuBar.svelte` (dropdowns, ☰ sheet when `compact`, Arrow / Escape keys), `menu.ts` (`MenuDef`, `MenuItem`, `tidy()`), `panels.svelte.ts` (`PanelDef`, `FrameLayout { collapsed, sidebar }` stored as pref `layout:<mode>`), `viewport.svelte.ts` (`wide` ≥ 1024, `medium` 640–1023, `narrow` < 640), `ToolRail.svelte`, `tools.ts`.
- Keyboard map `ui/src/lib/shell/shortcuts.ts`: `SHORTCUTS`, `TOOL_GROUP_KEYS`, `NATIVE_MENU_ACTIONS`, `actionFor()`, `toolGroupForKey()`, `isTypingTarget()`, `hint()`, `toolHint()`.
- Unsaved guard: shell `guard()` uses `session.dirty` mirrored through `onDirtyChange`; the Pixel editor additionally guards its own Open / Open Recent (`guardUnsaved` in `modes/pixel/file-ops.ts`). `beforeunload` handlers live in both editors.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Size field cleared, then Create or "Open like this on startup" ticked | Mode default size (Pixel 64 × 64, Image 800 × 600) |
| Image size 16384 × 16384 typed | Scaled down together to fit 8192 × 8192 px area |
| A file handed over by the OS arrives while the remembered startup document is loading | The file wins; the empty startup document is not created |
| Second open request while a file is still being read | Ignored until the first finishes |
| Editor chunk fails to load | Error text "Failed to load the <mode> editor: …" in place of the editor |
| Key pressed that the active mode has no tool group for | Nothing happens |
| Window resized while a menu is open | The menu closes |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | | open | |
| AC-02 | `ui/e2e/smoke.spec.ts › start screen: the New cards open in place, one at a time` | built | |
| AC-03 | `ui/e2e/smoke.spec.ts › start screen: the New cards open in place, one at a time` | built | |
| AC-04 | `ui/e2e/auto-new.spec.ts › a ticked New card opens its document on the next launch` | built | |
| AC-05 | | open | |
| AC-06 | | open | |
| AC-07 | `ui/e2e/auto-new.spec.ts › a ticked New card opens its document on the next launch`; `ui/e2e/auto-new.spec.ts › ticking one card clears the other` | built | |
| AC-08 | | open | |
| AC-09 | | open | |
| AC-10 | | open | |
| AC-11 | | open | |
| AC-12 | | open | |
| AC-13 | | open | |
| AC-14 | | open | |
| AC-15 | | open | |
| AC-16 | | open | |
| AC-17 | `ui/e2e/responsive.spec.ts › tools sit in a bottom strip, menus behind ☰, panels in a sheet`; `ui/e2e/responsive.spec.ts › the sidebar is a drawer toggled from the top bar` | built | |
| AC-18 | | open | |
| AC-19 | `ui/e2e/shortcuts.spec.ts › pixel mode: the shared keymap selects tools and cycles variants`; `ui/e2e/shortcuts.spec.ts › image mode: the shared keymap selects tools and cycles variants`; `ui/e2e/smoke.spec.ts › pixel mode: a pencil stroke is undoable` | built | |
| AC-20 | | open | |
| AC-21 | `ui/e2e/help.spec.ts › pixel mode: Help ▸ About Pincel names the version`; `ui/e2e/help.spec.ts › image mode: Help ▸ About Pincel names the version`; `ui/e2e/help.spec.ts › the start screen reaches About without a menu bar` | built | |

Partial coverage (status stays `open`): AC-16 — `ora.spec.ts › image mode: Save writes an OpenRaster file that reopens with its layers` asserts the unsaved marker appearing after an edit and disappearing after Save, not the mode badge or name; AC-01 — `smoke.spec.ts › start screen offers both modes` checks the two New cards only, Open… is clicked in `ora.spec.ts`, About in `help.spec.ts`; AC-13 — `smoke.spec.ts › image mode: a brush stroke is undoable and the pixel wasm is not loaded` asserts only the Image-mode direction; AC-14 — `recovery.spec.ts › a discarded document is not offered again, on New or after a reload` clicks Discard after File ▸ New… in Pixel mode; Cancel, Open and Image mode untested; AC-18 — `responsive.spec.ts › desktop: panels collapse and stay collapsed across sessions` covers collapse; the docked-sidebar toggle is untested; AC-20 — About (`help.spec.ts › … the About dialog holds the keyboard`) and paste in the New dialog (`paste.spec.ts › a paste inside the shell New dialog does not reach the canvas`) are covered; text-field focus and keys in the New dialog are not.

## Decisions

- 2026-10-29: Extracted from the code as built (reverse engineering); ACs describe current behavior.
