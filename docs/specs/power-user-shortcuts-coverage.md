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

- New default shortcuts for: new / duplicate / delete layer, merge down, select layer above / below, new frame (Pixel), flip horizontal / vertical (active layer), rotate 90° CW / CCW (whole image, desktop app only), toggle UI, brush opacity steps (Image), next / previous palette color (Pixel)
- A View ▸ Toggle UI menu item
- Moving Export Sheet from Ctrl+Shift+E to Ctrl+Alt+E, so merge down can take Ctrl+Shift+E (menus, native macOS menu, shortcut hints)
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
- Duplicate frame (and a key for it) — because no "duplicate frame" command exists yet and Alt+D is reserved in Chromium; command and key come with a timeline spec.

## Acceptance criteria

- [ ] AC-01 (event): When the user presses Ctrl+J, then the active layer is duplicated, as Layer ▸ Duplicate Layer does — in Pixel and in Image mode.
- [ ] AC-02 (event): When the user presses Ctrl+Shift+E, then the active layer is merged down, as Layer ▸ Merge Down does, and no sprite sheet export starts; Ctrl+E still exports.
- [ ] AC-03 (event): When the user presses Ctrl+Shift+Delete, then the active layer is deleted, as Layer ▸ Delete Layer does.
- [ ] AC-04 (event): When the user presses Shift+N, then a new layer is added, as Layer ▸ New Layer does — in the browser and in the desktop app — and the Gradient tool group (N) is not selected.
- [ ] AC-05 (event): When the user presses Alt+↑ or Alt+↓, then the layer above or below the active one becomes active; at the top or bottom of the stack nothing changes.
- [ ] AC-06 (event): When the user presses Alt+N in Pixel mode, then a new frame is added, as Frame ▸ New Frame does.
- [ ] AC-07 (event): When the user presses Shift+H or Shift+V, then the active layer is flipped horizontally or vertically, as Layer ▸ Flip Horizontal / Flip Vertical does, and the selected tool does not change.
- [ ] AC-08 (option): Where Pincel runs as the desktop app, Ctrl+R rotates the whole image 90° clockwise and Ctrl+Shift+R 90° counter-clockwise, as Image ▸ Rotate does; in the browser these keys keep their browser meaning and have no default binding.
- [ ] AC-09 (event): When the user presses Tab while the canvas stage has focus, then the tool rail, the options bar and the panels hide while the menu bar and the status bar stay; pressing Tab again shows them. While focus is on the page body or any other control, Tab moves focus as usual.
- [ ] AC-10 (event): When the user presses Shift+1 … Shift+9 in Image mode, then the brush opacity becomes 10 % … 90 %; Shift+0 sets 100 %. The bare digits keep their zoom meaning.
- [ ] AC-11 (event): When the user presses Alt+] or Alt+[ in Pixel mode, then the next or previous palette entry becomes the foreground color; `[` / `]` without Alt still resize the brush.
- [ ] AC-12 (always): Each menu item that has one of the new shortcuts shows it as its shortcut hint.
- [ ] AC-13 (always): Within each mode, no two actions or tool-group keys share a chord (checked by an automated test over the whole keymap).
- [ ] AC-14 (always): No default chord is one a browser tab cannot capture (Ctrl+W, Ctrl+T, Ctrl+N, Ctrl+Tab and the others listed in Technical notes); keys that only work in the desktop app are bound only there.
- [ ] AC-15 (state): While a text field has focus, none of the new shortcuts fires; the key reaches the field.
- [ ] AC-16 (error): If the action a new shortcut triggers is unavailable (e.g. merge down on the bottom layer, delete on the only layer, no document), then nothing happens and no error is raised — the same as the disabled menu item.
- [ ] AC-17 (event): When the user chooses View ▸ Toggle UI, then the UI toggles exactly as Tab on the canvas stage does (AC-09).
- [ ] AC-18 (state): While the UI is hidden, a reload shows the full UI — the hidden state is not persisted.
- [ ] AC-19 (event): When the user presses Ctrl+Alt+E (⌥⌘E on macOS) in Pixel mode, then the sprite sheet export dialog opens, as File ▸ Export ▸ Sprite Sheet does; the menu item, the native macOS menu item and the shortcut overview show Ctrl+Alt+E, and Ctrl+Shift+E no longer opens it.

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Keymap.** `ui/src/lib/shell/shortcuts.ts`: `ShortcutAction` union (≈L18), `Chord { key, mod?, shift?, alt? }`, `SHORTCUTS` (≈L44, primary chord plus alternates), `TOOL_GROUP_KEYS` (≈L80), `NATIVE_MENU_ACTIONS` (≈L105), `actionFor(e)` (≈L154), `toolGroupForKey(e)` (≈L162), `isTypingTarget` (≈L172).

**Matching pitfalls.**
- `toolGroupForKey` ignores Shift, so Shift+H and Shift+V would also select the Hand (Image) and Move tool groups; the new chords must win or tool keys must require no Shift.
- `chordMatches` compares `e.key`; Shift+digit produces layout-dependent characters (`!`, `)` on US, `=` for Shift+0 on German), and Alt+[ on macOS produces `“`. These chords need matching on `e.code`.
- `chordMatches` only enforces Shift for letter keys — Shift+Delete vs. Delete (delete selection) needs care.
- Action chords must be matched before tool-group keys, so Shift+N (new layer) doesn't select the Gradient group (N) and Shift+H / Shift+V don't select Hand / Move.
- Ctrl+Alt+E (Export Sheet): on Windows, Ctrl+Alt is AltGr, and on layouts where AltGr+E types a character (German `€`) `e.key` is that character — match this chord on `e.code` (`KeyE`), as with the Shift+digit chords.

**Export Sheet move.** `exportSheet` in `SHORTCUTS` becomes `{ key: 'e', mod: true, alt: true }`; the macOS native menu item `menu:exportSheet` in `src-tauri/src/main.rs` (≈L301) changes its accelerator from `CmdOrCtrl+Shift+E` to `CmdOrCtrl+Alt+E`. Menus and the shortcut overview print hints from `SHORTCUTS`, so they follow. `docs/specs/desktop-app.md` lists the old accelerator and needs the same update.

**Toggle UI focus.** The canvas stage gets `tabindex="-1"` and takes focus on pointerdown, so "Tab on the canvas stage" (AC-09) is observable and Tab elsewhere stays focus navigation (WCAG 2.1.1 / 2.1.2).

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
| Tab on the page body or a toolbar button (canvas stage not focused) | Focus moves to the next control, no UI toggle |
| UI hidden, user reloads the page | Full UI is shown (AC-18) |
| Shift+N in either mode | New layer; the Gradient tool group is not selected |
| Ctrl+Shift+E in Pixel mode | Merge down only; the sprite sheet dialog does not open |
| German keyboard on Windows, Ctrl+Alt+E (reported as AltGr, `e.key` = `€`) on the canvas | Sprite sheet dialog opens (matched on `e.code`) |
| Ctrl+Alt+E in Image mode | Nothing happens (no sprite sheet export in Image mode) |
| Shortcut pressed on the start screen (no document) | Nothing happens |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Alt+↑ / Alt+↓ select within the visible layer list order, entering groups like the list does | The plan says "Layer up / down (select)" without group rules | Medium |
| Alt+] / Alt+[ stop at the palette's ends instead of wrapping | Not specified in the plan | Low |

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
| AC-19 |      | open   |        |

## Decisions

- 2026-10-29 (migration): The legacy plan "Power-user keyboard" (L, three M parts) is split into `power-user-shortcuts-coverage`, `power-user-shortcuts-palette` and `power-user-shortcuts-keymap` — each part stands alone and is testable on its own.
- 2026-10-28 (legacy plan): Ctrl+E stays Export; merge down takes Ctrl+Shift+E — Export is an established binding.
- 2026-10-28 (legacy plan): Rotate 90° gets Ctrl+R / Ctrl+Shift+R only in the desktop app — Ctrl+R reloads a browser tab.
- 2026-10-28 (legacy plan): Brush opacity uses Shift+digits and palette stepping uses Alt+[ / Alt+] — bare digits are zoom keys and bare brackets are brush size.
- 2026-10-28 (legacy plan): The conflict check runs as an e2e test over the keymap in the page — the plan's no-new-dependency alternative to setting up Vitest, which would need approval.
- 2026-10-29: Merge Down takes Ctrl+Shift+E (AC-02) and Export Sheet moves from Ctrl+Shift+E to Ctrl+Alt+E (AC-19), in the web keymap and the native macOS menu; the default keymap goes into the §9.4 table, browser-reserved chords never get a default, Ctrl+R / Ctrl+Shift+R are desktop-only (§15 row recorded with this batch) — Ctrl+Alt+E is free in every spec and in `shortcuts.ts`, isn't browser-reserved and keeps Export Sheet next to Export (Ctrl+E) (owner decision).
- 2026-10-29: New layer uses Shift+N (Aseprite's key) in both builds, and action chords are matched before tool-group keys — Ctrl+Shift+N can't be captured in a browser tab, Shift+N can (owner accepted the recommendation).
- 2026-10-29: Duplicate frame is dropped from this spec — the command doesn't exist and Alt+D is reserved in Chromium; command and key come with a timeline spec (owner accepted the recommendation).
- 2026-10-29: Shift+H / Shift+V flip the active layer (Layer menu); Ctrl+R / Ctrl+Shift+R rotate the whole image (Image menu) — flipping is a per-layer drawing aid as in Aseprite, while a 90° layer rotation on a non-square canvas clips (owner accepted the recommendation).
- 2026-10-29: Tab toggles the UI only while the canvas stage has focus, and View ▸ Toggle UI does the same — painters get the Photoshop / Krita key and keyboard users keep Tab navigation (WCAG 2.1.1 / 2.1.2) (owner accepted the recommendation).
- 2026-10-29: Toggle UI hides the tool rail, options bar and panels, keeps the menu bar and status bar, and is not persisted — the way back stays visible and nobody is stuck in a bare UI after a reload (owner accepted the recommendation).
