---
status: todo
created: 2026-11-26
size: L
mode: both
design: [pincel.md §9.2, pincel.md §9.4]
---

# One dialog shell

## Context

Part of the UI refresh (owner request 2026-11-25; backlog "UI refresh: dialogs").

Pincel has about thirty modal dialogs. Almost all of them draw their panel with the shared `am-dialog` classes, but each one builds its own backdrop and keyboard handling. So they behave differently in ways a user notices:

- Only the effect panel has a close button (×) in its header; the others close only through Cancel or Escape.
- Some dialogs listen for Escape on the dialog, some on the window.
- Enter applies through a form in some, through a handler in others, and does nothing in a few.
- Focus goes to the first field in some and nowhere in others, and almost none returns it on close.
- Tab leaves most dialogs and walks into the editor behind them.
- One dialog's main button says "OK" where every other one names what it does.

Image mode already has a shared shell for five of its dialogs (`Modal.svelte`). This spec makes it the one shell for the form dialogs of both modes and the shell's own dialogs (the *dialogs in scope* below), and fixes how they behave.

Terms:

- **Dialog**: a modal window over the app with a title, content and buttons.
- **Primary button**: the dialog's main action, drawn as the primary button (`am-btn--primary`).
- **Field**: a text, number or range input, a select, or a checkbox or radio inside the dialog's content.
- **Dialogs in scope**:
  - Image mode: Print, Resize Image, Resize Canvas (Scale), Save As, Export.
  - Pixel mode: Export PNG, Grid Settings, Resize / Scale Sprite, Sprite Sheet export, Animation export.
  - Both modes: Rename layer, the selection Radius dialogs (Grow / Shrink / Feather / Border), SVG export.

## Scope

### In scope

- A shared dialog shell used by every dialog in scope
- The close control in the header, Escape, Enter, Tab, focus on open and on close
- The primary button's label names its action
- Strings: the close control reuses "Close" (`common.close`), which every shipped language has

### Out of scope

- The New-document dialog and form — because the start screen's New cards share its fields; one form for both is its own spec (`new-document-form`).
- Dialogs without a form or an Apply (About, Settings, Shortcuts, Agents, ComfyUI, update, recovery, file association, unsaved changes, drop, animated image, paste larger, OCR, text placement, tile editor, the AI install wizard) — because their buttons and keys are fixed by their own specs; moving them onto the shell changes nothing a user sees. Backlog.
- The effect panel — because it is not modal over the window (it floats in the canvas area, `effect-panel`).
- Pixel mode's Rotate dialog — because it sits beside the canvas without a dimmed backdrop so the live preview stays visible (`rotsprite`); the shell centres a dialog over a dimmed backdrop.
- Changing what a dialog asks for, its fields or its validation.

## Acceptance criteria

- [ ] AC-01 (always): Every dialog in scope is a `dialog` with `aria-modal="true"`, named by its title, and shows its title in a header with a close control named "Close" at the header's right end.
- [ ] AC-02 (event): When the user activates the close control or presses Escape with the focus anywhere inside the dialog, then the dialog closes as Cancel does: nothing in the document changes and no undo step is added.
- [ ] AC-03 (event): When the user presses Enter with the focus on a field of a dialog in scope, then the dialog does what its primary button does; while the primary button is disabled, Enter does nothing. Enter on a button activates that button only.
- [ ] AC-04 (always): The primary button of every dialog in scope names its action with a verb: Print, Resize, Scale, Save, Export, Apply, Rename. Pixel mode's Grid Settings says "Apply" instead of "OK".
- [ ] AC-05 (event): When a dialog in scope opens, then the focus is on its first field, or on its primary button when it has no field.
- [ ] AC-06 (event): When the user presses Tab on the dialog's last focusable control, then the focus moves to its first; Shift+Tab on the first moves to the last. The focus never leaves the dialog while it is open.
- [ ] AC-07 (event): When a dialog in scope closes, by its primary button, Cancel, the close control or Escape, then the focus returns to the element that had it before the dialog opened, or to the canvas area when that element is gone (a menu item is gone once its menu closed).
- [ ] AC-08 (state): While a dialog in scope is open, keys typed in it do not reach the editor: no tool changes, no shortcut runs.
- [ ] AC-09 (event): When the user presses on the backdrop outside the dialog, then the dialog stays open.
- [ ] AC-10 (option): Where the UI language is German, the close control is named "Schließen" and Grid Settings' primary button reads "Anwenden".

## Technical notes

Hints, not contracts.

