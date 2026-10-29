---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.4, pincel.md §11.3, pincel.md §9.5]
depends_on: [shortcut-overview]
---

# Power-user keyboard: custom keymap

## Context

Aseprite, paint.net, Krita and Photoshop all let users rebind shortcuts; Pincel's keymap (`shortcuts.ts`, §9.4) is fixed. Users coming from another editor want their muscle memory back.

Any shortcut action or tool-group key can be rebound in the Keyboard Shortcuts dialog (`shortcut-overview`). Conflicts are shown and resolved on the spot, rows and the whole map can be reset, and the keymap can be exported and imported as JSON. User overrides sit on top of the defaults and are stored platform-neutrally, so one keymap works on every OS. This is one of three parts of the legacy plan "Power-user keyboard" (with `power-user-shortcuts-coverage` and `power-user-shortcuts-palette`).

## Scope

### In scope

- Change, Reset per row and Reset all in the Keyboard Shortcuts dialog
- Conflict warning with Replace / Cancel
- Overrides persisted in IndexedDB prefs; menus, tooltips, the overview and key handling use the effective map
- Export and import of the overrides as a JSON file
- Keeping the native macOS menu's accelerators consistent with rebound actions
- en + de strings

### Out of scope

- Multi-key sequences (Emacs-style chords) — because a binding is one chord.
- Different bindings for the same action per mode — because §9.4 requires the same key to mean the same thing in both modes.
- Mouse-button and gesture remapping — because this feature covers keyboard shortcuts only.
- Fixed gestures (Space-drag, two-finger pan / zoom, Shift / Alt selection modifiers) — because they are pointer modifiers, not shortcut actions.

## Acceptance criteria

