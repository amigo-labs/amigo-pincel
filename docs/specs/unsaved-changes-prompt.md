---
status: todo
created: 2026-11-04
size: M
mode: shell
design: [pincel.md §9.2, pincel.md §11, app-shell.md AC-14, app-shell.md AC-15, app-shell.md AC-16, autosave-recovery.md]
---

# Unsaved-changes prompt: Save, Don't Save, Cancel

## Context

When an action would throw away unsaved work, Pincel today offers only "Discard changes" or "Cancel" (the shell's guard for New, Open, Open Recent and Paste as New Image, and a second, Pixel-only guard for the Pixel editor's own Open and Open Recent). There is no way to save from that prompt, so a user who wants to keep the work has to cancel, save and repeat the action.

Worse, the desktop app closes without asking at all: closing the window (title-bar ×, Alt+F4, the taskbar) or quitting on macOS ends the app immediately, and the browser's leave-page warning that protects the PWA does not exist in the desktop webview. Only the autosave recovery on the next start saves the user from losing the work.

This spec replaces both guards with a prompt that asks what should happen — **Save**, **Don't Save** or **Cancel** — the pattern of paint.net, Office and most desktop editors, and puts that prompt in front of closing the desktop app. It supersedes `app-shell` AC-14; `app-shell` AC-15 (the PWA's leave-page warning) stays as it is.

**Terms.** *Unsaved changes*: exactly the state in which the top bar shows the unsaved marker (`app-shell` AC-16). That includes a document restored by the autosave recovery and a Paste as New Image document until they are saved; undoing back to the saved state clears it. *Document name*: the name the top bar shows for the document, with its extension — for a never-saved document the default name (`pincel.aseprite` in Pixel mode, `image.ora` in Image mode today). *Pending action*: the action that triggered the prompt. *Running save*: a File ▸ Save or Save As (from the menu, a shortcut or the prompt) that has started and not yet finished; autosave snapshots are not saves. *Keys*: Ctrl means Ctrl on Windows and Linux and ⌘ Command on macOS, Alt means ⌥ Option on macOS — the `mod` convention of `ui/src/lib/shell/shortcuts.ts`.

## Scope

### In scope

- The same unsaved-changes prompt (title, text, buttons, keys) in front of every action from AC-01, in both modes
- Save from the prompt: the same save flow as File ▸ Save, continuing only when the save succeeded
- Desktop app: closing the window and quitting ask first while the document has unsaved changes
- A cancelled File ▸ Save / Save As no longer reports a failure (AC-12)

### Out of scope

- A list of several unsaved documents (Save all / Discard all) — because only one document is open at a time; the planned `document-tabs` spec extends this prompt to a list
- A Save button in the PWA's close / reload warning — because browsers only allow their own generic leave-page dialog there; the autosave recovery (`autosave-recovery`) is the safety net, and `app-shell` AC-15 keeps covering that warning
- Saving automatically on close without asking — because the user decides what happens to the work
- macOS Dock ▸ Quit and a Quit sent by the system (app switcher, log-out) — because macOS terminates the app directly and Tauri forwards no event the UI could answer; the autosave recovery is the safety net (owner decision 2026-11-04)
- Forced ends (operating-system shutdown or log-off, killing the process) — because the operating system does not wait for an answer; the autosave recovery covers them
- Prompts for unsaved state that is not the document (an open effect dialog, an unconfirmed transform, a palette edit in progress) — because those are confirmed or cancelled inside their own UI
- Dropping several files at once — because the shell opens one document; which of several dropped files it takes stays as it is today

## Acceptance criteria