- `ui/src/modes/image/components/dialogs/Modal.svelte` already does backdrop, header, Cancel / secondary / Apply footer, Escape, Enter outside buttons and focus on the first control, and sets `ui.modalOpen`. A shell in `ui/src/lib/components/` with the same props plus the close control, `aria-modal`, the Tab wrap and the focus return can replace it; Image mode keeps setting `ui.modalOpen` around it.
- Pixel mode's dialogs are forms with `onsubmit`; the shell can render the form or take the dialog's own submit as `onApply`.
- `App.svelte` and `ImageEditor.svelte` already keep editor shortcuts away from `[role=dialog]` elements.
- Today a click on Image mode's backdrop closes the dialog; AC-09 changes that, so a stray click cannot throw away typed values.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A select's list is open and the user presses Escape | The list closes; the dialog stays (the browser handles that Escape) |
| Enter in a multi-line text field | Not in any dialog in scope today; a shell option would keep Enter for the field |
| A field is invalid and the primary button is disabled | Enter does nothing; the field keeps its value |
| The dialog's content scrolls | The header with the close control stays in view |
| Phone width | The dialog is at most the window's width minus a margin, as today |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A backdrop click that does nothing is expected | Photoshop, paint.net and macOS sheets ignore it; Image mode closing on it lost typed values | Low |
| The dialogs out of scope can follow later without a visible inconsistency that matters | They have no Apply; most are informational | Medium |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs of both modes}: one shell — modal, named by its title, a Close control, a verb, focus on the first field" | built |        |
| AC-02 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs}: Close, Escape and Cancel change nothing; the backdrop does not close" | built |        |
| AC-03 | `ui/e2e/dialogs.spec.ts`: "pixel: Enter in a field does what the primary button does; disabled, it does nothing", "image: Enter in a field applies; Enter with the primary disabled does nothing" | built |        |
| AC-04 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs}: one shell — …, a verb, …"; `ui/e2e/grid.spec.ts` (Apply) | built |        |
| AC-05 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs}: one shell — …, focus on the first field" | built |        |
| AC-06 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs}: Tab stays inside; keys reach no tool; the focus returns on close"; `ui/e2e/export-svg.spec.ts`: "the dialog takes focus, keeps Tab inside and returns focus on close" | built |        |
| AC-07 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs}: Tab stays inside; …; the focus returns on close", "the focus returns to the element that had it before the dialog opened" | built |        |
| AC-08 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs}: Tab stays inside; keys reach no tool; …" | built |        |
| AC-09 | `ui/e2e/dialogs.spec.ts`: "{11 dialogs}: Close, Escape and Cancel change nothing; the backdrop does not close" | built |        |
| AC-10 | `ui/e2e/dialogs.spec.ts`: "German: Close reads \"Schließen\" and Grid Settings applies with \"Anwenden\"" | built |        |

## Decisions

- 2026-11-25 (owner, backlog): one shared dialog component with title, close button, Escape / Enter and a verb on the primary button.
- 2026-11-26 (spec): the form dialogs come first; dialogs without an Apply keep their own markup for now. The New-document form gets its own spec.
- 2026-11-26 (build): `ui/src/lib/components/Dialog.svelte` is the shell; Image mode's `Modal.svelte` wraps it and keeps setting `ui.modalOpen`. Thirteen dialogs use it (see Context). The tests cover eleven of them through the menus; Rename is covered through the command palette, the Radius family through Feather in Image mode, and Pixel mode's Sprite Sheet export through the existing `ui/e2e/sprite-sheet*.spec.ts`.
- 2026-11-26 (build): the shell's close control is the first focusable element, so Shift+Tab from the first field reaches Close before it wraps to the primary button (`export-svg.spec.ts` changed accordingly).
- 2026-11-26 (build): keys stop at the shell after it handled them, except chords with Ctrl, ⌘ or Alt: the editors already ignore those while a dialog is open, and the browser's own must stay the browser's (Ctrl+P while Save As is open, `image-formats-more-print` AC-09).
- 2026-11-26 (build): red run — the new tests of `ui/e2e/dialogs.spec.ts` failed against the bundle built before the change (37 at that time; the suite was trimmed to 34 when Pixel mode's Feather turned out not to exist and Image mode's PNG export to open no dialog).
- 2026-11-26 (build): found, not changed: Pixel mode's Resize dialog shows "Current size: {w}×{h} px" without `t()`; it predates this spec.
- 2026-11-26 (build): New public API, for review: the component `lib/components/Dialog.svelte` (props `title`, `applyLabel`, `onApply`, `onClose`, `secondary`, `applyDisabled`, `class`).
