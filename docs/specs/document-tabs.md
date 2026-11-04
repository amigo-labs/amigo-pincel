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

**Terms.** *Tab*: one open document in the tab bar, with its own content, undo history, file (name, location, format), unsaved state, view (zoom, scroll), selection and — in Pixel mode — current layer, frame and onion-skin state. *Active tab*: the tab whose document the canvas, panels, menus and status bar show. *Session*: the mounted editor with all its tabs. *Same-mode document*: a document whose format belongs to the mode of the mounted editor (`.aseprite` → Pixel; PNG / JPEG / WebP / BMP / GIF / TIFF / ORA → Image; a PNG opened from Pixel mode's own Open stays in Pixel mode as today). *Unsaved changes* and *document name*: as `unsaved-changes-prompt` defines them, per tab. *Shared state*: the active tool and its options, the foreground / background colours (RGBA values), the panel layout and the sidebar width — one for the whole session, unchanged when the active tab changes; each Pixel tab keeps its own palette, which is document content. *Keys*: chords are written with the Windows / Linux names; on macOS Ctrl means ⌘ Command and Alt means ⌥ Option (the `mod` convention of `ui/src/lib/shell/shortcuts.ts`) unless the tab-key table below says otherwise. *Same file*: in the desktop app, the same file path (compared case-insensitively on Windows and macOS, exactly on Linux, without resolving links); in the browser, two file handles the File System Access API reports as the same entry; a document without a path or handle (never saved, or opened through the download fallback) is never the same file as another.

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
- [ ] AC-04 (event): When the user clicks a tab, or presses a next / previous tab shortcut from the tab-key table, then that tab becomes active, wrapping from the last to the first and back; the canvas, layers, timeline (Pixel), palette (Pixel), status bar, undo / redo state, top-bar name and window title show the newly active document.
- [ ] AC-05 (always): Each tab keeps its own view and selection: after switching away and back, the zoom, the scroll position, the selection and — in Pixel mode — the current layer, current frame and onion-skin settings are as they were; the active tool, its options and the colours stay as they are when switching (shared state).
- [ ] AC-06 (event): When the user undoes or redoes, then only the active tab's history changes; edits in one tab never change another tab's document.
- [ ] AC-07 (event): When the user closes a tab without unsaved changes (its close button, a middle-click on the tab, File ▸ Close, or a close shortcut from the tab-key table), then the tab disappears and the tab to its right becomes active (the one to its left when it was the last) — as in browsers and paint.net.
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
- [ ] AC-20 (always): Every tab shortcut works with the keys of the tab-key table for the platform it runs on, and the hints in menus and tooltips name them as that platform does (on macOS with ⌃ ⌥ ⇧ ⌘ symbols, e.g. File ▸ Close shows ⌘W in the desktop app); on macOS the shortcuts match the physical key, so ⌥W closes the tab although the keyboard layout turns it into another character.
- [ ] AC-19 (event): When the user drops several files at once, then each file of the mounted editor's mode opens as a tab, in drop order (AC-02 / AC-03 for each); files of the other mode are not opened and a notice names how many were skipped; when every dropped file belongs to the other mode, the first one replaces the session as AC-10 – AC-12 describe and the rest are skipped with the same notice.
- [ ] AC-18 (option): Where the UI language is German, the tab bar's labels (close button, accessible names), the several-documents prompt and File ▸ Close are German ("Schließen", „Ungespeicherte Änderungen in {n} Bildern“, „Alle speichern“, „Nicht speichern“, „Abbrechen“).

## Tab keys

| Action | Windows / Linux desktop | macOS desktop | Web build, Windows / Linux | Web build, macOS |
| ------ | ----------------------- | ------------- | -------------------------- | ---------------- |
| Next tab | Ctrl+Tab, Alt+Page Down | ⌃Tab (Control, because ⌘Tab is the system app switcher), ⇧⌘], ⌥⇟ | Alt+Page Down | ⌥⇟ (fn+⌥↓ on keyboards without Page Down) |
| Previous tab | Ctrl+Shift+Tab, Alt+Page Up | ⌃⇧Tab, ⇧⌘[, ⌥⇞ | Alt+Page Up | ⌥⇞ |
| Close tab | Ctrl+W, Alt+W | ⌘W, ⌥W | Alt+W | ⌥W |

The web build gets no Ctrl / ⌘ tab chords because the browser keeps Ctrl+Tab, Ctrl+W, ⌘W and ⇧⌘[ / ] for its own tabs. On macOS the desktop app's native menu owns ⌘W (File ▸ Close) as it owns its other accelerators (pincel.md §9.4).

## Technical notes

Hints, not contracts — paths drift; grep for the names (verified 2026-11-04).

- Shell: `ui/src/App.svelte` (guard, `launch`), `ui/src/lib/shell/session.svelte.ts` (`session.dirty` is one flag today, `launch` remounts via `seq`), `lib/shell/types.ts` (`EditorProps`: `onDirtyChange`, `onOpenFile`, `onOpenForeign`, `onOpenRecent`). Same-mode routing has to reach the mounted editor instead of `launch`.
- Image mode: the core is handle-based (`pincel-image-wasm`), so several documents already fit; module singletons that assume one document: `modes/image/stores/editor.svelte.ts` (`editor.handle`), `modes/image/core/document-file.ts`, `controller.newDocument` / `openBytes` (they close the previous handle). `ImageEditor` unmount does not close the handle today (AC-16 fixes the leak).
- Pixel mode: `Document` is a per-instance wasm class; per-tab state lives in `modes/pixel/stores/editor.svelte.ts` (`ed.doc`, file, view) and `file-ops.ts` / `doc-ops.ts`.
- Autosave: `lib/idb/autosave-session.svelte.ts` (`AutosaveSession`, one `docId` per document; `close()` deletes the snapshot). One session per tab.
- Shortcuts: `lib/shell/shortcuts.ts` (`mod` = Ctrl or ⌘, `chordLabel` prints ⇧⌘S on macOS). ⌃Tab on macOS needs a real Control chord, not `mod`. On macOS ⌥ letters produce other characters (⌥W types "∑"), so match these chords on `KeyboardEvent.code`, not `key`.
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
| AC-01 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: same-mode documents open as tabs after the last one, without a prompt (AC-01, AC-02)", "{pixel,image}: the tab bar scrolls and keeps the active tab in view at every width (AC-01)" (1280 / 800 / 390 px; the bar between the options bar and the canvas); the active tab's background differs from the others in "{pixel,image}: same-mode documents open as tabs … (AC-01, AC-02)" | built | |
| AC-02 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: same-mode documents open as tabs after the last one, without a prompt (AC-01, AC-02)" (drop, File ▸ New…), "{pixel,image}: opening the file of an open tab … (AC-03)" (Open…), "the recovery dialog stays open … (AC-15)" (recover); `ui/e2e/document-tabs-desktop.spec.ts`: "the same path opens no second tab, whatever its case (AC-03)" (operating system); `ui/e2e/document-tabs.spec.ts`: "clipboard › image: Paste as New Image opens a tab after the unsaved one, without a prompt (AC-02)" (paste); `ui/e2e/document-tabs-desktop.spec.ts`: "on Windows › {pixel,image}: Open Recent of a closed file of this mode opens it as a tab, without a prompt (AC-02)" (Open Recent) | built | |
| AC-03 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: opening the file of an open tab activates that tab and keeps its changes (AC-03)" (File System Access `isSameEntry`), "{pixel,image}: dropping the file of an open tab activates that tab (AC-03, AC-19)" (drop, alone and among several files); `ui/e2e/document-tabs-desktop.spec.ts`: "on Windows › the same path opens no second tab, whatever its case (AC-03)", "on Linux › paths differing in case are different files (AC-03)"; `ui/e2e/unsaved-close.spec.ts`: "Open Recent asks first, from both editors; a missing recent shows its error (AC-01, AC-11)" (Open Recent of the open file); `ui/e2e/document-tabs.spec.ts`: "each unsaved tab is autosaved and recovered into its own tab (AC-14, AC-15)" (a recovered snapshot opens a new tab) | built | |
| AC-04 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: tabs switch by click and by keys, each with its own document, view and history (AC-04, AC-05, AC-06)" (click, Alt+Page Down / Up with wrap-around, top bar, window title, undo state), "{pixel,image}: the canvas, status bar and palette show the active tab's document (AC-04)" (canvas screenshot, status-bar size, Layers panel, Pixel palette; undo / redo state in the switch test); `ui/e2e/document-tabs-desktop.spec.ts`: "Ctrl+Tab, Ctrl+Shift+Tab and Ctrl+W … (AC-04, AC-07, AC-20)" | built | |
| AC-05 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: tabs switch by click and by keys, … (AC-04, AC-05, AC-06)" (zoom, selection, shared tool), "{pixel,image}: each tab keeps its scroll position; tool options and colours are shared (AC-05)" (pan, brush size, foreground colour), "pixel: each tab keeps its current layer, frame and onion skin (AC-05)" | built | |
| AC-06 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: tabs switch by click and by keys, … (AC-04, AC-05, AC-06)" (an edit and its undo in one tab leave the other unsaved) | built | |
| AC-07 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: closing a clean tab activates its right-hand neighbour; the last one shows the start screen (AC-07, AC-09)" (close button, middle-click, File ▸ Close, Alt+W); `ui/e2e/document-tabs-desktop.spec.ts`: "Ctrl+Tab, Ctrl+Shift+Tab and Ctrl+W … (AC-04, AC-07, AC-20)" | built | |
| AC-08 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: closing a tab with unsaved changes asks about that document (AC-08)" | built | |
| AC-09 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: closing a clean tab activates its right-hand neighbour; the last one shows the start screen (AC-07, AC-09)" | built | |
| AC-10 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: one unsaved tab is asked about before another mode replaces the session (AC-10, AC-12)"; `ui/e2e/document-tabs-desktop.spec.ts`: "one unsaved tab in the background is asked about and activated before closing (AC-10)"; `ui/e2e/unsaved-prompt.spec.ts`: "{pixel,image}: File ▸ New… of the other mode asks after the New dialog (AC-01, AC-05)"; `ui/e2e/paste-new-image.spec.ts`: "pixel: Paste as New Image asks about unsaved changes first (AC-23)" | built | |
| AC-11 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: several unsaved tabs are listed in one prompt — Save All, Don't Save, Cancel (AC-11)", "{pixel,image}: Don't Save in the list prompt discards every unsaved tab (AC-11, AC-12)", "{pixel,image}: a failed save in Save All aborts the action and keeps the rest unsaved (AC-11)"; `ui/e2e/document-tabs-desktop.spec.ts`: "closing the window with several unsaved tabs lists them; Save All saves, then closes (AC-10, AC-11, AC-13)" | built | |
| AC-12 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: one unsaved tab is asked about before another mode replaces the session (AC-10, AC-12)" (an unsupported file leaves every tab open), "{pixel,image}: Don't Save in the list prompt … (AC-11, AC-12)" | built | |
| AC-13 | `ui/e2e/document-tabs.spec.ts`: "the leave-page warning counts an unsaved tab in the background (AC-13)"; `ui/e2e/document-tabs-desktop.spec.ts`: "closing the window with several unsaved tabs lists them; … (AC-10, AC-11, AC-13)" (the active tab is clean) | built | |
| AC-14 | `ui/e2e/document-tabs.spec.ts`: "each unsaved tab is autosaved and recovered into its own tab (AC-14, AC-15)" (saving one tab and Don't Save on another remove only their snapshots) | built | |
| AC-15 | `ui/e2e/document-tabs.spec.ts`: "the recovery dialog stays open while snapshots remain; each Recover opens a tab (AC-15)", "each unsaved tab is autosaved and recovered into its own tab (AC-14, AC-15)" | built | |
| AC-16 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: closing tabs and leaving the editor frees their documents (AC-16)"; `crates/pincel-image-wasm/src/lib.rs` `document_count_tests::document_count_follows_create_and_close`; `crates/pincel-wasm/src/lib.rs` `live_count_tests::open_document_count_follows_new_open_and_drop` | built | |
| AC-17 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: a running save keeps its tab open and lets other tabs be switched to (AC-17)"; `ui/e2e/document-tabs-desktop.spec.ts`: "a close during a running save waits for it, then asks about the rest (AC-17)"; `ui/e2e/unsaved-prompt.spec.ts`: "an action while a File ▸ Save runs waits for it, then asks if still needed (AC-08, document-tabs AC-17)" | built | |
| AC-18 | `ui/e2e/document-tabs.spec.ts`: "German › the tab bar, the list prompt and File ▸ Close speak German (AC-18)" | built | |
| AC-19 | `ui/e2e/document-tabs.spec.ts`: "{pixel,image}: dropping several files opens this mode's as tabs and skips the others (AC-19)", "{pixel,image}: dropping the file of an open tab activates that tab (AC-03, AC-19)" | built | |
| AC-20 | `ui/e2e/document-tabs-desktop.spec.ts`: "on Windows › Ctrl+Tab, Ctrl+Shift+Tab and Ctrl+W … (AC-04, AC-07, AC-20)", "on macOS › ⌃Tab, ⇧⌘[ / ], ⌥⇟ and ⌥W work by physical key; the menu owns ⌘W (AC-20)", "web build › on macOS › ⌥⇟ / ⌥⇞ / ⌥W, named with macOS symbols; no Control or ⌘ chords (AC-20)", "web build › on Windows › Alt+Page Down / Up / Alt+W; Ctrl+Tab and Ctrl+W stay the browser's (AC-20)"; `src-tauri/src/main.rs`: File ▸ Close with `CmdOrCtrl+W` | built | |

## Decisions

- 2026-11-04: The owner chose "tabs within one mode only". Written without a further dialogue while the owner's goal "all planned specs implemented" was active; shortcuts and the several-documents prompt follow paint.net and recorded assumptions.
- 2026-11-04: Clarification pass settled without a dialogue: same-file rule defined (paths, `isSameEntry`, no handle = never same); recovering is not a session-replacing action (recovery lists only the mounted mode's snapshots); Save All runs in tab order; the core's open-document count is a test diagnostic (AC-16); several dropped files open as tabs (AC-19); a running save makes session-wide actions wait (AC-17).
- 2026-11-04: Revises `app-shell` AC-08 (Create in the same mode adds a tab instead of remounting), `app-shell` AC-16 (the top bar shows the active tab) and `autosave-recovery` AC-05 (Recover opens a tab and keeps the dialog open); those specs carry the new wording.
- 2026-11-04: Owner: "on Mac we have different modifier key names". Added the tab-key table per platform (⌃Tab on macOS because ⌘Tab is the app switcher; ⇧⌘[ / ] as in Safari and Finder; ⌘W) and AC-20 (platform key names in hints, physical-key matching for ⌥ chords).
- 2026-11-04 (spec-build): Per-tab state behind the existing stores: `lib/shell/tab-store.ts` returns a proxy that forwards each store's per-document keys (`ed`, `view`, `play`, `file` in Pixel mode; `editor`, `view` in Image mode) to the active tab's own `$state` and the rest to the shared state. Reason: every module keeps reading `store.key`; saving, closing and autosave bind to their tab explicitly, so they stay correct across a tab switch.
- 2026-11-04 (spec-build): File ▸ New… and File ▸ Open… no longer ask before the dialog / picker; the prompt follows Create or the pick, and only when the result is another mode's document. `unsaved-changes-prompt` AC-01 and its e2e tests are reworded accordingly. Reason: a same-mode result opens a tab without a prompt (AC-02), and AC-12 wants the document read and checked before the prompt.
- 2026-11-04 (spec-build): A session-wide action during a running save waits for it instead of being ignored (AC-17 over `unsaved-changes-prompt` AC-08's last sentence, which is reworded). While the prompt is open, a save from it runs or an action waits, every shell routing is ignored, same-mode opens included. Reason: AC-17; the prompt's answer applies to the action it names (`unsaved-changes-prompt` AC-08).
- 2026-11-04 (spec-build): `EditorProps.confirmUnsaved` is replaced by `confirmClose(doc, close)` (closing one tab asks about that document only), plus `registerSession(session) => unregister` (the shell routes same-mode documents to `EditorSession.open` / `create`) and `onEmpty(error?)` (last tab closed, or the first document failed: the start screen, with the error). `EditorDocument` gains `activate()`; every tab registers its own `EditorDocument`. Reason: the shell's single prompt serves one tab, several tabs and the desktop close.
- 2026-11-04 (spec-build): `AutosaveSession` is now one per document (its constructor takes the identity); the Recovery list moved to a new `RecoveryOffer` per editor. `finish()` waits for a snapshot write in flight before removing the snapshot (edge case "closing a tab while an autosave write of it is running").
- 2026-11-04 (spec-build): The open-document count (AC-16) is exported by the cores as `document_count()` (`pincel-image-wasm`, the handle map's size) and `openDocumentCount()` (`pincel-wasm`, a per-thread live counter in `Document`), and published for tests on `globalThis.__pincelDiagnostics.openDocuments.{pixel,image}`. Reason: the spec asks for a core-level diagnostic; a global hook is how the e2e reads it without importing the hashed wasm chunk.
- 2026-11-04 (spec-build): Same file: the path comparison folds case where the user agent is Windows or macOS (the desktop app runs on the OS it reports). A PNG opened in Pixel mode saves to a new `.aseprite` and has no path, so opening that PNG again opens a second tab ("no handle = never the same file").
- 2026-11-04 (spec-build): The window title is `document.title`, now set by Image mode as well (`● name – Pincel`, as Pixel mode did). The desktop window's native title is not synced — unchanged by this spec.
- 2026-11-04 (spec-build): Tab switching is blocked while a dialog is open because the dialogs are modal overlays above the tab bar and the editors' key handlers return early while one is open; the tab being left drops its gestures, playback and sub-modes (Pixel: polygon, stamp, tile editor; Image: gesture, previews).
- 2026-11-04 (spec-build): Pixel-mode Save no longer sets the editor-wide `file.busy`; the tab's own running save is what blocks closing it and autosaving it, so other tabs stay usable during a save (AC-17).
- 2026-11-04 (spec-build): Several dropped files: an unsupported one among them is skipped (its error shows unless the skipped-files notice replaces it); the notice reads "{n} dropped file(s) of the other mode were not opened". Files dropped on the start screen keep today's behavior (the first one). An OS hand-over that arrives while the previous one is still being opened is dropped, as before this spec.
- 2026-11-04 (spec-build): On macOS a chord with a physical key (`Chord.code`: ⌥W, ⇧⌘[ / ]) matches `KeyboardEvent.code`; elsewhere chords match `key` as before, so other keyboard layouts keep their letters. The native menu (macOS only) gets File ▸ Close with `CmdOrCtrl+W`; ⌘W is then not matched in the page (`Chord.menu`).
- 2026-11-04 (spec-build): The tab bar shifted the Image canvas, and at some pan offsets the selection's one-pixel ants straddled two device pixels and drew at half strength (the existing `marching-ants` e2e test caught it). The committed selection outline is now drawn in device space with its vertices on pixel centres, crisp at any zoom and pan.
- 2026-11-04 (spec-build): The UI tests were written in the same block as the code (the shell contract changed for both editors at once); their red run against the pre-feature build is recorded in the block commit message. The Rust diagnostics were test-first (red: `cannot find function`).
- 2026-11-04 (spec-build, verification round 1): The tests for AC-01 (Pixel at every width), AC-02 (Paste as New Image and Open Recent as tabs), AC-04 (canvas, status bar, palette) and AC-05 (scroll position, tool options, colours) were added after the first `/spec-check` found the coverage short; they were green on their first run because the behavior was already built. Round 2 added the Layers panel and redo state (AC-04), the tab bar's position (AC-01) and a failing save in Save All (AC-11), green at once for the same reason.
- 2026-11-04 (spec-build, verification round 3): A dropped file now carries its File System Access handle (`DataTransferItem.getAsFileSystemHandle`, read during the `drop` event; `droppedHandles` in `lib/fs`), so dropping the file of an open tab activates that tab (AC-03). Side effect: like a file from Open…, a dropped file with a handle saves back in place and is listed under Open Recent. Where the browser gives no handle, and in the desktop app (the webview's HTML drop carries no path), a dropped file is never the same file as another. The visual-mark check of AC-01 (computed background of the selected tab) was green on its first run; the drop test was red first (a second tab opened).
