---
status: todo
created: 2026-11-04
size: L
mode: both
design: [pincel.md §9.2, pincel.md §10, pincel.md §15 (2026-09-06 "Two cores, one shell", 2026-11-04 "Several documents per editor")]
---

# Document tabs: several images open in one mode

## Context

Pincel holds one document at a time: New, Open, Open Recent, a file drop or Paste as New Image replace the open document (after the unsaved-changes prompt). The owner wants several images open side by side in tabs, as in paint.net, and chose **tabs within one mode**: Image-mode documents share one tab bar, Pixel-mode documents share one tab bar, and opening a document of the other mode replaces the whole session after asking about unsaved work.

Decision Log 2026-11-04 ("Several documents per editor") records the architecture: still exactly one editor mounted (`app-shell` AC-13 stays true), and that editor holds several documents and swaps between them. It amends 2026-09-06 ("Two cores, one shell"), whose session holds one document.

This spec builds on `unsaved-changes-prompt` (Save / Don't Save / Cancel) and extends it to several documents, as that spec's out-of-scope list announces.

**Terms.** *Tab*: one open document in the tab bar, with its own content, undo history, file (name, location, format), unsaved state, view (zoom, scroll), selection and — in Pixel mode — current layer, frame and onion-skin state. *Active tab*: the tab whose document the canvas, panels, menus and status bar show. *Session*: the mounted editor with all its tabs. *Same-mode document*: a document whose format belongs to the mode of the mounted editor (`.aseprite` → Pixel; PNG / JPEG / WebP / BMP / GIF / TIFF / ORA → Image; a PNG opened from Pixel mode's own Open stays in Pixel mode as today). *Unsaved changes* and *document name*: as `unsaved-changes-prompt` defines them, per tab. *Shared state*: the active tool and its options, the foreground / background colours (RGBA values), the panel layout and the sidebar width — one for the whole session, unchanged when the active tab changes; each Pixel tab keeps its own palette, which is document content. *Same file*: in the desktop app, the same file path (compared case-insensitively on Windows and macOS, exactly on Linux, without resolving links); in the browser, two file handles the File System Access API reports as the same entry; a document without a path or handle (never saved, or opened through the download fallback) is never the same file as another.

## Scope

### In scope

- A tab bar in both editors; opening, creating, switching, closing tabs with pointer and keyboard
- New, Open, Open Recent, file drop, Paste as New Image and files handed over by the operating system: a same-mode document opens as a new tab; another-mode document replaces the session after the prompt
- Per-tab document state; shared tool and colour state
- The unsaved-changes prompt for one tab (closing it) and for several tabs (closing the session, replacing it, closing the desktop window)
- Autosave per tab; recovery offers every tab's snapshot and recovers into tabs
- Top bar and window title follow the active tab
- en + de strings
- Revision of `app-shell` AC-08 and AC-16 and `autosave-recovery` AC-05

### Out of scope

- Tabs that mix Pixel and Image documents — because each mode has its own core and editor; the owner chose tabs within one mode
- Reordering tabs by drag, tearing a tab off into its own window, split views — because they are layout features beyond "several images open"; paint.net 5 has no split view either
- Reopening the tabs of the last session on start — because the recovery covers unsaved work, and remembering saved files is Open Recent's job
- Copying layers or frames between tabs by drag — because copy / paste across tabs already works through the clipboard (`copy-cut-paste`)
- A limit on the number of tabs — because memory depends on document sizes, not on a count (see Assumptions for what running out of memory means)

## Acceptance criteria

- [ ] AC-01 (always): While an editor is mounted, a tab bar between the options bar and the canvas shows one tab per open document, in the order they were opened, each with the document name, an unsaved marker while it has unsaved changes and a close button; the active tab is visually marked and exposed as the selected tab — in Pixel and in Image mode, at window widths of 1280, 800 and 390 px, where the bar scrolls horizontally when the tabs do not fit and scrolls the active tab into view.
- [ ] AC-02 (event): When the user opens, creates (File ▸ New… with the current mode), recovers, drops, receives from the operating system or pastes as new image a same-mode document (a pasted image is routed by the format the paste produces, as today), then it opens as a new tab after the last tab and becomes the active tab; every other tab keeps its document, undo history and unsaved state, and no unsaved-changes prompt appears.
- [ ] AC-03 (event): When the user opens (Open, Open Recent, drop, operating system) the same file as an open tab, then that tab becomes active, no second tab opens and that tab's unsaved changes are kept. A recovered snapshot always opens as a new tab, because it holds edits the file does not.
- [ ] AC-04 (event): When the user clicks a tab, or presses the next / previous tab shortcut (Ctrl+Tab / Ctrl+Shift+Tab in the desktop app; Alt+Page Down / Alt+Page Up in both), then that tab becomes active, wrapping from the last to the first and back; the canvas, layers, timeline (Pixel), palette (Pixel), status bar, undo / redo state, top-bar name and window title show the newly active document.
- [ ] AC-05 (always): Each tab keeps its own view and selection: after switching away and back, the zoom, the scroll position, the selection and — in Pixel mode — the current layer, current frame and onion-skin settings are as they were; the active tool, its options and the colours stay as they are when switching (shared state).
- [ ] AC-06 (event): When the user undoes or redoes, then only the active tab's history changes; edits in one tab never change another tab's document.
- [ ] AC-07 (event): When the user closes a tab without unsaved changes (its close button, a middle-click on the tab, File ▸ Close, Ctrl+W in the desktop app, Alt+W in both), then the tab disappears and the tab to its right becomes active (the one to its left when it was the last) — as in browsers and paint.net.
- [ ] AC-08 (event): When the user closes a tab with unsaved changes, then the unsaved-changes prompt of `unsaved-changes-prompt` asks about that document (Save / Don't Save / Cancel, with its name); Save saves it and closes the tab only after a successful save, Don't Save closes it, Cancel keeps it; while the prompt is open the tab being closed is the active tab.
- [ ] AC-09 (event): When the last tab is closed, then the editor is unmounted and the start screen is shown.
- [ ] AC-10 (event): When an action would replace the whole session — opening, creating, dropping, receiving from the operating system or pasting as new image a document of the other mode, or closing the desktop window — and exactly one tab has unsaved changes, then the single-document prompt of `unsaved-changes-prompt` asks about that tab first, and that tab becomes active.
- [ ] AC-11 (event): When such an action happens and two or more tabs have unsaved changes, then one prompt lists the names of all of them and offers Save All, Don't Save and Cancel (Save All has focus, Escape cancels); Save All saves them one after another in tab order, left to right, each tab becoming active while it saves (a never-saved document asks for its location as File ▸ Save does) and continues the action only when every save succeeded; if a save is cancelled or fails, the action is aborted, documents saved so far stay saved, and the rest keep their unsaved changes; Don't Save continues and discards them all; Cancel changes nothing.
- [ ] AC-12 (event): When the action replaces the session after the prompt (or without one when no tab has unsaved changes), then all tabs close and the other mode's editor opens with the new document as its only tab. The new document is read and checked before the prompt (as `unsaved-changes-prompt` AC-01 / AC-11), so an unreadable file shows its error and leaves every tab open.
- [ ] AC-13 (always): The shell's notion of unsaved changes — the browser's leave-page warning (`app-shell` AC-15) and the desktop close prompt — counts the session as unsaved while any tab has unsaved changes.
- [ ] AC-14 (state): While several tabs have unsaved changes, each is autosaved on its own (as `autosave-recovery` AC-01 – AC-03 describe for one document), and a save or a Don't Save on one tab removes only that tab's snapshot.
- [ ] AC-15 (event): When the recovery dialog offers snapshots and the user clicks Recover on one, then that document opens as a new tab marked unsaved and the dialog stays open while snapshots remain; recovering several gives several tabs.
- [ ] AC-16 (always): Closing a tab releases its document's memory, and leaving the editor releases all of them: a diagnostic count of open documents, which each mode's core exposes for tests, equals the number of open tabs after any sequence of opening and closing tabs, and is 0 after the editor is left.
- [ ] AC-17 (state): While a tab has a running save, closing that tab by any means of AC-07 does nothing, and other tabs can be switched to; an action that replaces the session or closes the desktop window waits for the running save to finish and then prompts as AC-10 / AC-11 describe (as `unsaved-changes-prompt` AC-10 does for one document).
- [ ] AC-19 (event): When the user drops several files at once, then each file of the mounted editor's mode opens as a tab, in drop order (AC-02 / AC-03 for each); files of the other mode are not opened and a notice names how many were skipped; when every dropped file belongs to the other mode, the first one replaces the session as AC-10 – AC-12 describe and the rest are skipped with the same notice.
- [ ] AC-18 (option): Where the UI language is German, the tab bar's labels (close button, accessible names), the several-documents prompt and File ▸ Close are German ("Schließen", „Ungespeicherte Änderungen in {n} Bildern“, „Alle speichern“, „Nicht speichern“, „Abbrechen“).

## Technical notes

Hints, not contracts — paths drift; grep for the names (verified 2026-11-04).

- Shell: `ui/src/App.svelte` (guard, `launch`), `ui/src/lib/shell/session.svelte.ts` (`session.dirty` is one flag today, `launch` remounts via `seq`), `lib/shell/types.ts` (`EditorProps`: `onDirtyChange`, `onOpenFile`, `onOpenForeign`, `onOpenRecent`). Same-mode routing has to reach the mounted editor instead of `launch`.
- Image mode: the core is handle-based (`pincel-image-wasm`), so several documents already fit; module singletons that assume one document: `modes/image/stores/editor.svelte.ts` (`editor.handle`), `modes/image/core/document-file.ts`, `controller.newDocument` / `openBytes` (they close the previous handle). `ImageEditor` unmount does not close the handle today (AC-16 fixes the leak).
- Pixel mode: `Document` is a per-instance wasm class; per-tab state lives in `modes/pixel/stores/editor.svelte.ts` (`ed.doc`, file, view) and `file-ops.ts` / `doc-ops.ts`.
- Autosave: `lib/idb/autosave-session.svelte.ts` (`AutosaveSession`, one `docId` per document; `close()` deletes the snapshot). One session per tab.
- Shortcuts: `lib/shell/shortcuts.ts`. Browsers keep Ctrl+Tab and Ctrl+W for their own tabs, hence the Alt variants in the web build.
- A tab bar component under `lib/components/layout/` shared by both editors via `EditorFrame`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Pixel mode opens a PNG through its own Open | It opens as a Pixel tab (same-mode rule of today) |
| The same untitled name twice (two new documents) | Both tabs read the default name; they are separate documents |
| An effect panel or other dialog is open and the user clicks another tab | Tab switching is blocked while a dialog is open, as other editor input is |
| Several dirty tabs, Save All, the second document's Save As is cancelled | The first stays saved, the second and later stay unsaved, the action is aborted (AC-11) |
| A tab's file is a recent entry that is opened again from Open Recent | The existing tab becomes active (AC-03) |
| Closing a tab while an autosave write of it is running | The tab closes; its snapshot is removed when the write finishes |
| Opening a same-mode document fails (unreadable file) | An error is shown; no tab opens and the active tab stays |
| The other mode's editor fails to open the already-checked document after the prompt | The start screen shows the error; the old tabs are gone, their changes were saved or discarded by the user's choice |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Closing the last tab shows the start screen | The start screen is the "no document" state the shell already has | Low |
| Tool and colours are shared, per-tab state is document state | paint.net shares tools and the palette across images | Low |
| The several-documents prompt is a list with Save All / Don't Save / Cancel, not one prompt per tab | paint.net and Office show one list for many documents; one decision is faster than n prompts | Low |
| Alt+Page Up / Down and Alt+W in the web build | Browsers reserve Ctrl+Tab and Ctrl+W | Medium: Alt+W may collide with a browser or OS shortcut on some systems; the tab close button and File ▸ Close remain |
| No tab limit | All tabs of a mode share one wasm memory (up to 4 GB); running out of it ends the whole editor, not one tab, and the autosave recovery is the safety net — the same as one huge document today | Medium |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |

## Decisions

- 2026-11-04: The owner chose "tabs within one mode only". Written without a further dialogue while the owner's goal "all planned specs implemented" was active; shortcuts and the several-documents prompt follow paint.net and recorded assumptions.
- 2026-11-04: Clarification pass settled without a dialogue: same-file rule defined (paths, `isSameEntry`, no handle = never same); recovering is not a session-replacing action (recovery lists only the mounted mode's snapshots); Save All runs in tab order; the core's open-document count is a test diagnostic (AC-16); several dropped files open as tabs (AC-19); a running save makes session-wide actions wait (AC-17).
- 2026-11-04: Revises `app-shell` AC-08 (Create in the same mode adds a tab instead of remounting), `app-shell` AC-16 (the top bar shows the active tab) and `autosave-recovery` AC-05 (Recover opens a tab and keeps the dialog open); those specs carry the new wording.
