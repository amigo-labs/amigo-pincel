---
status: todo
created: 2026-11-15
size: L
design: [pincel.md §9.2, pincel.md §15]
mode: both
---

# Controls on the shared amigo components

## Context

`amigo-theme` gives Pincel the colours, font and radii of the shared amigo theme. Its controls are still Pincel's own: `.panel-btn` and `Button.svelte`, the option fields, sliders, checkboxes and radios styled in `app.css`, its menus, dialogs and panels. The owner wants the apps to look the same in full (Decision Log 2026-11-15), so the controls take the shape of the shared components too: the `am-*` classes of the theme files' `components.css`.

Terms as in `amigo-theme`. **Shared component**: a control family of `components.css` (button, field, toggle, segmented control, tabs, chip, progress bar, banner, panel, menu, dialog, and the slider this spec needs). **Dense**: the 24 px height (`control-sm`) of buttons, fields and segmented controls that sit in the editor frame of §9.2 (top bar, options bar, panels, timeline, status bar); in dialogs, popovers and on the start screen they are 32 px (`control-md`). Menus have the shared menu's own item height everywhere. **Busy**: a button whose action is running (spinner, `aria-busy`), as today.

## Scope

### In scope

- Buttons, text / number fields and selects, checkboxes, radios and switches, sliders, segmented controls, tabs, menus (menu bar, context menus, flyouts), dialogs, panels, banners and notices, chips and progress bars, in both editors, the start screen and every dialog.
- A slider in the shared theme, added in amigo-labs/website first.

### Out of scope

- Controls that are Pincel's own instruments — the colour picker's area and hue strips, the palette swatches, the timeline's frame cells, the layer thumbnails, the angle dial — because the shared theme has no counterpart; they keep their shape and take their colours from the tokens through `amigo-theme`.
- The shape of icon-only buttons (tool rail, panel headers) — because the shared theme has no icon button; they stay square and take the shared button's colours: the ghost role when idle, `accent-solid` with `on-accent` when active or switched on (`amigo-theme` AC-03).
- Layout of the editor frame (§9.2) — because only the controls change, not where they sit.
- New Svelte component APIs — because existing components keep their props; only what they render changes.

## Acceptance criteria

- [ ] AC-01 (always): Every text button in the UI has the shared button's look in one of its four roles (primary, default, ghost, danger): the role's fill, text colour and outline, `radius-sm`, and the height 24 px inside the editor frame and 32 px in dialogs and on the start screen; hover, pressed, focus, disabled and busy each change the button's computed fill, outline or opacity against its resting state in both palettes.
- [ ] AC-02 (always): Text fields, number fields and selects have the shared field's look: `surface-raised` with a `border-strong` outline, `radius-sm`, the focus ring replacing the outline, an invalid value outlined in `danger-ink` (a focused invalid field shows the focus ring and keeps its error text); 24 px high inside the editor frame, 32 px in dialogs, popovers and on the start screen.
- [ ] AC-03 (always): Checkboxes, radios and switches have the shared toggle's look: unchecked a `border-strong` outline, checked `accent-solid` with an `on-accent` mark.
- [ ] AC-04 (always): Every slider has the shared slider's look (a `border-strong` rail with the travelled part in `accent`, an `accent` thumb).
- [ ] AC-05 (always): Segmented controls (tool variants in the options bar, the theme choice) and tab rows (the phone's panel sheet) have the shared look: the selected segment raised on `surface`, the selected tab in `accent-ink` over an `accent` underline.
- [ ] AC-06 (always): Menus opened from the menu bar, context menus and tool flyouts have the shared menu's look: `surface`, `radius-md`, `elev-2`, the shared item height, the shortcut as a key cap at the right, destructive items in `danger-ink`, disabled items in `text-muted` without a hover fill.
- [ ] AC-07 (always): Dialogs have the shared dialog's look: `surface`, `radius-lg`, `elev-3`, the title in the `title` text style, the buttons at the bottom right, the safe choice (Cancel, Keep) first and the action (primary or danger) last; panels have the shared panel's header and hairline.
- [ ] AC-08 (always): Notices (update notice, recovery, clipboard offer, status messages with a state) use the shared banner and chip: `success` for something finished, `warning` for something that needs attention, `danger` for a failure, neutral otherwise, always with their text.
- [ ] AC-09 (always): Every control keeps its accessible name, role, keyboard behavior and shortcut; the existing end-to-end suite passes, its specs changed only where they addressed a control by a style class.
- [ ] AC-10 (always): For each control family of AC-01 to AC-08, a control in the app has the same computed fill, text colour, outline, radius and height as the shared component of that family rendered alone from the theme files, in both palettes.
- [ ] AC-11 (state, manual: visual design): While either palette is active, both editors with every panel open, the start screen and the Settings, New, Export, Effects and Keyboard Shortcuts dialogs look consistent: no control of an old shape next to a shared one, nothing clipped or misaligned at 1280 px, 768 px and 390 px width (the three layouts of §9.2); sliders also look right in WebKit (the macOS desktop app or an iPad), which the automated suite does not run.

## Technical notes

- Depends on `amigo-theme`. Before the build, amigo-labs/website gains the slider (`am-range`) in `theme/components.css` and the theme files here are regenerated; that is a change in the other repository, reviewed there. If the regenerated theme files lack the slider, the build of this spec stops and says so.
- Dense controls use the shared classes with `control-sm`; if `components.css` lacks a dense variant of a family, it is added in the website, not overridden here.
- `Button.svelte` and `.panel-btn` both end up rendering the shared button; callers do not change.
- Tests: Playwright specs read computed radius, height and colours of one representative per control family in Pixel mode, Image mode, a dialog and the start screen, in both palettes.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A button with only an icon (tool rail, panel header) | Keeps its square shape; ghost colours when idle, `accent-solid` when active |
| A checkbox that is checked and disabled; a button that is busy | Keeps its checked mark at reduced contrast; the busy button is disabled and shows its spinner |
| A menu taller or wider than the window | Scrolls inside the window, as today |
| A long German or Japanese label in a 24 px button | The button grows in width; the label is not clipped |
| A slider at its minimum or maximum | The thumb stays inside the rail's ends |
| A disabled control in the light palette | Reads as disabled and still shows its label at 3:1 or more |
| Touch input (tablet, phone) | A control's hit area is never smaller than before the change, also where its drawn height is 24 px |
| Reduce motion is on | Control transitions are off |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 24 px controls inside the editor frame, 32 px elsewhere | The shared theme names `control-sm` for editors and inspectors; Pincel's options bar is built for 24 px | A panel that needs more room shows in AC-11 |
| Chromium is the automated engine; WebKit is checked by hand (AC-11), Firefox is not checked | The e2e suite runs Chromium only; the desktop app on macOS and iPads use WebKit; Firefox users are few and the slider keeps its Firefox rules | A Firefox-only slider defect goes unnoticed until reported |

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

## Decisions

- 2026-11-15: What the shared theme lacks for Pincel (a slider, dense variants) is added in amigo-labs/website and regenerated, never overridden in Pincel, because an override here would be the next private look.
