---
status: todo
created: 2026-11-18
size: L
mode: shell
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §11.3]
---

# macOS: the native menu bar only

## Context

In the desktop app on macOS the window shows two menu bars (owner request 2026-11-18, "on macOS I only want the Mac menu bar at the top"):

- the **native menu bar** at the top of the screen. It has a fixed set of entries (File: New, Open, Open Recent, Close, Save, Save As, four exports, Print, Quit; Edit: Undo, Redo, Cut, Copy, Paste, Settings; View: three zoom entries; Help: Keyboard Shortcuts, Tour, About, Check for Updates). macOS shows its first menu (File) under the application's name, because the bar has no App menu of its own.
- the **in-window menu bar** in the editor's top bar. It has every editor menu (File, Edit, Select, Image, Layer, Frame, Effects, Adjustments, View, Tools, …), with dimmed entries, check marks, submenus and shortcut hints that follow the editor's state. On macOS it has no Help menu, because the native bar has one (Decision Log 2026-10-27).

The owner wants the in-window bar gone on macOS. Then the native bar has to offer everything the in-window bar offers today. The owner chose (2026-11-18):

- **Variant 1**: only the in-window *menus* go. The top bar keeps the Pincel wordmark, the mode badge, the document title with its unsaved-changes dot and the quick buttons.
- **Shortcuts**: every native entry whose shortcut uses ⌘, ⌃ or ⌥ shows that shortcut, and the native menu fires it (it "owns" the chord). Single keys (B, +, F2, Delete, arrows, Shift+letter) stay with the editor and are not shown in the native bar, because a native key equivalent without a modifier takes the key away from text fields.
- **App menu**: the macOS standard. About Pincel, Check for Updates…, Settings… ⌘,, Services, Hide Pincel ⌘H, Hide Others ⌥⌘H, Show All, Quit Pincel ⌘Q.

Terms:

