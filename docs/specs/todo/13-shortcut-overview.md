---
id: todo-shortcut-overview
title: Help ▸ Keyboard Shortcuts overview
status: todo
size: S (one session)
mode: both
depends_on: []
created: 2026-10-28
---

# Help ▸ Keyboard Shortcuts overview

## Goal

**Help ▸ Keyboard Shortcuts** (and **Ctrl+/**) opens a searchable dialog listing every shortcut of the current mode:
- grouped by File, Edit, Select, View, Tools and Frames (Pixel);
- shown in the platform's notation (⌘ on macOS, Ctrl elsewhere);
- generated from the keymap, never hand-written, so it can't drift.

## Why

Today shortcuts appear only in menu hints and tooltips; there is no overview. New users can't discover tool keys like M, Q, W or U, which cycle variants. `21-power-user-shortcuts.md` builds the editable keymap **on top of this dialog**, so this comes first.

## Current state (read these first)

**`ui/src/lib/shell/shortcuts.ts`.** The single keymap (spec §9.4):
- `ShortcutAction` and `SHORTCUTS` (the primary chord plus alternates);
- `TOOL_GROUP_KEYS` (group id → key);
- `hint(action)` and `toolHint(group)`, which format a chord for display. **Reuse their formatting.**
- `isMac()` comes from `ui/src/lib/platform`.

**Help menu.** `ui/src/lib/help/menu.ts`: `helpMenus()` appends **About** and the updater items, **only where no native menu exists** (macOS has a native Help menu, built in `src-tauri/src/main.rs`).

**Dialog pattern.** `ui/src/lib/components/AboutDialog.svelte` and `help.about` in `ui/src/lib/help/state.svelte.ts`.

**Labels.** Menu labels are i18n `menu.*` keys (`ui/src/lib/i18n/en.ts`), and tool labels are `tool.*`. Most actions already have a menu label to reuse.

**Tool groups per mode.** Pixel `toolGroups()` in `ui/src/modes/pixel/tools.ts`; Image `toolGroups` in `ui/src/modes/image/ImageEditor.svelte`.

## Design

**Data.** `shortcutList(mode)` in `shortcuts.ts` or next to it. It builds rows `{ group, label, chords[] }` from:
- `SHORTCUTS`, mapping each action to its `menu.*` label key. Add a small `ACTION_LABEL` map and a type check that every action has a label;
- the mode's tool groups plus `TOOL_GROUP_KEYS`, with the note "press again to cycle variants" on groups with more than one tool;
- fixed gestures: Space-drag pans, Alt + tool for the temporary eyedropper (Pixel), Shift / Alt selection modifiers, two-finger pan / zoom.

Rows only list actions **the current mode binds**. Each editor already knows which actions it handles (its `runShortcut` cases), so export the list of supported actions per mode.

**Dialog.** `ShortcutsDialog.svelte` in `ui/src/lib/components/`:
- a filter field, groups as sections, and `<kbd>` styling with `pc-*` tokens;
- Escape closes it, `role="dialog"`, and the filter field gets focus;
- it is printable (a simple CSS print style).

**Entry points.**
- **Help ▸ Keyboard Shortcuts.** Add it in `helpMenus()` and in the native macOS Help menu (a new menu id in `src-tauri/src/main.rs` that emits an event the shell opens the dialog on).
- **Ctrl+/** as a new `ShortcutAction` `shortcuts`.
- `?` is common too, but conflicts with typing in some layouts; skip it.

**i18n.** The dialog strings in en and de.

## Tasks (one S session)

1. **`docs(spec)`.** Add the overview to §9.4 and §11.3 (Help menu).
2. **`feat(ui)`** (+ `feat(tauri)` for the native Help entry).
   - The list builder with a test that every `ShortcutAction` has a label, the dialog, the entry points and i18n.
   - e2e `ui/e2e/shortcuts-overview.spec.ts`:
     - Ctrl+/ opens the dialog in both modes;
     - "Undo" is listed with `Ctrl+Z`;
     - filtering "lasso" leaves only the Lasso row (key Q);
     - Pixel mode lists the frame keys and Image mode doesn't.

## Out of scope

- Editing or rebinding (`21-power-user-shortcuts.md`).
- A command palette (also 21).
