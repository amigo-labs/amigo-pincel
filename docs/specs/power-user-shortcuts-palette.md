---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §9.5]
---

# Power-user keyboard: command palette

## Context

Aseprite has Run Command since 1.3.16; many editors offer a searchable command list. Pincel has no command search: every command lives in its menu, and users must know where.

**Ctrl+K** (⌘K on macOS) opens a command palette: a search over every menu command and every tool of the current mode. Type, pick, run. Each entry shows its shortcut, so the palette also teaches the keys. It is one of three parts of the legacy plan "Power-user keyboard" (with `power-user-shortcuts-coverage` and `power-user-shortcuts-keymap`); it doesn't depend on the other two.

## Scope

### In scope

- A command palette opened with Ctrl+K in both modes
- Entries: every menu command (with its menu path), every tool, and "Go to frame…" in Pixel mode
- Fuzzy matching on the label in the UI language and in English; recently run commands first
- Shortcut hints per entry; disabled commands shown dimmed and not runnable
- Keyboard operation and an accessible combobox / listbox pattern
- en + de strings

### Out of scope

- Rebinding from the palette — because `power-user-shortcuts-keymap` owns rebinding.
- Searching document content (layer, tag or slice names) — because the palette searches commands, and document content is never translated or treated as a command.
- Multi-step command arguments inside the palette (beyond "Go to frame…") — because commands with parameters open their existing dialogs.

## Acceptance criteria

- [ ] AC-01 (event): When the user presses Ctrl+K while no text field has focus, then the command palette opens with its search field focused — in Pixel and in Image mode.
- [ ] AC-02 (always): The palette lists every command of the current mode's menu bar, labelled with its menu path (e.g. "Layer ▸ Merge Down"), and every tool of the current mode (e.g. "Tool: Magic Wand").
- [ ] AC-03 (state): While in Pixel mode, the palette also offers "Go to frame…"; in Image mode it does not.
- [ ] AC-04 (event): When the user types a query, then only entries whose label fuzzy-matches the query remain, best matches first; typing "merge" lists "Layer ▸ Merge Down".
- [ ] AC-05 (option): Where the UI language is not English, a query in English also finds the command (with German UI, "merge" finds "Ebene ▸ Nach unten vereinen").
- [ ] AC-06 (always): Each entry that has a shortcut shows it, in the same text as its menu hint.
- [ ] AC-07 (event): When the user moves with ↑ / ↓ and presses Enter, then the highlighted command runs and the palette closes; with "merge" + Enter on a two-layer document the layers are merged down, and one Undo restores both layers.
- [ ] AC-08 (event): When the user clicks an enabled entry, then that command runs and the palette closes.
- [ ] AC-09 (state): While a command is disabled in its menu, its palette entry is shown dimmed and Enter or a click on it does nothing and keeps the palette open.
- [ ] AC-10 (event): When the user picks a tool entry, then that tool becomes active, as clicking it in the tool rail does.
- [ ] AC-11 (event): When the user runs "Go to frame…" and enters a frame number within the sprite's frames, then that frame becomes the current frame.
- [ ] AC-12 (error): If the user enters a frame number outside 1 … frame count or not a number in "Go to frame…", then the current frame does not change.
- [ ] AC-13 (event): When the user opens the palette after running commands from it, then the most recently run commands are listed first while the query is empty.
- [ ] AC-14 (event): When the user presses Escape, then the palette closes without running anything.
- [ ] AC-15 (always): The palette is a `role="dialog"` with a combobox search field controlling a listbox; the highlighted entry is announced as the active option.
- [ ] AC-16 (state): While the search matches nothing, the list is empty and Enter runs nothing.
- [ ] AC-17 (option): Where the UI language is German, the palette's own strings (placeholder, "Tool:", "Go to frame…") are German.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Command source.** Both editors build `MenuDef[]` with items `{ label, shortcut, disabled, run }` (`ui/src/lib/components/layout/menu.ts`; Pixel menus in `ui/src/modes/pixel/menus.ts`, Image menus in `ui/src/modes/image/ImageEditor.svelte`). Flatten them with their path. `helpMenus()` (`ui/src/lib/help/menu.ts`) adds Help items where no native menu exists. Separators and submenus (e.g. Open Recent, Export) need flattening rules.

**English labels.** The i18n dictionary (`ui/src/lib/i18n/en.ts`, `de.ts`, `t()`) can resolve a key in English independent of the current locale; the palette needs the key, not only the translated label, for each item — `MenuItem` carries only `label` today.

**Tools.** Pixel `toolGroups()` in `ui/src/modes/pixel/tools.ts`; Image `toolGroups` in `ImageEditor.svelte`.

**Shortcuts.** `hint(action)` / `toolHint(group)` / `chordLabel` in `ui/src/lib/shell/shortcuts.ts`; add a `commandPalette` `ShortcutAction` with `{ key: 'k', mod: true }`. If the native macOS menu gets an item for it, add it to `NATIVE_MENU_ACTIONS`.

**Recent commands.** Persist in IndexedDB prefs if kept across sessions (`ui/src/lib/idb/prefs.ts`), never localStorage.

**Tests.** e2e: Ctrl+K, type "merge", Enter merges down, Undo works (pattern `ui/e2e/layers.spec.ts`, `ui/e2e/i18n.spec.ts` for the language switch).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Query in different case or as a subsequence of the label ("mrgdwn") | Fuzzy match still finds Merge Down |
| No document open (start screen) | Ctrl+K does nothing (no menu bar to search) |
| Ctrl+K while a text field has focus | The key goes to the field |
| Command that opens a dialog (e.g. Resize Canvas…) | The palette closes, the dialog opens |
| Same label in two menus (Image ▸ Flip Horizontal, Layer ▸ Flip Horizontal) | Two entries, told apart by their menu path |
| Language switched while the palette is closed | Next opening shows the new language |
| ↑ on the first entry / ↓ on the last | Highlight stays (see Assumptions) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| ↑ / ↓ stop at the list ends instead of wrapping | The plan specifies only ↑ / ↓ to move | Low |
| The palette is not available on the start screen | It searches the current editor's menus and tools | Low |
| "Recent" means the last 5 commands run from the palette, kept across reloads | The plan says "recent commands come first" without a count or lifetime | Medium |

## Open questions

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

## Decisions

- 2026-10-29 (migration): Split out of the legacy plan "Power-user keyboard" as its own spec — the palette needs neither the new default shortcuts nor the custom keymap.
- 2026-10-28 (legacy plan): The palette is built from the editors' menu data and tool groups — one command source, so the palette can't list commands the menus don't have.
- 2026-10-28 (legacy plan): Matching runs on the localized label and the English label — English command names then work in every locale.
- 2026-10-28 (legacy plan): Ctrl+K opens the palette — the common convention (Aseprite's Run Command, many editors).
