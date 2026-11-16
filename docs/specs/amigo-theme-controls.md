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
| Touch input (tablet, phone) | A button's hit area is never smaller than before the change, also where its drawn height is 24 px. A dense field in a panel is 24 px high on touch, 4 px less than before: a field cannot carry a hit-area pseudo-element, and its tap target is the whole field |
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
| AC-01 | "{Dark, Light}: text buttons are the shared button in its four roles, 24 px in the frame and 32 px elsewhere (AC-01)" in `ui/e2e/amigo-theme-controls.spec.ts` | built  |        |
| AC-02 | "{Dark, Light}: fields and selects are the shared field, 24 px in the frame and 32 px elsewhere (AC-02)" in `ui/e2e/amigo-theme-controls.spec.ts`; the danger edge of a wrong value also in `ui/e2e/image-resize-autocrop.spec.ts` ("an invalid used field blocks Apply and Enter (AC-07)") | built  |        |
| AC-03 | "{Dark, Light}: checkboxes and radios are the shared toggle (AC-03)" in `ui/e2e/amigo-theme-controls.spec.ts` | built  |        |
| AC-04 | "{Dark, Light}: sliders are the shared slider: the travelled part in accent on a border-strong rail (AC-04)" in `ui/e2e/amigo-theme-controls.spec.ts` | built  |        |
| AC-05 | "{Dark, Light}: the tool variants and the theme choice are the shared segmented control (AC-05)", "{Dark, Light}: the phone's panel sheet has the shared tab row (AC-05)", "{Dark, Light}: the preview's zoom choice is the shared segmented control (AC-05)" in `ui/e2e/amigo-theme-controls.spec.ts` | built  |        |
| AC-06 | "{Dark, Light}: menus from the menu bar, context menus and tool flyouts are the shared menu (AC-06)" in `ui/e2e/amigo-theme-controls.spec.ts`; menu structure, hints and keyboard in `menu-structure.spec.ts`, `tool-flyout.spec.ts`, `palette.spec.ts` | built  |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 | button, field, toggle, slider, segmented control and tabs only: "{Dark, Light}: each control family looks like the shared component rendered alone (AC-10)" in `ui/e2e/amigo-theme-controls.spec.ts` | open   |        |
| AC-11 |      | open   |        |

## Decisions

