---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.4, pincel.md §9.5, pincel.md §11.3]
---

# Help ▸ Keyboard Shortcuts overview

## Context

Shortcuts appear today only as menu hints and tooltips; there is no overview. New users can't discover the tool keys such as M, Q, W or U, which cycle variants when pressed again.

**Help ▸ Keyboard Shortcuts** and **Ctrl+/** (⌘/ on macOS) open a searchable dialog that lists every shortcut of the current mode, grouped and printed in the platform's notation. The list is generated from the single keymap (`shortcuts.ts`, §9.4), so it can't drift from what the keys actually do. `power-user-shortcuts-keymap` later makes this dialog editable, so this feature comes first.

## Scope

### In scope

- A Keyboard Shortcuts dialog with a filter field, rows grouped into sections, chords in platform notation, printable
- Rows for the shortcut actions and tool-group keys the current mode binds, plus the pointer gestures it supports
- Entry points: Help ▸ Keyboard Shortcuts in the in-app Help menu and in the native macOS Help menu; the new shortcut Ctrl+/
- en + de strings
- §9.4 and §11.3 of the design spec mention the overview

### Out of scope

- Editing or rebinding shortcuts — because `power-user-shortcuts-keymap` builds the keymap editor on top of this dialog.
- A command palette — because `power-user-shortcuts-palette` covers it.
- `?` as an extra key for the overview — because it is typed with Shift on many layouts and conflicts with typing (see Decisions).
- The overview on the start screen — because there is no current mode and so no keymap to list (see Assumptions).

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses Help ▸ Keyboard Shortcuts from the in-app menu bar, then a dialog named "Keyboard Shortcuts" opens, in Pixel and in Image mode.
- [ ] AC-02 (event): When the user presses Ctrl+/ while no text field has focus, then the Keyboard Shortcuts dialog opens, in Pixel and in Image mode.
- [ ] AC-03 (event, manual: the native macOS menu of the desktop app can't be driven by Playwright): When the user chooses Help ▸ Keyboard Shortcuts in the desktop app's native macOS menu, then the Keyboard Shortcuts dialog opens.
- [ ] AC-04 (always): The dialog lists one row per shortcut action and per tool-group key that the current mode responds to, each with a label and its chord(s): for example "Undo" with `Ctrl+Z`, and the Lasso group with `Q`.
- [ ] AC-05 (always): No row of the dialog shows an empty label or an untranslated i18n key.
- [ ] AC-06 (always): The chord a row shows for an action is the same text as the shortcut hint the menu shows for that action (e.g. Edit ▸ Undo).
- [ ] AC-07 (always): Rows are grouped into the sections File, Edit, Select, View, Tools and — in Pixel mode only — Frames; a section without rows is not shown.
- [ ] AC-08 (state): While the document is in Image mode, the dialog lists no frame shortcuts (`,`, `.`, Enter) and no Frames section; in Pixel mode it lists them.
- [ ] AC-09 (always): A tool-group row whose group has more than one tool carries a note that pressing the key again cycles the variants; a single-tool group row has no such note.
- [ ] AC-10 (always): The dialog lists the pointer gestures the current mode supports: Space-drag pans, two-finger drag / pinch pans and zooms, Shift / Alt modify a selection.
- [ ] AC-11 (option): Where the platform is macOS (Mac user agent), chords are shown in macOS notation (e.g. `⌘Z`, `⇧⌘S`); everywhere else as `Ctrl+Z`, `Ctrl+Shift+S`.
- [ ] AC-12 (event): When the dialog opens, then its filter field has keyboard focus.
- [ ] AC-13 (event): When the user types into the filter field, then only rows whose label contains the typed text (case-insensitive) remain and sections left without rows are hidden; typing "lasso" leaves only the Lasso row with key `Q`.
- [ ] AC-14 (state): While the filter field has focus, typing letters does not trigger tool keys or other editor shortcuts (typing "b" does not switch the tool).
- [ ] AC-15 (event): When the user presses Escape while the dialog is open, then the dialog closes.
- [ ] AC-16 (always): The dialog is exposed as `role="dialog"` with an accessible name.
- [ ] AC-17 (option): Where the page is rendered for print, the shortcut list is printed in full (not cut off by the dialog's scroll area) and the editor behind the dialog is not printed.
- [ ] AC-18 (option): Where the UI language is German, the dialog title, section names, filter placeholder, gesture rows and the cycle note are German.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Keymap.** `ui/src/lib/shell/shortcuts.ts` (§9.4):
- `ShortcutAction` (≈L18) and `SHORTCUTS: Record<ShortcutAction, Chord[]>` (≈L44): primary chord plus alternates (alternates are matched but not displayed).
- `TOOL_GROUP_KEYS` (≈L80): group id → key.
- `chordLabel(c)` (≈L124), `hint(action)` (≈L133) and `toolHint(group)` (≈L139) format chords; `IS_MAC` comes from `isMac()` in `ui/src/lib/platform/index.ts` (≈L32, user-agent sniff — Playwright can set a Mac `userAgent` for AC-11). Reuse this formatting so AC-06 holds by construction.
- `NATIVE_MENU_ACTIONS` (≈L105): if the native Help item gets a `⌘/` accelerator, add the new `shortcuts` action there so it doesn't fire twice on macOS.
- `chordMatches` only enforces Shift for letter keys, so Ctrl+/ also matches on layouts where `/` is typed with Shift (German: Ctrl+Shift+7).

**Data.** Plan design: a `shortcutList(mode)` builder next to `shortcuts.ts` producing rows `{ group, label, chords[] }` from:
- `SHORTCUTS`, mapped to `menu.*` label keys through a small `ACTION_LABEL: Record<ShortcutAction, I18nKey>` map — a `Record` makes `pnpm check` fail when an action has no label. Actions without a menu label today (e.g. `brushSmaller`, `brushLarger`) need new keys in `en.ts` and `de.ts`.
- the mode's tool groups (Pixel `toolGroups()` in `ui/src/modes/pixel/tools.ts`; Image `toolGroups` in `ui/src/modes/image/ImageEditor.svelte`) plus `TOOL_GROUP_KEYS`;
- the fixed gestures.

Rows list only what the mode binds. Each editor's `runShortcut` switch (`PixelEditor.svelte` ≈L336, `ImageEditor.svelte` ≈L455) is the truth; export the supported-action list per mode. Image mode binds no `prevFrame` / `nextFrame` / `togglePlay` / `exportSheet` today. `paste` is a hint-only action: the editors leave Ctrl+V to the browser's `paste` event, so list it where the mode handles paste.

**Dialog.** Pattern: `ui/src/lib/components/AboutDialog.svelte` with `help.about` in `ui/src/lib/help/state.svelte.ts`. New `ShortcutsDialog.svelte` in `ui/src/lib/components/`, `<kbd>` styled with `pc-*` tokens, a simple `@media print` style.

**Entry points.**
- `helpMenus()` in `ui/src/lib/help/menu.ts` (≈L13) returns nothing where `hasNativeMenu()` holds (macOS desktop); add the item there.
- Native macOS Help menu: built in `src-tauri/src/main.rs`; a new menu id that emits an event the shell opens the dialog on (commit separately as `feat(tauri)`).

**Tests.** e2e `ui/e2e/shortcuts-overview.spec.ts` (Ctrl+/ in both modes, Undo with `Ctrl+Z`, filter "lasso", frame keys Pixel-only). Existing `ui/e2e/help.spec.ts` shows how About is opened and tested; `ui/e2e/i18n.spec.ts` how the language is switched.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Desktop app on macOS (native menu owns Help) | No in-app Help menu; the native Help item and ⌘/ open the dialog |
| Ctrl+/ pressed while a text field (e.g. layer rename) has focus | The dialog does not open; the key goes to the field |
| Filter text matches no row | No rows and no sections are shown; the filter keeps its text |
| Filter text with different case ("LASSO") | Matches the same rows as "lasso" |
| Action with alternate chords (Redo: Ctrl+Shift+Z and Ctrl+Y) | The row shows all its chords, the primary one (equal to the menu hint) first |
| Tool group with a single tool (e.g. Text) | Row without the cycle note |
| UI language switched while the dialog is closed | The next opening shows the new language |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Actions and gestures without an obvious menu go into the section of the related menu: swap / reset colors and brush size → Tools, pan / zoom gestures → View, selection modifiers → Select | The plan names the sections but not where these rows go | Low — only grouping |
| The overview is only reachable inside an editor, not on the start screen | The plan lists "every shortcut of the current mode"; the start screen has no mode | Low |
| The filter matches labels only, not chord text | The plan's example filters by label ("lasso") | Low |

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
| AC-18 |      | open   |        |

## Decisions

- 2026-10-28 (legacy plan): The list is generated from the keymap and the editors' supported actions, never hand-written — so it can't drift from the real bindings.
- 2026-10-28 (legacy plan): Ctrl+/ opens the overview; `?` is not bound — it conflicts with typing on some keyboard layouts.
- 2026-10-28 (legacy plan): The overview lists only the actions the current mode binds — same key, same meaning (§9.4), and a mode without an action shouldn't advertise it.
- 2026-10-29 (migration): The gesture rows list only gestures that exist. The plan's "Alt + tool for a temporary eyedropper (Pixel)" is not implemented (Alt only modifies selections in `ui/src/modes/pixel/pointer.ts`), so it is not listed.
- 2026-10-29 (migration): Size raised from the plan's S to M — the change spans the keymap, a new dialog, the Help menu, the native macOS menu, both editors, i18n and e2e.
