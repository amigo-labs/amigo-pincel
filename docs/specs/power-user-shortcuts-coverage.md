---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.4, pincel.md §9.2]
---

# Power-user keyboard: default shortcut coverage

## Context

Aseprite users expect dense keyboard control; paint.net, Krita and Photoshop likewise bind nearly every frequent command. Pincel's keymap (`shortcuts.ts`, §9.4) today covers file, undo / redo, selection, zoom, colors, brush size, frame stepping and the tool-group keys — not layers, frames, transforms, opacity or the palette.

This spec gives every frequent action a default shortcut, following Aseprite, Photoshop and paint.net conventions and the §9.4 rule "same key, same meaning in both modes". It is the first of three parts of the legacy plan "Power-user keyboard" (with `power-user-shortcuts-palette` and `power-user-shortcuts-keymap`); the parts don't depend on each other.

## Scope

### In scope

- New default shortcuts for: new / duplicate / delete layer, merge down, select layer above / below, new frame (Pixel), flip horizontal / vertical, rotate 90° CW / CCW (desktop app only), toggle UI, brush opacity steps (Image), next / previous palette color (Pixel)
- Menu hints for the new shortcuts
- A conflict check: no two actions share a chord within a mode
- §9.4 gets the full default table

### Out of scope

- Ctrl+/ (shortcut overview) — because `shortcut-overview` binds it.
- Ctrl+K (command palette) — because `power-user-shortcuts-palette` binds it.
- Ctrl+' / Ctrl+Shift+' (grid / pixel grid) — because `pixel-grid` binds them together with the grids.
- Rebinding — because `power-user-shortcuts-keymap` covers it.
- Multi-key sequences (Emacs-style chords) — because the keymap model is one chord per binding.
- Different bindings for the same action per mode — because §9.4 requires the same key to mean the same thing in both modes.
- Mouse-button and gesture remapping — because this feature covers keyboard shortcuts only.

## Acceptance criteria