- 2026-11-15: What the shared theme lacks for Pincel (a slider, dense variants) is added in amigo-labs/website and regenerated, never overridden in Pincel, because an override here would be the next private look.
- 2026-11-16 (build): The markup writes the shared classes itself (`am-btn`, `am-input`, `am-check`, `am-range`); `.panel-btn`, `.opt-field` and the element rules for sliders, checkboxes and radios in `app.css` are gone, and `Button.svelte` renders `am-btn` with its role. A CSS class cannot stand for another one without copying its rules, and a copy would be the private look this spec removes. The technical note "callers do not change" holds for `Button.svelte`'s props; callers that wrote `panel-btn` by hand write `am-btn` now.
- 2026-11-16 (build): `components.css` is loaded as a whole into the `components` cascade layer (`app.css`), after `tokens.css`. In that layer a layout utility on a control (a width, a flex share, `w-auto` for a field that is not full width) still applies, and nothing in Pincel restyles a shared class. Its base rules come with it: body text is `text-body` (14/20) and the focus ring sits 2 px off the control instead of 1 px.
- 2026-11-16 (build): Dense or not is decided where the control is written: `am-btn--sm` / `am-input--sm` in the options bars, the panels, the timeline, the status bar and the top bar's tour hint; the plain 32 px classes in dialogs, popovers (colour picker, font picker, effect panel, tour), the notices and on the start screen (its controls were 36 px). The font control finds out by itself whether it sits in a dialog, so `FontPicker` keeps its props.
- 2026-11-16 (build): AC-01 reads a `filter` as a change of the fill: the shared primary and danger buttons brighten their fill on hover and pressed (`filter: brightness()`), their computed background colour stays. A pressed ghost or danger button looks like the hovered one in the shared theme; both differ from the resting state, which is what the AC asks.
- 2026-11-16 (build): A busy button is a disabled shared button with the spinner (the edge-case table says so); the former busy look that kept the button's colours is gone. No button can be held busy from a test, so the AC-01 test puts the Create button into the state `Button.svelte` renders (`disabled`, `aria-busy`) and reads it.
- 2026-11-16 (build): A text button that is a toggle and switched on (Clone Stamp's "Set source", an effect's "Pick", the anchor of Pixel mode's Canvas Size) takes the primary role, because the shared button has no pressed state; that matches the icon buttons, which are `accent-solid` when on.
- 2026-11-16 (build): What the shared field has no rule for stays as Pincel had it, without touching the field's look: the chevron of a select (now in the unlayered select rule, because the shared field resets `background`), number fields without spin buttons, a textarea's own height (`h-auto` plus its rows or `min-h-*`), and `disabled:opacity-50` where a caller wrote it (the shared field has no disabled look). The soft glow of a focused field is gone: the shared ring replaces the outline. The forced-colours fallback of checkboxes and radios is gone with their element rules; the shared toggle has none.
- 2026-11-16 (build): Touch hit areas: a dense button gets a transparent `::after` that reaches 2 px above and below it (28 px as before, drawn 24 px), and a tool option's label stretches over the bar's height, so a tap next to its field lands on it. A 24 px field in a panel (Slices, Tilesets, the tag editor) has no such help (owner decision below).
- 2026-11-16 (build): The frame-duration field inside a timeline cell (transparent until hovered) stays as it is: it is part of the frame cell, which is out of scope. The colour picker's hue and alpha strips keep their own slider look for the same reason; every other slider is `am-range`, its travelled part set by the new action `rangeFill` (`ui/src/lib/range-fill.ts`, new public API).
- 2026-11-16 (build): Red run before block 1 (build of ac1c67d, all 8 tests of the new spec failed): AC-01 a start-screen button 36 px high with the hairline edge; AC-02 "start screen, Width: height" 36 instead of 32; AC-03 the checkbox 14 px with the old fill; AC-04 no `--am-range-fill` on a slider.
- 2026-11-16 (build): AC-09 allows spec changes only where a spec named a style class. `ui/e2e/amigo-theme.spec.ts` also changed where it asserted the field look that AC-02 replaces: a focused field's ring is 2 px instead of the 1 px edge, and the outline check reads a field at rest (a focused shared field has the ring instead of its outline) and takes a checked toggle's fill as its edge (the shared toggle has no outline when checked). `ui/e2e/buttons-tooltips.spec.ts` changed only in its class names.
- 2026-11-16 (build): First part of block 2: the tab row of the phone's panel sheet is `am-tabs`, with the sheet's close button beside the row instead of inside it (`.am-tabs button` would style it as a tab), and the preview's zoom choice, three toggle buttons with `aria-pressed`, is `am-seg am-seg--sm`. The document tab bar keeps its shape: its tabs carry a close button and a dirty mark and sit on the frame like browser tabs, and AC-05 names only the phone's panel sheet. Red run before the change (build of dd7d04f): the selected tab had the `active` fill and `text` instead of `accent-ink` over an `accent` underline; the zoom choice had no `surface-raised` ground.
- 2026-11-16 (build): AC-10 compares against a page that holds only `ui/src/theme/tokens.css`, `components.css` and the component's markup (`sharedPage` in `ui/e2e/controls-helpers.ts`). A toggle and a slider are compared without the text colour (they have no text; the property is their label's colour), a slider in addition pixel by pixel along the rail and on the thumb. The families of blocks 2 and 3 that are not built (menu, dialog, panel, banner, chip, progress, and the two segmented controls of the Open questions) are not in the test yet, so AC-10 stays open.
- 2026-11-16 (build): The build stopped after block 2. Not started: AC-07 (dialogs, panels), AC-08 (notices, chips, progress) and with them the leftovers that belong to those families (the ComfyUI variant thumbnail, the two inline `alertdialog` boxes, list boxes on the fixed 4 px radius); AC-09 and AC-10 can only close after them.
- 2026-11-16 (owner): Dense fields are exempt from the touch hit-area rule (edge case revised): a field cannot carry a hit-area pseudo-element, and its tap target is the whole field.
- 2026-11-16 (build): The theme files are regenerated from amigo-labs/website `feat/theme-menu-seg-states` (website#7): menu states (`data-active`, inset focus ring, disabled item, `am-menu-check`, `am-menu-label`), segments that are radios (`aria-checked`, a label around a native radio), the toggle button (`am-btn[aria-pressed="true"]`), `:active` for ghost and danger, `am-input:disabled`. The Site data check of this repository passes once website#7 is merged. Clone Stamp's "Set source" and an effect's "Pick" use the toggle state now instead of the primary role; the anchor of Pixel mode's Canvas Size is a radio group and keeps the primary role for its checked button.
- 2026-11-16 (build): The tool variants are `am-seg am-seg--sm` with their `role="radio"` buttons; the theme choice is `am-seg` with a label around each native radio. The shared theme hides that radio and takes pointer events off it, so a Playwright `check()` on the radio needs `force: true` (the press lands on the label, as a user's does); `amigo-theme.spec.ts` and the helpers changed there, and the predecessor's "radio outline" check on the theme choice is gone (a radio's outline is covered by AC-03 on Scale Image's size mode; the segmented control's own edge is the shared hairline).
- 2026-11-16 (build): Menus are `am-menu`: items are its buttons, the check mark sits in `am-menu-check` inside `am-menu-label` (as an inline SVG, so the label's text stays the item's name; as siblings the menu's 24 px gap would separate mark and label), the shortcut is `am-kbd`, separators are `hr`. The item with the keyboard focus, the row of an open submenu and the flyout entry under a long-press finger carry `data-active` (new action `activeOnFocus`, `ui/src/lib/components/layout/menu-focus.ts`). New optional field `danger` on `MenuAction`; Delete Layer, Delete Frame and Image mode's Reset palette are drawn in `danger-ink`. The menu bar's own entries (File, Edit, …) keep their shape: AC-06 is about the menus they open. Specs that read menu rows by `span.flex-1`, `[role="separator"]` or `.text-pc-faint` read `.am-menu-label`, `hr` and `kbd` now.
- 2026-11-16 (build): Red run before the rest of block 2 (build of 18e0231 with the regenerated theme): the variants' group was 34 px high instead of 24, a menu item 32 px with its own padding instead of the shared 30 px.

