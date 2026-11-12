---
status: todo
created: 2026-11-12
size: M
mode: both
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §9.5]
---

# Settings dialog and a tidy View menu

## Context

The View menu mixes three kinds of entries: what the canvas shows (zoom, rulers, grids, units, hiding the interface), app settings (Reduce Motion, Touch Gestures, Language) and the AI setup (AI Agents…, Image Generation…). The owner asked for a consistent View menu (2026-11-12, screenshot of the German menu) and chose a settings dialog for the app settings: Edit ▸ Settings… with Ctrl+, (⌘, on macOS), as in most desktop apps. View then holds only zoom, view aids and the interface toggles. The AI entries leave View when the AI panel arrives (`ai-panel`, owner's choice of the same day); until then they stay where they are.

## Scope

### In scope

- A "Settings" dialog owned by the shell, opened from Edit ▸ Settings… in both editors, Ctrl+, / ⌘, and a "Settings" button on the start screen where no native menu exists
- The dialog's settings: Language, Reduce motion (with the note when the system asks for it), Touch gestures — the existing prefs and setters, unchanged in effect and persistence
- Removing Reduce Motion, Touch Gestures and Language from View in both editors
- The macOS native menu: Edit ▸ Settings… (⌘,)
- The shortcut as a rebindable action, listed in the Keyboard Shortcuts overview
- en + de strings

### Out of scope

- New settings — because the dialog first gathers the existing ones; later settings can join it.
- The AI entries in View (AI Agents…, Image Generation…) — because they move into the AI panel (`ai-panel`).
- The start screen's language picker — because it serves a first run before any settings are known; it stays.

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses Edit ▸ Settings… in either editor, then a modal dialog "Settings" opens with Language (each language labelled in its own language, the active one selected), Reduce motion and Touch gestures.
- [ ] AC-02 (event): When the user presses Ctrl+, (⌘, on macOS) in either editor, or on the start screen with focus outside a text field and no modal dialog open, then the Settings dialog opens; the Keyboard Shortcuts overview lists the action "Settings" with that key, and it can be rebound like the other actions.
- [ ] AC-03 (state): While the app has no native menu bar (browser, Windows, Linux), the start screen shows a "Settings" button next to Tour and About that opens the dialog.
- [ ] AC-04 (event): When the user changes a setting in the dialog, then it takes effect at once, exactly as the former View menu entry did (`i18n` AC-04, `reduced-motion` AC-09 / AC-13, `tablet-gestures` AC-12), and is kept after a reload.
- [ ] AC-05 (state): While the system requests reduced motion and the user's own option is off, the Reduce motion setting is unchecked and notes that the system setting is on ("On in system settings").
- [ ] AC-06 (event): When the user presses Escape or Close in the dialog, then it closes and focus returns to where it was; keys pressed while it is open never reach the editor (no tool switch, no undo).
- [ ] AC-07 (always): The View menu of both editors lists no Reduce Motion, Touch Gestures or Language entry; it holds zoom, the view aids of its mode, Hide Interface (and Pixel mode's probe), followed by the AI entries until `ai-panel` moves them.
- [ ] AC-08 (event, manual: needs macOS): When the user chooses Edit ▸ Settings… or presses ⌘, in the macOS desktop app, on the start screen or in an editor, then the Settings dialog opens.
- [ ] AC-09 (option): Where the UI language is German, the menu entry reads "Einstellungen…" and the dialog "Einstellungen", with "Sprache", "Bewegung reduzieren" and "Touch-Gesten".

## Technical notes

Hints, not contracts.

- Setters and stores exist: `setLocale` / `LOCALES` / `locale()` (`ui/src/lib/i18n/locale.ts`, `index.svelte.ts`), `setReduceMotion` / `motion.option` / `motion.system` (`ui/src/lib/motion.svelte.ts`), `setTouchGestures` / `touchGestures.enabled` (`ui/src/lib/shell/touch-gestures.svelte.ts`). The View items `reduceMotionMenuItem()`, `touchGesturesItem()`, `languageMenu()` go away with their callers.
- Shell ownership like About: an `openSettings()` state next to `openAbout` (`ui/src/lib/help/state.svelte`) and the dialog mounted once in `App.svelte`; dialog pattern as `AboutDialog.svelte` (focus, Escape, `stopPropagation`).
- Shortcut: a new `ShortcutAction` `'settings'` in `ui/src/lib/shell/shortcuts.ts` with `{ key: ',', mod: true }` (free: `prevFrame` is `,` without a modifier); on macOS in `NATIVE_MENU_ACTIONS` with the native id `menu:settings` (`src-tauri/src/main.rs` `build_menu`, Edit submenu; labels in its Rust label table).
- Menus: `ui/src/modes/pixel/menus.ts` (View ≈L288, Edit ≈L173), `ui/src/modes/image/ImageEditor.svelte` (View ≈L1554, Edit ≈L1364).
- e2e specs that use the moved entries move to the dialog: `ui/e2e/gestures.spec.ts`, `ui/e2e/i18n.spec.ts`, `ui/e2e/reduced-motion.spec.ts` (grep `'Reduce Motion'`, `'Touch Gestures'`, `'Language'` with `menu(`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Ctrl+, while a text field has focus (layer name, a dialog field) | The field keeps the key; no dialog |
| Ctrl+, while another modal dialog is open | Nothing opens |
| Settings opened, language switched | The dialog itself switches language at once |
| IndexedDB unavailable | Settings take effect for the session; nothing is stored (as before) |
| The user rebinds Settings to another chord | The overview and the menu hint show the new chord (`power-user-shortcuts-keymap`) |

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

## Decisions

- 2026-11-12: App settings move from View into a Settings dialog (Edit ▸ Settings…, Ctrl+, / ⌘,) — the owner asked for a consistent View menu and chose the dialog over an Extras submenu; Edit ▸ Settings… is where Windows and Linux apps put it, ⌘, is the macOS convention, and a dialog has room for settings that come later.
- 2026-11-12: On macOS the native entry sits in Edit, not in the application menu — the native menu bar builds no application menu of its own today (`build_menu` in `src-tauri/src/main.rs`), and Edit matches the in-app menus of the other platforms.
- 2026-11-12: The start screen keeps its language picker and gains a Settings button only where no native menu exists — the picker serves a first run; on macOS ⌘, and the native menu reach the dialog.