- [ ] AC-01 (event): When the user presses Ctrl+J, then the active layer is duplicated, as Layer ▸ Duplicate Layer does — in Pixel and in Image mode.
- [ ] AC-02 (event): When the user presses Ctrl+Shift+E, then the active layer is merged down, as Layer ▸ Merge Down does; Ctrl+E still exports.
- [ ] AC-03 (event): When the user presses Ctrl+Shift+Delete, then the active layer is deleted, as Layer ▸ Delete Layer does.
- [ ] AC-04 (event): When the user presses Ctrl+Shift+N, then a new layer is added, as Layer ▸ New Layer does.
- [ ] AC-05 (event): When the user presses Alt+↑ or Alt+↓, then the layer above or below the active one becomes active; at the top or bottom of the stack nothing changes.
- [ ] AC-06 (event): When the user presses Alt+N in Pixel mode, then a new frame is added, as Frame ▸ New Frame does.
- [ ] AC-07 (event): When the user presses Shift+H or Shift+V, then the flip horizontal or flip vertical command runs (target: see Open questions), and the selected tool does not change.
- [ ] AC-08 (option): Where Pincel runs as the desktop app, Ctrl+R rotates 90° clockwise and Ctrl+Shift+R 90° counter-clockwise; in the browser these keys keep their browser meaning and have no default binding.
- [ ] AC-09 (event): When the user presses Tab while no text field or dialog has focus, then the panels and the tool rail hide; pressing Tab again shows them.
- [ ] AC-10 (event): When the user presses Shift+1 … Shift+9 in Image mode, then the brush opacity becomes 10 % … 90 %; Shift+0 sets 100 %. The bare digits keep their zoom meaning.
- [ ] AC-11 (event): When the user presses Alt+] or Alt+[ in Pixel mode, then the next or previous palette entry becomes the foreground color; `[` / `]` without Alt still resize the brush.
- [ ] AC-12 (always): Each menu item that has one of the new shortcuts shows it as its shortcut hint.
- [ ] AC-13 (always): Within each mode, no two actions or tool-group keys share a chord (checked by an automated test over the whole keymap).
- [ ] AC-14 (always): No default chord is one a browser tab cannot capture (Ctrl+W, Ctrl+T, Ctrl+N, Ctrl+Tab and the others listed in Technical notes); keys that only work in the desktop app are bound only there.
- [ ] AC-15 (state): While a text field has focus, none of the new shortcuts fires; the key reaches the field.
- [ ] AC-16 (error): If the action a new shortcut triggers is unavailable (e.g. merge down on the bottom layer, delete on the only layer, no document), then nothing happens and no error is raised — the same as the disabled menu item.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Keymap.** `ui/src/lib/shell/shortcuts.ts`: `ShortcutAction` union (≈L18), `Chord { key, mod?, shift?, alt? }`, `SHORTCUTS` (≈L44, primary chord plus alternates), `TOOL_GROUP_KEYS` (≈L80), `NATIVE_MENU_ACTIONS` (≈L105), `actionFor(e)` (≈L154), `toolGroupForKey(e)` (≈L162), `isTypingTarget` (≈L172).

**Matching pitfalls.**
- `toolGroupForKey` ignores Shift, so Shift+H and Shift+V would also select the Hand (Image) and Move tool groups; the new chords must win or tool keys must require no Shift.
- `chordMatches` compares `e.key`; Shift+digit produces layout-dependent characters (`!`, `)` on US, `=` for Shift+0 on German), and Alt+[ on macOS produces `“`. These chords need matching on `e.code`.
- `chordMatches` only enforces Shift for letter keys — Shift+Delete vs. Delete (delete selection) needs care.

**Dispatch.** `runShortcut(action)` in `ui/src/modes/pixel/PixelEditor.svelte` (≈L336) and `ui/src/modes/image/ImageEditor.svelte` (≈L455). Unknown actions return `false`, so the browser default still happens.

**Menus.** Pixel `ui/src/modes/pixel/menus.ts`: `transforms()` (≈L83) is used for both Image ▸ (canvas, ≈L171) and Layer ▸ (layer, ≈L191) flips / rotations; Image mode likewise has `flipCanvas` (Image menu) and `transformLayer('flip_h')` (Layer menu) in `ImageEditor.svelte` (≈L632 / L680). Labels `menu.newLayer`, `menu.duplicateLayer`, `menu.deleteLayer`, `menu.mergeDown`, `menu.newFrame`, `menu.flipH`, `menu.flipV`, `menu.rotateCw`, `menu.rotateCcw` exist in `ui/src/lib/i18n/en.ts`. No "duplicate frame" command exists. The "Toggle panels" button (`frame.togglePanels`, `EditorFrame.svelte` ≈L144) hides panels only, not the rail.

**Taken keys.** Alt+←/→ move a layer out of / into a group on the layer grips (`ui/src/lib/components/layers/LayersPanel.svelte` ≈L151). `[` / `]` are brush size, bare digits and `+` / `-` zoom.

**Browser-reserved chords** (cannot be captured in a Chromium tab): Ctrl+N, Ctrl+Shift+N, Ctrl+T, Ctrl+Shift+T, Ctrl+W, Ctrl+Shift+W, Ctrl+Tab, Ctrl+Shift+Tab; Ctrl+R / F5 reload. They work in the Tauri app and partly in an installed PWA window. Playwright's synthetic key events reach the page even for reserved chords, so e2e can't prove AC-14 — the conflict test should check the keymap against this list.

**Native macOS menu.** Built in `src-tauri/src/main.rs` with fixed accelerators; a new action with a native menu item there must be added to `NATIVE_MENU_ACTIONS` (Decision Log 2026-10-27).

**Tests.** e2e sample: Ctrl+J duplicates a layer, Shift+H flips, Tab hides the UI (pattern: `ui/e2e/shortcuts.spec.ts`, `ui/e2e/layers.spec.ts`). The conflict check (AC-13) runs as an e2e test that loads the keymap in the page — Vitest is not set up (AGENTS.md §10).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Merge down on the bottom layer | Nothing happens (AC-16) |
| Delete layer on the only layer | Nothing happens, as the disabled menu item |
| Alt+↑ on the top layer | Active layer unchanged |
| Shift+H in Image mode | Flips; the Hand tool is not selected |
| German keyboard, Shift+0 (`=`) in Image mode | Opacity 100 %, no zoom-in |
| Alt+] with no palette or a one-color palette | No change / stays on the only color |
| Tab while a dialog or the layer-name field has focus | Normal focus navigation / typing, no UI toggle |
| Shortcut pressed on the start screen (no document) | Nothing happens |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Alt+↑ / Alt+↓ select within the visible layer list order, entering groups like the list does | The plan says "Layer up / down (select)" without group rules | Medium |
| Alt+] / Alt+[ stop at the palette's ends instead of wrapping | Not specified in the plan | Low |

## Open questions

- Decision Log entry not yet written: the full default table in §9.4, the browser-reserved keys, keeping Ctrl+E for Export (merge down on Ctrl+Shift+E) and Ctrl+R / Ctrl+Shift+R as desktop-only.
- Ctrl+Shift+N (new layer) is reserved by Chromium for a new incognito window and can't be captured in a browser tab — the same class as Ctrl+N, which the plan rules out. Keep it desktop-only, or pick another key?
- Alt+D (duplicate frame) focuses the address bar in Chromium on Windows / Linux, and no "duplicate frame" command exists yet. Drop duplicate frame from this spec, or add the command and choose a key?
- Shift+H / Shift+V and Ctrl+R / Ctrl+Shift+R: do they flip / rotate the whole image (Image menu) or the active layer (Layer menu)? Both menus have these commands in both modes.
- Tab as "Toggle UI" takes the key keyboard users need for focus navigation (WCAG 2.1.1 / 2.1.2). Is that acceptable when only fired while no focusable control has focus, or should Toggle UI use another key?
- Toggle UI hides "panels and rail" — should the menu bar, options bar and status bar stay, and is the hidden state persisted across reloads?

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

## Decisions

- 2026-10-29 (migration): The legacy plan "Power-user keyboard" (L, three M parts) is split into `power-user-shortcuts-coverage`, `power-user-shortcuts-palette` and `power-user-shortcuts-keymap` — each part stands alone and is testable on its own.
- 2026-10-28 (legacy plan): Ctrl+E stays Export; merge down takes Ctrl+Shift+E — Export is an established binding.
- 2026-10-28 (legacy plan): Rotate 90° gets Ctrl+R / Ctrl+Shift+R only in the desktop app — Ctrl+R reloads a browser tab.
- 2026-10-28 (legacy plan): Brush opacity uses Shift+digits and palette stepping uses Alt+[ / Alt+] — bare digits are zoom keys and bare brackets are brush size.
- 2026-10-28 (legacy plan): The conflict check runs as an e2e test over the keymap in the page — the plan's no-new-dependency alternative to setting up Vitest, which would need approval.
