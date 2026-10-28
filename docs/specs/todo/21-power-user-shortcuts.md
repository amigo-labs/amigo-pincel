---
id: todo-power-user-shortcuts
title: Power-user keyboard — full coverage, custom keymap, command palette
status: todo
size: L (three M sessions)
mode: both
depends_on: [todo-shortcut-overview (13) for the overview dialog it extends]
created: 2026-10-28
---

# Power-user keyboard: full coverage, custom keymap, command palette

## Goal

Three things for people who live on the keyboard:
1. **Coverage.** Every frequent action has a default shortcut, not just file, undo, selection and zoom.
2. **Custom keymap.** Any menu action or tool can be rebound. Conflicts are shown, and the keymap can be exported and imported as JSON.
3. **Command palette.** Ctrl+K (⌘K) opens a search over every menu command and tool. Type, pick, run. It shows each command's shortcut, so it also teaches them.

## Why

The user asked for it. Aseprite users expect dense keyboard control: Aseprite has keys for nearly every command and a keymap editor, and since 1.3.16 it has Run Command. paint.net, Krita and Photoshop all have customizable shortcuts.

Pincel today has:
- 22 shared actions (`SHORTCUTS`) and the tool-group keys;
- no rebinding;
- no command search.

## Current state (read these first)

**`ui/src/lib/shell/shortcuts.ts`.** This is the single keymap (spec §9.4).
- `ShortcutAction` union, with `Chord { key, mod?, shift?, alt? }`.
- `SHORTCUTS` holds the primary chord plus alternates.
- `TOOL_GROUP_KEYS`.
- `NATIVE_MENU_ACTIONS`: the actions the native macOS menu fires as accelerators.
- `actionFor(e)`, `hint(action)` (menu labels), `toolHint`, `isTypingTarget`.

**Dispatch.** `runShortcut(action)` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L336) and in `ui/src/modes/image/ImageEditor.svelte`. Each mode binds the actions it supports. Unknown actions return `false`, so the browser default still happens; `paste` relies on that.

**Menus as data.** Both editors build `MenuDef[]` with `{ label, shortcut, disabled, run }` items (`ui/src/lib/components/layout/menu.ts`; the Pixel menus are in `ui/src/modes/pixel/menus.ts`). **These are the command source for the palette.**

**Native macOS menu.** It is built in Rust (`src-tauri/src/main.rs`) with fixed accelerators, so custom bindings must keep it in sync or fall back to in-app handling. Decision Log 2026-10-27 covers the macOS-only native menu.

**Persistence.** IndexedDB prefs.

**Overview dialog.** `13-shortcut-overview.md` adds Help ▸ Keyboard Shortcuts; this spec makes that dialog editable.

## Design

### 1. Coverage: new default actions

All new actions go into `ShortcutAction`, following the conventions of Aseprite, Photoshop and paint.net. Before taking a key, check it against browser-reserved shortcuts: Ctrl+W, Ctrl+T, Ctrl+N and Ctrl+Tab can't be captured in a browser tab. Use the Tauri app or the installed PWA window, or pick another key.

| Action | Default | Note |
|--------|---------|------|
| New layer | Ctrl+Shift+N | |
| Duplicate layer | Ctrl+J | |
| Merge down | Ctrl+Shift+E | Ctrl+E is Export today; keep that |
| Delete layer | Ctrl+Shift+Delete | |
| Layer up / down (select) | Alt+↑ / Alt+↓ | Alt+←/→ are taken by grips (§9.2) |
| New frame / duplicate frame (Pixel) | Alt+N / Alt+D | Aseprite-like; check against tool keys |
| Flip horizontal / vertical | Shift+H / Shift+V | |
| Rotate 90° CW / CCW | Ctrl+R / Ctrl+Shift+R | Not in the browser: Ctrl+R reloads. The Tauri app only; elsewhere no default |
| Toggle UI (hide panels and rail) | Tab | Photoshop |
| Toggle pixel grid / grid | Ctrl+' / Ctrl+Shift+' | once `09-pixel-grid.md` exists |
| Brush opacity 10 %…100 % (Image) | Shift+1…Shift+0 | 1 and 0 are zoom keys today |
| Next / previous palette color (Pixel) | ] / [ with Alt | [ / ] are brush size |
| Command palette | Ctrl+K | |
| Keyboard shortcuts overview | Ctrl+/ | |

A **checked conflict table** in a unit test asserts no two actions share a chord within a mode. The test lives with `shortcuts.ts`; set up Vitest if it doesn't exist yet. That needs approval as a new dev dependency (CLAUDE.md §7.4). Otherwise use an e2e test that loads the map in the page and checks it.

### 2. Custom keymap

**Model.**
- A `Keymap = Partial<Record<ShortcutAction | `tool:${string}`, Chord[]>>` of **user overrides** on top of the defaults. An empty array means unbound.
- Stored in the IndexedDB pref `keymap`.
- `actionFor` / `hint` / `toolGroupForKey` read the effective map (defaults merged with overrides). This needs a reactive store in `ui/src/lib/shell/keymap.svelte.ts`.

**Editor.** In the shortcuts overview dialog (spec 13):
- every row gets **Change**: press the new chord, and the dialog shows a conflict warning with "replace" or "cancel";
- **Reset** per row, and **Reset all**;
- **Export** and **Import** as JSON. Validate the schema; unknown actions are ignored with a notice.

**Native macOS menu.** When a menu action is rebound, send the new accelerator through a new Tauri command, `set_menu_accelerators`, or drop that item's native accelerator and let the webview handle the key. Record the choice in the Decision Log.

**Platform.** A stored chord uses `mod`, not Ctrl or ⌘, so one keymap works on all OSes.

### 3. Command palette

- **Ctrl+K** opens a centered search box with a list. Escape closes it.
- **Entries:**
  - every enabled `MenuDef` item, flattened with its path, e.g. "Layer ▸ Merge Down";
  - every tool ("Tool: Magic Wand");
  - "Go to frame…" (Pixel).
- **Fuzzy match** on the label in the current language **and in English**, so English command names work in every locale. Recent commands come first.
- Each entry shows its shortcut via `hint()`.
- Running a command closes the palette and runs the item's `run`. Disabled items are shown dimmed and are not runnable.
- **Use the keyboard:** ↑/↓ moves, Enter runs. `role="dialog"` with a `combobox` + `listbox` pattern.

## Tasks (one M session each)

1. **`docs(spec)` + coverage.**
   - §9.4 gets the full default table, plus a Decision Log entry for the browser-reserved keys and the Ctrl+E / Ctrl+R choices.
   - The new actions are bound in both modes where they apply.
   - Conflict test.
   - e2e for a sample of new keys: Ctrl+Shift+N adds a layer, Shift+H flips, Tab hides the UI.
2. **Command palette.** Built from the menus and tools, with i18n. e2e: Ctrl+K, type "merge", Enter merges down, and undo works.
3. **Custom keymap.**
   - Store and pref, the editor in the overview dialog, export/import, native menu sync.
   - e2e: rebind Undo to Ctrl+U, check that it works and that the menu hint shows Ctrl+U, reload and it persists, Reset all.

## Out of scope

- Multi-key sequences (Emacs-style chords).
- Per-mode different bindings for the same action. Same key, same meaning (§9.4).
- Mouse-button and gesture remapping.