- **macOS desktop app**: the Tauri build running on macOS, where `hasNativeMenu()` holds today. **Elsewhere**: the browser (on any OS, macOS included) and the desktop app on Windows and Linux.
- **Editor menus**: the menus the active editor tab lists (today rendered by the in-window bar), in their order, with their labels, dimmed state, check marks, separators and submenus.
- **Text field**: an element where keyboard input is typing (input, textarea, contenteditable — what the editors already treat as a typing target), including the hidden field of a live text object.
- **Modifier chord**: a shortcut with ⌘ or ⌃ (and optionally ⇧ / ⌥). An **option chord** uses ⌥ (with or without ⇧) but neither ⌘ nor ⌃. A chord is **expressible** when the native menu can draw it as a key equivalent: every modifier and option chord of a typed key is; a chord bound to a physical key position (the keymap's `code` chords, e.g. the bracket keys on a German layout) is not.
- **Formerly native-owned actions**: Open, Save, Save As, Export PNG (frame), Export Sheet, Print, Undo, Redo, Keyboard Shortcuts, Settings — the chords the native menu bound before this spec.
- **Updatable installation**: a release build the updater can update (`updater::supported()`), the condition under which Windows and Linux list Check for Updates… in their Help menu.
- **Text-editing entries**: Cut, Copy, Paste (Edit menu) and Select All (Select menu).
- **Inert menu**: as in `menu-structure` — while an effect is open every menu except Effects and Adjustments is inert; while an Assistant answer runs every menu except View is inert.

## Scope

### In scope

- macOS desktop app: no in-window menus, in either width (dropdowns on wide windows and the ☰ sheet on narrow ones).
- macOS desktop app: the native menu bar lists the App menu, then the editor menus of the active tab, then Help; it follows the editor's state as the in-window bar does.
- An App menu on macOS, with the standard entries; About Pincel, Check for Updates… and Settings… move there.
- Shortcuts in the native bar for modifier chords, owned by the native menu.
- Text fields keep cut / copy / paste / select all of their text.
- The start screen (no tab open) keeps a fixed native menu, now with the App menu in front.

### Out of scope

- Windows, Linux and the browser — because their only menu bar is the in-window one (Decision Log 2026-10-27), and the owner asked about macOS only.
- Moving the title, mode badge or quick buttons into the window's title bar (variant 2) — because the owner chose variant 1.
- A Window menu (Minimize ⌘M, Zoom, Bring All to Front) — because no menu bar has one today; it is its own small spec if wanted.
- Showing single-key shortcuts in the native bar — because AppKit would then take those keys away from text fields (owner decision above).
- Changes to the command palette, the menus' content or their order — because `menu-structure` owns those; this spec only changes where the menus are drawn on macOS.
- The macOS Dock menu — because it is not a menu bar, and its Quit stays as it is (AGENTS.md Gotchas).

## Acceptance criteria

- [ ] AC-01 (state): While the macOS desktop app shows an editor tab, the editor shows no in-window menu bar — neither the dropdown menus nor the ☰ button — at any window width; the Pincel wordmark, the mode badge, the document title with its unsaved-changes dot and the quick buttons stay in the top bar.
- [ ] AC-02 (always): Elsewhere (the browser on any OS, the desktop app on Windows and Linux) the editor shows its in-window menu bar as before, Help menu included, and sends no editor menus to a native menu.
- [ ] AC-03 (state): While the macOS desktop app shows an editor tab, the native menu bar lists, in this order: the App menu, then every editor menu of the active tab in the in-window order with the same labels, then Help. Each entry has the in-window label, the same dimmed state, check mark, separators and submenus (Open Recent and Export included).
- [ ] AC-04 (event): When the user picks an entry of an editor menu in the native bar, then the editor runs the same action the in-window entry runs, once.
- [ ] AC-05 (event): When the editor's state changes what the editor menus show (a dimmed entry becomes usable, a check mark flips, the recents list or the language changes, the active tab switches to the other mode, a tab of another document becomes active), then the native bar shows the new state within 200 ms.
- [ ] AC-06 (state): While an effect is open, every entry of the native editor menus except those of Effects and Adjustments is dimmed; while an Assistant answer runs, every entry except those of View is dimmed. The App menu and Help stay usable in both states, as the native Help is today.
- [ ] AC-07 (always): The App menu lists, in this order: About Pincel; Check for Updates… (only in an updatable installation); a separator; Settings… ⌘,; a separator; Services; a separator; Hide Pincel ⌘H, Hide Others ⌥⌘H, Show All; a separator; Quit Pincel ⌘Q. About opens the in-app About dialog, Check for Updates… checks as the Help entry does today, Settings… opens the Settings dialog, Quit asks about unsaved changes as File ▸ Quit does today.
- [ ] AC-08 (state): While the macOS desktop app shows an editor tab, the native Help menu lists Keyboard Shortcuts ⌘/, Tour and, in Pixel mode, the Frame-Time Probe (Image mode has no diagnostic entry); About Pincel and Check for Updates… are not in Help, and Settings… is not in Edit.
- [ ] AC-09 (state): While no tab is open (the start screen), the native menu bar shows the App menu of AC-07, then File (New ⌘N, Open… ⌘O, Open Recent), Edit (the system's Cut, Copy, Paste, Select All), and Help (Keyboard Shortcuts, Tour); picking New, Open… or a recent file works as today.
- [ ] AC-10 (state): While the macOS desktop app shows an editor tab and no text field has focus, every native entry whose action's first chord is an expressible modifier or option chord shows that chord, the editor does not act on that chord itself (the native entry fires it), and picking the entry runs the action once; an entry whose action has only a single-key shortcut, an inexpressible chord, or none, shows no shortcut, and the editor keeps handling that key.
- [ ] AC-11 (event): When the user rebinds an action in the Keyboard Shortcuts dialog, then its native entry shows the new chord and the new chord fires it; a chord rebound to a single key is no longer shown and the editor handles that key again. This includes Settings… in the App menu and Keyboard Shortcuts in Help: AC-07 / AC-08 name their default chords.
- [ ] AC-12 (state): While a text field has focus in the macOS desktop app, the native bar's text-editing entries are the operating system's own Cut ⌘X, Copy ⌘C, Paste ⌘V and Select All ⌘A (marked as system items in the menu data the editor sends; that they act on the field's text is AC-16), and the editor's own Cut / Copy / Paste / Select All entries are not listed; once the focus leaves the field, the editor's entries are back within 200 ms.
- [ ] AC-13 (state): While a text field has focus in the macOS desktop app, the native bar shows and binds chords only for the formerly native-owned actions and the text-editing entries; every other entry shows no shortcut, so its chord (an option chord included) goes to the field.
- [ ] AC-14 (error): If sending the menu to the native bar fails (the call is rejected), then the editor keeps working, no in-window menu bar appears in its place, the shortcuts the failed menu would have bound stay with the editor, and the next state change sends the menu again; the native bar keeps what it showed before (the start-screen menu if no editor menu ever arrived).
- [ ] AC-15 (event): When the last tab closes, then the native menu bar shows the start-screen menu of AC-09 again; when a tab opens from the start screen, the editor menus of AC-03 replace it.
- [ ] AC-16 (state, manual: AppKit key routing needs a real Mac): In a release build on macOS, the native menu bar shows the App menu under "Pincel" and the editor menus in the in-window order; from the canvas ⌘D deselects once and ⌘S saves once; in a layer name field ⌘C / ⌘V copy and paste the name's text, ⌘A selects it, and ⌥E then E types é.
- [ ] AC-17 (error): If the user picks a native entry that the editor's current menus no longer list, or list dimmed (a menu opened before a state change), then nothing happens.
- [ ] AC-18 (event): When the user rebinds an action to a chord the App menu uses (⌘, ⌘H ⌥⌘H ⌘Q), then the App menu keeps that chord and the rebound entry shows no shortcut. If two editor entries end up with the same chord, the first in menu order (menus left to right, entries top to bottom) shows and fires it, the others show none.

## Technical notes

- Who builds what: the desktop shell builds the App menu and the start-screen menu itself (labels from its own table in six languages); the renderer sends the editor menus, Help included. The App menu and start-screen ACs (AC-07, AC-09, AC-18's App-menu half) are proven by unit tests of the shell's menu description; the editor-menu ACs by e2e specs on the mocked bridge, which records the sent menu data.
- The renderer already has the editor menus as data (`MenuDef[]`, `ui/src/lib/components/layout/menu.ts`) with `run` closures; the macOS path serializes that tree (ids per entry, label, dimmed, checked, submenu, separator, accelerator), pushes it to the desktop shell over one new Tauri command, and keeps the closures in a table keyed by entry id. The shell rebuilds the native menu from it and emits the entry id back through the existing `menu` event (`ui/src/lib/menu/index.ts`). This is the cross-package API this spec adds; it gets a Decision Log entry.
- Accelerators come from the effective keymap (`ui/src/lib/shell/shortcuts.ts`, `menuAccelerator`); the editors' key handlers leave a chord to the native menu while that menu shows it (the existing `nativeMenuOwns` pattern, extended from the fixed set to whatever the native menu currently binds).
- Text-editing entries are AppKit's predefined items while a text field has focus (they send `cut:` / `copy:` / `paste:` / `selectAll:` to the web view), and the editor's own entries otherwise.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A Pixel tab and an Image tab are open; the user switches between them | The native bar shows the active tab's editor menus (Frame appears and disappears with Pixel mode) (AC-05) |
| An effect is open and the user picks Layer ▸ Duplicate in the native bar | Nothing happens: the entry is dimmed (AC-06) |
| The user presses ⌘S while the canvas has focus | The document saves once — not twice (AC-10) |
| The user types ⌥E (´) into a layer name field | The dead key types its accent into the field; no menu action fires (AC-13) |
| The user presses ⌘A with the focus in a text field | The field's text is selected; the document's selection is unchanged (AC-12) |
| The user picks Edit ▸ Copy with the mouse while the canvas has focus | The editor copies the selection, as the in-window entry does (AC-04) |
| An action has two chords (e.g. Redo ⌘⇧Z and ⌘Y) | The entry shows the first — because a native entry carries one key equivalent; the second still works through the editor, except while a text field has focus, where it goes to the field like any non-native key (AC-10, AC-13) |
| A native entry is picked from a menu opened before the entry was dimmed | Nothing happens (AC-17) |
| The user rebinds Undo to ⌘Q | Quit keeps ⌘Q; Undo shows no shortcut (AC-18) |
| The first push of editor menus fails | The start-screen menu stays until the next state change succeeds (AC-14) |
| A shortcut can't be expressed as a native accelerator (a physical-key chord) | The entry shows no shortcut, and the editor handles the key (AC-10, AC-11) |
| The window is narrower than 640 px | No ☰ button; the native bar still has every menu (AC-01) |
| The editor tour reaches its last step ("The Help menu has …") on macOS | The step is shown without an in-window anchor, as for any step whose anchor is missing |
| The language changes in Settings | The editor menus, App menu and Help relabel in the new language (AC-05, AC-07) |
| The native menu update fails | The previous native menu stays; the next state change retries (AC-14) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| While a text field has focus only the formerly native-owned chords stay bound (AC-13) | Those chords reached the native menu from text fields before this spec, so Save / Undo while typing keep working; every other chord belonged to the field on every platform, and keeping that avoids e.g. ⌘D deselecting while renaming a layer | A user expects a further chord to work while typing |
| The start-screen menu (AC-09) drops Close, Save, Save As, the exports, Print, Undo, Redo, the zoom entries and moves Settings / About / updates to the App menu, and gains Select All | The dropped entries serve no document on the start screen (they did nothing there before); Select All keeps ⌘A for the New dialog's number fields | None |
| The App menu and Help stay usable while an effect is open or an answer runs (AC-06) | That is how the native bar behaved before this spec: About, Settings and the shortcut overview are guarded by the shell, which ignores them while a dialog of its own is open | A dialog opens over an open effect |
| A key event the page handles with `preventDefault` does not reach the native menu in WKWebView | Pages in Safari override ⌘S this way; the editors stop at the native menu's chords anyway (AC-10), so this only matters for chords bound twice by mistake | A chord fires twice; AC-16 checks it on a real Mac |
| The Frame-Time Probe stays a Help entry in the native bar | `menu-structure` AC-06 made it a palette command on macOS only because the native Help had no room for mode entries; the mirrored Help has | None |
| Undo and Redo keep acting on the document while a text field has focus | That is today's macOS behavior (native ⌘Z); changing it is a separate decision | A user expects text undo in a field |

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

## Decisions

- 2026-11-18: Variant 1 (only the in-window menus go), shortcuts for every ⌘ / ⌃ / ⌥ chord, the macOS standard App menu — owner answers in the spec dialogue.
- 2026-11-18: Option chords are not bound while a text field has focus — because on macOS ⌥ plus a letter types a character (´, ®, …), and the owner's "all ⌥ chords" would otherwise take those characters away from text fields.
- 2026-11-18: Second clarification pass — who builds the App / start-screen menu, system items marked in the menu data, chord clashes between editor entries (AC-18), failure = rejected call (AC-14).
- 2026-11-18: Clarification pass — key routing inside AppKit (which key reaches a field, the menu or the page) cannot be observed without a real Mac; AC-12 / AC-13 now state what the native bar offers (observable on the mocked bridge) and AC-16 checks the routing by hand. Added AC-17 (stale pick), AC-18 (rebind onto an App menu chord).
