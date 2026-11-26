---
status: todo
created: 2026-11-26
size: M
mode: shell
design: [pincel.md §9.2]
---

# One size form for New, with presets

## Context

Part of the UI refresh (owner request 2026-11-25; backlog "UI refresh: dialogs": one New-document form for the start screen and the dialog, with size presets).

A new document is made in two places: the start screen's New cards and the New dialog (File ▸ New…). Both ask for width and height, but differently:

- The dialog explains under a field what Create will make of an empty, invalid or out-of-range value; the start card does not, it silently clamps (`app-shell` AC-06).
- Neither offers common sizes, so every 32 × 32 sprite and every 1920 × 1080 image is typed by hand.

Terms:

- **Size form**: the Width and Height fields with their notes and the presets, in a New card of the start screen or in the New dialog.
- **Note**: the line under a field, or under both, that says what Create makes of the value (the dialog's `newDoc.*` texts).
- **Preset**: a button that puts a common size into both fields.

## Scope

### In scope

- The same size form in both places: fields, notes, presets
- Presets per mode
- Strings: the presets group's name in every shipped UI language

### Out of scope

- The mode choice (radios in the dialog, one card per mode on the start screen) and the startup checkbox — because they already work the same; only their place differs.
- Custom presets — because no one asked for them.
- Units other than pixels — because both modes make pixel documents.

## Acceptance criteria

- [ ] AC-01 (always): The start screen's New cards and the New dialog show the same size form: Width and Height fields with these names, and the presets of the card's or the selected mode, below the fields.
- [ ] AC-02 (always): The presets are, in this order: Pixel mode 16 × 16, 32 × 32, 64 × 64, 128 × 128, 256 × 256; Image mode 800 × 600, 1280 × 720, 1920 × 1080, 1080 × 1080. Each is a button named by its size ("32 × 32"); together they are a group named "Size presets".
- [ ] AC-03 (event): When the user presses a preset, then both fields hold its size, their notes go, and Create makes a document of that size.
- [ ] AC-04 (state): While a field holds the size of a preset in both fields, that preset is marked as pressed (`aria-pressed`); no other is.
- [ ] AC-05 (state): While a field of the start card is empty, not a number, out of range or not whole, the card shows the dialog's note for it under the fields, and the note for a size above the image area limit; the notes are the fields' descriptions for assistive technology. (Create still clamps as `app-shell` AC-06 says.)
- [ ] AC-06 (event): When the user switches the dialog's mode, then the presets are the new mode's.
- [ ] AC-07 (option): Where the UI language is German, the group is named "Größenvorgaben".

## Technical notes

Hints, not contracts.

- `ui/src/lib/shell/StartScreen.svelte` (New cards, `<form aria-label={card title}>`) and `ui/src/lib/shell/NewDocumentDialog.svelte` (fields with `noteFor` / `areaNote`); clamp and limits in `ui/src/lib/shell/new-doc.ts` (rune-free, so a Node-side spec can test the note logic).
- e2e: `newDocument` in `ui/e2e/helpers.ts` drives the start card; about 34 specs open the dialog by its name "New document".

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A preset larger than the clipboard prefill | The preset replaces the prefilled size |
| The fields hold 32 × 64 | No preset is pressed |
| A phone-width start card | The presets wrap onto as many lines as they need |
| A preset in Image mode above the area limit | None is: all presets are within the limit |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| These sizes cover the common cases | Sprite sizes double; the image sizes are SVGA, HD, Full HD and a square post | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/new-document-form.spec.ts`: "the start cards and the dialog show Width, Height and the mode’s presets below them" | built |        |
| AC-02 | `ui/e2e/new-document-form.spec.ts`: "the start cards and the dialog show Width, Height and the mode’s presets below them" | built |        |
| AC-03 | `ui/e2e/new-document-form.spec.ts`: "a preset fills both fields, is pressed while they hold it, and Create makes that size" | built |        |
| AC-04 | `ui/e2e/new-document-form.spec.ts`: "a preset fills both fields, is pressed while they hold it, and Create makes that size" | built |        |
| AC-05 | `ui/e2e/new-document-form.spec.ts`: "the start card shows the dialog’s notes and describes its fields with them" | built |        |
| AC-06 | `ui/e2e/new-document-form.spec.ts`: "the start cards and the dialog show Width, Height and the mode’s presets below them" | built |        |
| AC-07 | `ui/e2e/new-document-form.spec.ts`: "German: the presets group is named \"Größenvorgaben\"" | built |        |

## Decisions

- 2026-11-25 (owner, backlog): one New-document form for the start screen and the dialog, with size presets.
- 2026-11-26 (spec): "one form" means one size form (fields, notes, presets) shared by both; the mode choice and the startup checkbox keep their places.
- 2026-11-26 (build): the note logic moved from the New dialog into `ui/src/lib/shell/doc-size.ts` (`fieldNote`, `areaNote`, `edgeOf`, `SIZE_PRESETS`); the dialog and the start cards call it, and both render the presets through `SizePresets.svelte`. The start card lists its notes together under the fields, as the card is narrower than the dialog; the dialog keeps one under each field.
- 2026-11-26 (build): `doc-size.ts` stays free of runes, but `new-doc.ts` (the limits) imports a rune module, so the tests drive the forms in the browser rather than in Node.
- 2026-11-26 (build): red run — the 4 tests of `ui/e2e/new-document-form.spec.ts` failed against the bundle built before the change.
- 2026-11-26 (build): New public API, for review: `ui/src/lib/shell/doc-size.ts`, the component `ui/src/lib/shell/SizePresets.svelte`, the string `newDoc.presets`.
