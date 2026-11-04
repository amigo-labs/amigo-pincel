---
status: implemented
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

- [x] AC-01 (event): When the user presses Ctrl+K while no text field has focus, then the command palette opens with its search field focused — in Pixel and in Image mode.
- [x] AC-02 (always): The palette lists every command of the current mode's menu bar, labelled with its menu path (e.g. "Layer ▸ Merge Down"), and every tool of the current mode (e.g. "Tool: Magic Wand").
- [x] AC-03 (state): While in Pixel mode, the palette also offers "Go to frame…"; in Image mode it does not.
- [x] AC-04 (event): When the user types a query, then only entries whose label fuzzy-matches the query remain, best matches first; typing "merge" lists "Layer ▸ Merge Down".
- [x] AC-05 (option): Where the UI language is not English, a query in English also finds the command (with German UI, "merge" finds "Ebene ▸ Nach unten vereinen").
- [x] AC-06 (always): Each entry that has a shortcut shows it, in the same text as its menu hint.
- [x] AC-07 (event): When the user moves with ↑ / ↓ and presses Enter, then the highlighted command runs and the palette closes; with "merge" + Enter on a two-layer document the layers are merged down, and one Undo restores both layers.
- [x] AC-08 (event): When the user clicks an enabled entry, then that command runs and the palette closes.
- [x] AC-09 (state): While a command is disabled in its menu, its palette entry is shown dimmed and Enter or a click on it does nothing and keeps the palette open.
- [x] AC-10 (event): When the user picks a tool entry, then that tool becomes active, as clicking it in the tool rail does.
- [x] AC-11 (event): When the user runs "Go to frame…" and enters a frame number within the sprite's frames, then that frame becomes the current frame.
- [x] AC-12 (error): If the user enters a frame number outside 1 … frame count or not a number in "Go to frame…", then the current frame does not change.
- [x] AC-13 (event): When the user opens the palette after running commands from it, then the most recently run commands are listed first while the query is empty.
- [x] AC-14 (event): When the user presses Escape, then the palette closes without running anything.
- [x] AC-15 (always): The palette is a `role="dialog"` with a combobox search field controlling a listbox; the highlighted entry is announced as the active option.
- [x] AC-16 (state): While the search matches nothing, the list is empty and Enter runs nothing.
- [x] AC-17 (option): Where the UI language is German, the palette's own strings (placeholder, "Tool:", "Go to frame…") are German.

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
| AC-01 | `ui/e2e/command-palette.spec.ts` "Ctrl+K opens the palette with every command and tool; Escape closes it" (pixel, image); `ui/e2e/command-palette.spec.ts` "a text field keeps Ctrl+K" (pixel, image) | verified | b445733 |
| AC-02 | `ui/e2e/command-palette.spec.ts` "Ctrl+K opens the palette with every command and tool; Escape closes it" (pixel, image); `ui/e2e/command-palette.spec.ts` "the palette lists exactly the menu bar's commands plus the tools" (pixel, image; 2e3a91c) | verified | b445733 |
| AC-03 | `ui/e2e/command-palette.spec.ts` "Ctrl+K opens the palette with every command and tool; Escape closes it" (pixel, image); `ui/e2e/command-palette.spec.ts` "the palette speaks German and finds English queries" | verified | b445733 |
| AC-04 | `ui/e2e/command-palette.spec.ts` "typing filters fuzzily, Enter runs the highlighted command, one Undo restores" | verified | b445733 |
| AC-05 | `ui/e2e/command-palette.spec.ts` "the palette speaks German and finds English queries" | verified | b445733 |
| AC-06 | `ui/e2e/command-palette.spec.ts` "typing filters fuzzily, Enter runs the highlighted command, one Undo restores" | verified | b445733 |
| AC-07 | `ui/e2e/command-palette.spec.ts` "typing filters fuzzily, Enter runs the highlighted command, one Undo restores" | verified | b445733 |
| AC-08 | `ui/e2e/command-palette.spec.ts` "a click runs the command, a tool entry selects the tool" | verified | b445733 |
| AC-09 | `ui/e2e/command-palette.spec.ts` "a disabled command is dimmed and does nothing; no match runs nothing" | verified | b445733 |
| AC-10 | `ui/e2e/command-palette.spec.ts` "a click runs the command, a tool entry selects the tool" | verified | b445733 |
| AC-11 | `ui/e2e/command-palette.spec.ts` "Go to frame… jumps to a frame in range and ignores anything else" | verified | b445733 |
| AC-12 | `ui/e2e/command-palette.spec.ts` "Go to frame… jumps to a frame in range and ignores anything else" | verified | b445733 |
| AC-13 | `ui/e2e/command-palette.spec.ts` "recently run commands come first, also after a reload" | verified | b445733 |
| AC-14 | `ui/e2e/command-palette.spec.ts` "Ctrl+K opens the palette with every command and tool; Escape closes it" (pixel, image) | verified | b445733 |
| AC-15 | `ui/e2e/command-palette.spec.ts` "typing filters fuzzily, Enter runs the highlighted command, one Undo restores" | verified | b445733 |
| AC-16 | `ui/e2e/command-palette.spec.ts` "a disabled command is dimmed and does nothing; no match runs nothing" | verified | b445733 |
| AC-17 | `ui/e2e/command-palette.spec.ts` "the palette speaks German and finds English queries" | verified | b445733 |

## Summary

> Completed: 2026-11-04

- Ctrl+K opens a command palette (`CommandPalette.svelte`, `lib/shell/command-palette.ts`) over every menu-bar command, labelled with its menu path and shortcut, plus every tool and, in Pixel mode, "Go to frame…".
- Fuzzy matching in the UI language and in English; disabled commands are dimmed and don't run.
- The last commands run from the palette come first and persist in IndexedDB.
- Accessible combobox / listbox dialog, German strings through `t()`.

## Decisions

- 2026-10-29 (migration): Split out of the legacy plan "Power-user keyboard" as its own spec — the palette needs neither the new default shortcuts nor the custom keymap.
- 2026-10-28 (legacy plan): The palette is built from the editors' menu data and tool groups — one command source, so the palette can't list commands the menus don't have.
- 2026-10-28 (legacy plan): Matching runs on the localized label and the English label — English command names then work in every locale.
- 2026-10-28 (legacy plan): Ctrl+K opens the palette — the common convention (Aseprite's Run Command, many editors).
- 2026-11-04 (build): English labels come from a reverse lookup of the active locale's dictionary (`englishLabel` in `ui/src/lib/i18n/`) instead of a message key on every `MenuItem` — one place, no change to the menu data of both editors; labels that aren't plain messages (file names in Open Recent, "Pause" / "Play" filled in at runtime) are matched in the UI language only.
- 2026-11-04 (build): An entry's id (for the recent list) is its English menu path — recents survive a language switch.
- 2026-11-04 (build): Fuzzy matching is an in-order subsequence over the menu path with bonuses for adjacent characters, word starts, a plain substring and a command name that starts with the query — "merge" then ranks Merge Down above Copy Merged.
- 2026-11-04 (build): A tool entry shows its group's key, for every tool of the group — the key reaches each of them by cycling.
- 2026-11-04 (build): An invalid frame number in "Go to frame…" keeps the field open and marks it `aria-invalid` — the user can correct it, and Escape closes without a change (AC-12).
- 2026-11-04 (build): Ctrl+K has no native macOS menu item — the spec doesn't ask for one; the editors handle the key on every platform.