- [ ] AC-01 (event): When the user clicks Change on a row of the Keyboard Shortcuts dialog and presses a new chord, then that chord triggers the row's action and the previous chord no longer does (rebinding Undo to Ctrl+U: Ctrl+U undoes, Ctrl+Z does nothing).
- [ ] AC-02 (always): After a rebinding, the menu hint, tooltips and the overview row of that action show the new chord (Edit ▸ Undo shows `Ctrl+U`).
- [ ] AC-03 (event): When the user rebinds a tool-group key (e.g. Lasso from `Q` to `Y`), then the new key selects and cycles that tool group and the old key no longer does.
- [ ] AC-04 (event): When the pressed chord is already bound to another action or tool group, then the dialog warns, naming that action, and offers Replace and Cancel; Replace gives the chord to the edited row and leaves the other action without that chord, Cancel changes nothing.
- [ ] AC-05 (event): When the user clicks Reset on a row, then that row's default chords return; when the user clicks Reset all, then every action and tool group has its default chords again.
- [ ] AC-06 (event): When the user reloads the app after rebinding, then the rebinding is still in effect, in both modes.
- [ ] AC-07 (event): When the user clicks Export, then a JSON file is offered for download that contains the user's overrides.
- [ ] AC-08 (event): When the user imports a JSON file exported by Pincel, then its overrides become the effective keymap.
- [ ] AC-09 (error): If an imported file is not valid JSON or does not match the keymap schema, then the import is rejected with a message and the current keymap stays unchanged.
- [ ] AC-10 (error): If an imported file names actions or tool groups Pincel doesn't know, then those entries are ignored, the rest is applied, and a notice says how many entries were ignored.
- [ ] AC-11 (always): A stored chord records "the platform's command modifier", not Ctrl or ⌘: a keymap exported on Windows with Ctrl+U shows and works as `⌘U` on macOS.
- [ ] AC-12 (state): While the dialog waits for a new chord, the pressed keys don't trigger editor actions; pressing only a modifier key (Shift, Ctrl, Alt, ⌘) does not complete the capture.
- [ ] AC-13 (event): When the user presses Escape while the dialog waits for a new chord, then the capture is cancelled and the binding is unchanged.
- [ ] AC-14 (event, manual: the native macOS menu of the desktop app can't be driven by Playwright): When the user rebinds an action that the native macOS menu owns (open, save, export, undo, redo, …), then the new chord runs it exactly once, the old chord no longer runs it, and the native menu shows the new chord.
- [ ] AC-15 (option): Where the UI language is German, Change, Reset, Reset all, Export, Import, the conflict warning and the import messages are German.
- [ ] AC-16 (error): If an imported file binds the same chord to two of its entries, then the import is rejected with a message naming that chord, and the current keymap stays unchanged.
- [ ] AC-17 (event): When an imported file binds a chord that a remaining default still uses, then the import wins as with Replace — the default loses that chord — and a notice says how many defaults were replaced.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Keymap.** `ui/src/lib/shell/shortcuts.ts`: `ShortcutAction` (≈L18), `Chord { key, mod?, shift?, alt? }` (already platform-neutral via `mod`), `SHORTCUTS` (≈L44), `TOOL_GROUP_KEYS` (≈L80), `NATIVE_MENU_ACTIONS` (≈L105), `chordLabel` / `hint` / `toolHint` (≈L124–139), `actionFor` (≈L154), `toolGroupForKey` (≈L162). All read the static maps today.

**Model (plan design).** ``Keymap = Partial<Record<ShortcutAction | `tool:${string}`, Chord[]>>`` of user overrides on top of the defaults; an empty array means unbound. Stored in the IndexedDB pref `keymap` (`ui/src/lib/idb/prefs.ts`). `actionFor` / `hint` / `toolGroupForKey` read the effective map (defaults merged with overrides) from a reactive store, e.g. `ui/src/lib/shell/keymap.svelte.ts`. Menus re-render their hints from it.

**Editor.** Lives in the Keyboard Shortcuts dialog of `shortcut-overview` (`ui/src/lib/components/ShortcutsDialog.svelte`). File download / open goes through `ui/src/lib/fs` (FSA / download / Tauri).

**Native macOS menu.** Built in Rust (`src-tauri/src/main.rs`) with fixed accelerators for `NATIVE_MENU_ACTIONS`; the editors leave those chords to it where `hasNativeMenu()` holds (Decision Log 2026-10-27). A new Tauri command `set_menu_accelerators(map)` pushes the effective chords of rebound native-menu actions and rebuilds the macOS menu, as `set_menu_language` already does; `NATIVE_MENU_ACTIONS` ownership is unchanged, and the command is a no-op off macOS.

**Tests.** e2e: rebind Undo to Ctrl+U, check it works and the menu hint shows `Ctrl+U`, reload and it persists, Reset all (patterns `ui/e2e/shortcuts.spec.ts`, `ui/e2e/recents.spec.ts` for reload persistence, `ui/e2e/export.spec.ts` for downloads).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Chord assigned that a browser tab can't capture (e.g. Ctrl+W) | The capture never receives it in a tab; the binding is unchanged |
| Action left without any chord after Replace | It shows no hint and works from its menu only; Reset restores it |
| Import of a keymap with a conflict inside the file | Rejected with a message naming the chord; keymap unchanged (AC-16) |
| Import of a keymap whose chord a remaining default uses | Import wins; the default loses the chord; the notice counts replaced defaults (AC-17) |
| Rebinding a Pixel-only action (e.g. next frame) to a key Image mode uses for a tool | Reported as a conflict (one keymap for both modes) |
| Empty overrides exported | A valid JSON file with no overrides; importing it equals Reset all |
| Same chord pressed as the current one | No change, no warning |
| Pref store unavailable (IndexedDB blocked) | Rebinding works for the session; it is not kept after reload |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Conflicts are checked across both modes' actions and tool keys | One shared keymap and §9.4's "same key, same meaning" | Medium |
| The JSON file holds only overrides, with a schema version field | The plan stores overrides and asks to "validate the schema" without defining it | Medium |
| Change sets the primary chord and drops alternates (e.g. Redo's Ctrl+Y) | The plan's editor has one "press the new chord" step per row | Medium |

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

## Decisions

- 2026-10-29 (migration): Split out of the legacy plan "Power-user keyboard" as its own spec; it builds on the Keyboard Shortcuts dialog of `shortcut-overview`.
- 2026-10-28 (legacy plan): The keymap stores user overrides on top of the defaults, not a full copy — new default shortcuts reach users who never rebound them.
- 2026-10-28 (legacy plan): Stored chords use the platform-neutral `mod` modifier — one keymap works on all OSes.
- 2026-10-28 (legacy plan): Unknown actions in an import are ignored with a notice — keymaps from newer or older versions still import.
- 2026-10-29: Rebound native-menu actions are pushed to the macOS menu by a new Tauri command `set_menu_accelerators(map)` that rebuilds the menu as `set_menu_language` does, so the native menu shows the new chord (AC-14) — the rebuild path exists already, and the menu keeps showing correct hints (owner accepted the recommendation).
- 2026-10-29: An import whose file binds one chord twice is rejected naming the chord (AC-16); a chord a remaining default uses is taken by the import as with Replace, with a notice counting replaced defaults (AC-17) — Pincel's own exports can't contain internal conflicts, and an imported keymap should win over defaults (owner accepted the recommendation).