- [ ] AC-01 (state): While the open document has unsaved changes, each of these actions shows the unsaved-changes prompt before it replaces the document: File ▸ New… (before the New dialog opens), File ▸ Open… (before the file picker opens), Open Recent (from the shell and from the Pixel editor's own File menu), dropping a file onto the window, Paste as New Image, and a file handed to Pincel by the operating system while it runs (desktop file association, or a file passed to a second launch that the running app receives). For a dropped, recent, handed-over or pasted file, the file or clipboard is read and its format recognized first, so the prompt only appears for something that can actually be opened (AC-11).
- [ ] AC-02 (always): The prompt is titled "Unsaved changes", asks 'Save changes to "<document name>"?' and offers the buttons Save, Don't Save and Cancel; Save has the focus when it opens, Tab and Shift+Tab cycle the focus among the three buttons without leaving the prompt, Enter activates the focused button, Escape chooses Cancel; while it is open, no editor or menu shortcut runs — checked with B, E, M, Ctrl+Z, Ctrl+S, Ctrl+O, Delete and Space, none of which changes the document, the canvas or the open dialogs.
- [ ] AC-03 (event): When the user chooses Save, then the document is saved exactly as File ▸ Save would save it — a Save As picker, format or flatten question appears where File ▸ Save would show one — and once the save has succeeded the unsaved marker is gone and the pending action runs. In the PWA without file-system access a save counts as succeeded once the download has been handed to the browser.
- [ ] AC-04 (error): If the save started from the prompt is cancelled (Save As picker, format or flatten question dismissed) or fails, then the pending action does not run and the document stays open with its unsaved changes and its unsaved marker; a failed save shows the same error message File ▸ Save shows, a cancelled one shows none.
- [ ] AC-05 (event): When the user chooses Don't Save, then the pending action runs without saving; once it replaces the document or closes the app, those unsaved changes are gone, and the recovery dialog never offers them (unless the recovery store could not be written, see Edge cases).
- [ ] AC-06 (event): When the user chooses Cancel (button or Escape), then the prompt closes, the pending action does not run, and the document, its undo history and its unsaved marker are unchanged.
- [ ] AC-07 (state): While the open document has no unsaved changes, every action from AC-01 runs at once without a prompt.
- [ ] AC-08 (state): While the prompt is open — including while a save started from it is running — a further action from AC-01 or AC-09 is ignored: no second prompt opens, its action does not run, and the open prompt still answers the first action. While a save started outside the prompt is running, a further action from AC-01 is ignored as well.
- [ ] AC-09 (event, manual: needs the desktop app on Windows, macOS and Linux; evidence names OS, version and date per close path): When the user closes the desktop window (title-bar ×, Alt+F4, the window's taskbar menu, macOS File ▸ Quit / Cmd+Q) while the document has unsaved changes, then the window stays open and the prompt shows; Save (once it succeeded) or Don't Save closes the app, Cancel keeps it running; with no unsaved changes, or on the start screen, the app closes at once.
- [ ] AC-10 (event, manual: needs the desktop app; provoke with a large Image document saved to a slow drive): When the user closes the desktop window while a save started outside the prompt is running (File ▸ Save or Save As), then the app waits for the save to finish; if it succeeded and nothing changed since, the app closes at once; if it failed, or the document has unsaved changes again, the prompt shows.
- [ ] AC-11 (error): If a dropped file, a file handed over by the operating system or a recent file cannot be read (missing, permission denied) or is not a supported format, or Paste as New Image finds no image on the clipboard, then the existing error message shows, no prompt appears and the open document stays unchanged; if a file picked in File ▸ Open… after Save or Don't Save cannot be opened, its error message shows and the open document stays open as it was after that answer.
- [ ] AC-12 (event): When the user dismisses the Save As picker of File ▸ Save or File ▸ Save As in either mode, then no error or failure message appears and the document keeps its unsaved changes.
- [ ] AC-13 (always): The prompt's strings exist in English and German: "Unsaved changes" / "Ungespeicherte Änderungen", 'Save changes to "<name>"?' / 'Änderungen an „<name>“ speichern?', Save / Speichern, Don't Save / Nicht speichern, Cancel / Abbrechen.

## Technical notes

Hints, verified 2026-11-04; paths and line numbers drift.

- Today's guards: the shell's `guard()` and confirm dialog in `ui/src/App.svelte:77` / `:377` (strings `shell.discardPrompt`, `shell.discard`), Pixel's own `guardUnsaved` / `dialogs.confirm` in `ui/src/modes/pixel/file-ops.ts:74` (`px.discardNamed`). Both go away in favor of the one prompt.
- `onDropFile` (`ui/src/App.svelte:198`) calls `openFile` directly, not through `guard()` — check whether dropping onto an open editor reaches it unguarded today.
- Saving from the shell needs a way to ask the mounted editor to save and learn the outcome. Image mode's `saveDocument()` (`ui/src/modes/image/core/save-flow.ts:45`) already resolves `null` on cancel; Pixel mode's `save()` (`ui/src/modes/pixel/file-ops.ts:157`) returns nothing and reports a dismissed picker as a failure in the status bar (AC-12). A natural place is a callback in `EditorProps` (`ui/src/lib/shell/types.ts`) the editor registers on mount, beside `registerMenuHandlers`.
- Desktop close: `getCurrentWindow().onCloseRequested` from `@tauri-apps/api/window` (already a dependency) lets the UI prevent the close and decide; the window then closes with `destroy()`, which needs `core:window:allow-destroy` in `src-tauri/capabilities/default.json`. macOS's `PredefinedMenuItem::quit` (`src-tauri/src/main.rs:328`) exits without a window close request, so Quit becomes a custom item that emits a menu event the UI handles like a close request.
- Recovery after Don't Save (AC-05): replacing a document already clears its snapshot (`AutosaveSession.opened()`), and unmount deletes it (`close()` in `ui/src/lib/idb/autosave-session.svelte.ts`); `destroy()` does not unmount the editor, so on close the snapshot is dropped explicitly first. If IndexedDB fails there, the app still closes.
- Existing e2e specs that click "Discard changes" (e.g. `ui/e2e/paste-new-image.spec.ts`, `ui/e2e/image-clipboard.spec.ts`, `ui/e2e/recovery.spec.ts`) move to "Don't Save".

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Document was never saved and the user chooses Save | The Save As picker opens; dismissing it is AC-04: nothing happens, the document stays open |
| Image document with layers saved to a flat format | The flatten question appears as for File ▸ Save; dismissing it is AC-04 |
| Save fails (disk full, permission denied, file locked) | Error message as for File ▸ Save, pending action does not run, document stays dirty |
| User clicks the window × twice, or drops a second file, while the prompt is open | Ignored (AC-08) — because the user's answer must apply to the action they are reading about |
| Don't Save before File ▸ New…, then Cancel in the New dialog | The document stays open with its unsaved changes — Don't Save only lets the action proceed; nothing is discarded until a new document replaces it |
| Save or Don't Save before File ▸ Open…, then the Open picker is dismissed | The document stays open (saved after Save, still dirty after Don't Save) |
| File opened from the OS belongs to the other mode | Same prompt; after Save or Don't Save the other mode's editor opens the file |
| File opened from the OS is the file already open | Same prompt; after Save or Don't Save the file is opened again from disk |
| Close request with unsaved changes while the New dialog or an editor dialog is open | The prompt shows above it; Cancel returns to that dialog, Save / Don't Save close the app |
| Close request while a document is still loading | The app closes at once if no document with unsaved changes is open — after Don't Save the old document's changes were already discarded |
| Save from the prompt while a transform, a floating selection or an effect preview is unconfirmed | Saves exactly what File ▸ Save would save in that state |
| Don't Save on close, and the recovery store (IndexedDB) cannot be written | The app still closes; the discarded changes may then be offered on the next start — closing must never hang on a storage error |
| User edits, then undoes back to the saved state, then closes | No prompt (no unsaved changes, see Terms) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Save has the focus when the prompt opens | Matches paint.net and Office, and pressing Enter then never loses work | Users used to "Enter = Discard" from today's dialog press Enter and get a save picker — harmless |
| Don't Save means the changes are gone for good, also for recovery (AC-05) | The user explicitly chose to discard; offering it again on the next start contradicts that choice | A mis-click loses the work for good, as in every desktop editor |
| A close waits for a running save (AC-10) | Ending the app mid-write could leave a half-written file | A hanging save blocks closing until it ends or fails |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->


## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/unsaved-prompt.spec.ts`: "File ▸ Open… asks before the picker opens (AC-01)", "dropping a file onto an open editor asks first (AC-01)", New in every "… (AC-02)" / "… (AC-03)" test; `ui/e2e/paste-new-image.spec.ts`: "pixel: Paste as New Image asks about unsaved changes first (AC-23)"; `ui/e2e/unsaved-close.spec.ts` (mocked Tauri bridge): "a file handed over by the OS asks first (AC-01)", "Open Recent asks first, from both editors; a missing recent shows its error (AC-01, AC-11)" | built | |
| AC-02 | `ui/e2e/unsaved-prompt.spec.ts`: "{pixel,image}: the prompt names the document and offers Save, Don't Save, Cancel (AC-02)", "keys pressed while the prompt is open reach neither editor nor menus (AC-02)", "Enter on Don't Save runs the pending action (AC-02, AC-05)"; `ui/e2e/unsaved-close.spec.ts`: "native-menu items reach no editor while the prompt is open (AC-02)" | built | |
| AC-03 | `ui/e2e/unsaved-prompt.spec.ts`: "{pixel,image}: Save saves like File ▸ Save, then runs the action (AC-03)", "Save counts as done once the download is handed over, without file-system access (AC-03)" | built | |
| AC-04 | `ui/e2e/unsaved-prompt.spec.ts`: "{pixel,image}: a cancelled save from the prompt keeps the document and skips the action (AC-04)", "{pixel,image}: a failed save from the prompt shows File ▸ Save's error and skips the action (AC-04)" | built | |
| AC-05 | `ui/e2e/unsaved-prompt.spec.ts`: "{pixel,image}: Don't Save runs the action; cancelling the New dialog keeps the document (AC-05)"; `ui/e2e/recovery.spec.ts`: "a discarded document is not offered again, on New or after a reload" | built | |
| AC-06 | `ui/e2e/unsaved-prompt.spec.ts`: "{pixel,image}: Cancel keeps the document, its history and its marker (AC-06)", Escape in "… offers Save, Don't Save, Cancel (AC-02)" | built | |
| AC-07 | `ui/e2e/unsaved-prompt.spec.ts`: "a clean document runs every action at once (AC-07)"; `ui/e2e/unsaved-close.spec.ts`: "a clean document opens a handed-over file and a recent at once (AC-07)", "without unsaved changes, or on the start screen, the app closes at once (AC-09, AC-07)" | built | |
| AC-08 | `ui/e2e/unsaved-prompt.spec.ts`: "a second action while the prompt is open is ignored (AC-08)", "a second action while the prompt's save runs is ignored (AC-08)", "an action while a File ▸ Save runs is ignored (AC-08)"; `ui/e2e/unsaved-close.spec.ts`: "closing with unsaved changes asks; Cancel keeps the app running (AC-09, AC-06)" (second close ignored) | built | |
| AC-09 | automated against a mocked Tauri bridge in `ui/e2e/unsaved-close.spec.ts`: "closing with unsaved changes asks; Cancel keeps the app running", "Don't Save closes the app and recovery never offers the work", "Save closes the app once the save succeeded", "a dismissed save picker keeps the app open", "without unsaved changes, or on the start screen, the app closes at once", "macOS Quit (menu:quit) asks like a window close"; manual check on Windows / macOS / Linux still open | open | |
| AC-10 | automated against a mocked Tauri bridge in `ui/e2e/unsaved-close.spec.ts`: "a close during a running save waits for it, then closes (AC-10)", "a close during a save that fails asks after it (AC-10)"; manual check still open | built | |
| AC-11 | `ui/e2e/unsaved-prompt.spec.ts`: "a dropped file that cannot be opened shows its error and no prompt (AC-11)", "clipboard › Paste as New Image with no image on the clipboard shows no prompt (AC-11)", "a file picked after Don't Save that cannot be opened leaves the document as it was (AC-11)"; `ui/e2e/unsaved-close.spec.ts`: "a handed-over file that cannot be read shows its error and no prompt (AC-11)", "Open Recent asks first, from both editors; a missing recent shows its error (AC-01, AC-11)" | built | |
| AC-12 | `ui/e2e/unsaved-prompt.spec.ts`: "{pixel,image}: dismissing the Save As picker reports no failure (AC-12)" | built | |
| AC-13 | `ui/e2e/unsaved-prompt.spec.ts`: "German › the prompt speaks German (AC-13)" (strings `shell.unsavedChanges`, `shell.saveChangesTo`, `shell.save`, `shell.dontSave`, `common.cancel` in `ui/src/lib/i18n/{en,de}.ts`) | built | |

## Decisions

- 2026-11-04: One prompt with Save / Don't Save / Cancel replaces both "Discard changes" guards, and the desktop app asks before closing — the owner wants to decide what happens to unsaved work instead of the app closing or discarding. Supersedes `app-shell` AC-14.
- 2026-11-04: A second trigger while the prompt is open is ignored, and a close during a running save waits for it (owner's choice in the clarification pass).
- 2026-11-04 (owner: macOS uses ⌘/⌥): Ctrl in this spec means ⌘ on macOS and Alt means ⌥ — the `mod` convention of `shortcuts.ts`. The prompt blocks every key while open, the Meta chords included (tested with Meta+Z / Meta+S / Meta+O); it shows no shortcut hints.
- 2026-11-04: The prompt lives in the shell (`ui/src/lib/shell/unsaved.svelte.ts`, `UnsavedPrompt.svelte`); editors hand over their document through a new `EditorProps.registerDocument` (name, dirty, save, running save, discard snapshot) and Pixel mode's own Open / Open Recent ask through `EditorProps.confirmUnsaved`. `EditorProps.onDirtyChange` and `session.dirty` are gone — the shell reads `dirty()` live, so a save that just finished is seen at once (AC-10). Reason: one prompt for both modes, no second source of truth for the dirty state.
- 2026-11-04: While the prompt is open a capture-phase window key listener stops and prevents every key (Space included, which would otherwise click the focused Save button); Tab / Shift+Tab, Enter and Escape are handled by the prompt itself. Reason: AC-02 lists Space among the keys that must change nothing.
- 2026-11-04: After Save, Cancel or a failed / cancelled save the prompt closes; it is not re-shown. Reason: the spec only requires that the pending action does not run (AC-04).
- 2026-11-04: Pixel-mode Save now writes through `writeTarget` and treats a dismissed picker as "nothing written" — before, `saveBytes` returned the old target on cancel and the document was marked saved without being written. Image mode already did this; its AC-12 test was green on the first run.
- 2026-11-04: Dropping a file onto an open editor is new: the shell listens for file drops on the window while an editor is mounted (before, only the start screen took drops). In the desktop app the webview's own drag-drop handling (Tauri `dragDropEnabled`, default on) may keep HTML drops from arriving at all — unchanged by this spec, as for the start screen.
- 2026-11-04: "A file picked in File ▸ Open… that cannot be opened" (AC-11) is checked for unreadable and unrecognized files, as everywhere in the shell; a file whose format is recognized but whose content is corrupt still replaces the editor with its error, as before.
- 2026-11-04: Desktop close: the shell registers `getCurrentWindow().onCloseRequested`, always prevents the default close and decides itself (`ui/src/lib/shell/close-guard.ts`), closing with `destroy()` (`core:window:allow-destroy` added to `src-tauri/capabilities/default.json`). macOS File ▸ Quit is a custom `menu:quit` item (Cmd+Q) instead of the predefined one, handled by the shell like a close request. Reason: the spec's technical notes; the predefined Quit exits without a close request.
- 2026-11-04: After Don't Save on close, dropping the autosave snapshot waits at most 2 s, then the window closes anyway. Reason: "closing must never hang on a storage error" (Edge cases).
- 2026-11-04: A close request while an earlier one waits for a running save is ignored, like a second close while the prompt is open. Reason: AC-08's rule, applied to the waiting state.
- 2026-11-04: The desktop paths (close, Quit, OS hand-over, Open Recent by path) are automated in Playwright against a mocked `__TAURI_INTERNALS__` bridge (IPC answers, virtual disk, emitted events); the real window on each OS stays the manual part of AC-09 / AC-10. Reason: the browser e2e has no Tauri runtime.
- 2026-11-04: Re-opening a recent through the shell (Image mode's Open Recent, or a recent of the other mode) lists it again under a new id — behavior from before this spec, not changed here; the e2e test clicks the first of the duplicate rows.
- 2026-11-04: While the prompt is open (or its save runs) the shell hands native-menu events to no editor, so macOS accelerators (Cmd+Z, Cmd+S, Cmd+E, zoom), which arrive as `menu` events rather than DOM keys, change nothing (AC-02); New / Open / Open Recent / Quit are ignored by the prompt's own rule (AC-08). Found by the first verification round.
- 2026-11-04: The clean-document test for Open Recent and the OS hand-over (AC-07) was green on its first run: the behavior already held, the test pins it.
- 2026-11-04: Pixel mode's own Open Recent reads the file first; a recent of another format goes to the shell (which recognizes it and asks), an `.aseprite` asks and then opens in place. Reason: AC-01 / AC-11 want the prompt only for a file that can be opened.
- 2026-11-04: Owner accepted the macOS Dock ▸ Quit gap: AC-09 no longer names the Dock menu (back to `open`, its automated tests unchanged), and Dock / system Quit is out of scope — no native `applicationShouldTerminate:` hook (would need `unsafe` Objective-C) and no waiting for upstream.
