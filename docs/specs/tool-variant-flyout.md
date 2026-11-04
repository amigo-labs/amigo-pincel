---
status: todo
created: 2026-11-04
size: S
mode: both
design: [pincel.md §9.2, pincel.md §9.4]
---

# Tool-variant flyout on the tool rail

## Context

A tool-rail button that groups several variants (the shapes, the marquee shapes, the lasso kinds) carries a small triangle in its corner. In paint.net, Photoshop and Krita that triangle means "press and hold or right-click for the other variants". In Pincel it does nothing: a right-click on the button opens the browser's own context menu, and a long press does nothing. The variants are only reachable through the segmented control at the start of the options bar (spec §9.2) or by pressing the group's key again.

This spec adds the flyout the triangle promises. The options-bar control and the key cycling stay, so §9.2's "no variant *needs* a long-press" stays true; the flyout is a second, faster way.

*Group button*: a tool-rail button whose group has more than one variant (it shows the triangle). *Single button*: a tool-rail button whose group has exactly one tool. *Flyout*: the small menu this spec adds, listing one group's variants. *Ctrl* means Ctrl on Windows / Linux and ⌘ Command on macOS, *Alt* means ⌥ Option on macOS (the `mod` convention of `ui/src/lib/shell/shortcuts.ts`); on macOS a Control-click is a right-click.

## Scope

### In scope

- Opening the flyout from a group button with a right-click, a long press (touch or pen) or the keyboard
- Choosing a variant in the flyout with pointer or keyboard; closing it without a choice
- No browser context menu anywhere on the tool rail
- Both modes, every rail orientation (vertical rail, horizontal phone strip)
- en + de strings; the §9.2 sentence about variants in `pincel.md` updated

### Out of scope

- Changing which tools are grouped, or their order — because the groups are each mode's tool model, unchanged here
- Long-press or context menus on the canvas, panels or layers — because they are a separate interaction design (`tablet-gestures` keeps that out of scope too)
- Tear-off or pinned flyouts — because one choice closes the flyout and the options bar already shows the variants permanently
- Removing the options-bar variant control — because it is the touch-friendly path that needs no hold, and existing e2e specs rely on it

## Acceptance criteria

"The browser's context menu does not open" is observed as: the `contextmenu` event on the rail is cancelled (`defaultPrevented`), since automation cannot see a native menu.

- [ ] AC-01 (event): When the user right-clicks a group button, then a flyout opens next to that button listing every variant of the group in the group's order, each with its icon and name, and the browser's context menu does not open — in Pixel and in Image mode.
- [ ] AC-02 (event): When the user right-clicks or long-presses (as in AC-03) a single button, a group button whose variants are all disabled, or any part of the tool rail that is not a button (separators, padding, the space after the last button), then neither a flyout nor the browser's context menu opens, and the active tool does not change.
- [ ] AC-03 (event): When a mouse, touch or pen presses a group button and stays down for 500 ms without moving more than 10 CSS px and without a second pointer going down, then the flyout opens; releasing over an enabled entry chooses it (AC-06), releasing anywhere else leaves the flyout open with no tool change.
- [ ] AC-04 (event): When a press on a group button is released before 500 ms, or moves more than 10 CSS px, or is cancelled, or a second pointer goes down before 500 ms, then no flyout opens and the button behaves as it does without this feature: a released short press selects the variant the button shows, or the group's first enabled variant when the shown one is disabled.
- [ ] AC-05 (always): The flyout's header shows the group's name and, when the group has one, its shortcut key (e.g. "Shapes (U)"); the entry of the active tool is marked checked when the active tool belongs to the group; an entry whose tool is disabled is shown disabled and cannot be chosen by any means.
- [ ] AC-06 (event): When the user chooses an enabled entry (click, tap, release after a long press, or Enter / Space on the focused entry), then that variant becomes the active tool, the group button shows it, the options-bar variant control marks it, the flyout closes and focus returns to the group button.
- [ ] AC-07 (event): When the flyout is open and the user presses Escape, then it closes without changing the active tool and focus returns to the group button; when the user presses Tab or Shift+Tab, clicks or taps outside the flyout, right-clicks the same group button again, or opens the flyout of another group button, then the first flyout closes without changing the active tool and focus goes where that input puts it (the next element, the clicked element, or the new flyout).
- [ ] AC-08 (event): When the flyout opens, then keyboard focus is on the checked entry, or on the first enabled entry when none is checked; Arrow Down / Arrow Up move to the next / previous enabled entry and wrap around, Home / End go to the first / last enabled entry.
- [ ] AC-09 (event): When a group button has keyboard focus and the user presses Shift+F10 or the context-menu key, then the flyout opens as in AC-08.
- [ ] AC-10 (state): While the flyout is open, a key press other than Arrow Up / Down, Home, End, Enter, Space, Escape and Tab changes nothing in the editor (no tool switch for B or E, no undo for Ctrl+Z, no save for Ctrl+S); after the flyout closes, these keys work again.
- [ ] AC-11 (always): On the vertical rail the flyout's top edge lines up with the button's top edge and the flyout sits right of the rail; on the horizontal phone strip its bottom edge sits above the strip and its left edge lines up with the button's left edge; where that position would cross a window edge, the flyout is shifted back inside, and where the window is shorter than the flyout, the flyout's height is limited to the window and its entries scroll.
- [ ] AC-12 (always): A screen reader announces the open flyout as a menu named after the group, and each entry with its tool name and whether it is checked or disabled.
- [ ] AC-13 (option): Where the UI language is German, the flyout's header and its accessible name are German.
- [ ] AC-14 (event): When, while the flyout is open, the window is resized, the rail is scrolled, the active tool changes by any other means (key, options bar, agent), the document is replaced or the editor is closed, then the flyout closes without changing the active tool.

## Technical notes

Hints, not contracts — paths drift; grep for the names (verified 2026-11-04).

- Rail: `ui/src/lib/components/layout/ToolRail.svelte` (both modes mount it: `PixelEditor.svelte`, `ImageEditor.svelte`); the triangle is the `pointer-events-none` SVG in the button. Group model and helpers: `layout/tools.ts` (`ToolGroup`, `groupOf`, `cycleGroup`). Options-bar control: `layout/ToolVariants.svelte`.
- Long press: the pointer-capture pattern in `ui/src/lib/components/layers/LayersPanel.svelte` (`setPointerCapture`); suppress the click that follows a long press.
- Shortcut blocking while open: the editors check `ui.modalOpen`-style flags before handling keys; the flyout either sets such a flag or stops key events at its root.
- `pincel.md` §9.2 ("so no variant needs a long-press"): add that a right-click or long press on a group button also opens a flyout with the variants.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Group where every variant is disabled (button disabled) | Right-click or long press opens no flyout and no browser menu (AC-02) |
| Long press, then the finger slides onto an enabled entry and lifts | That entry is chosen (AC-03, the press-drag-release of Photoshop and paint.net) |
| Long press, then the finger slides onto a disabled entry and lifts | Nothing is chosen; the flyout stays open |
| Right-click on the active group button | Flyout opens with the active variant checked; the active tool stays until a choice |
| Right-click on the group button whose flyout is open | The flyout closes (AC-07) |
| An entry becomes enabled or disabled while the flyout is open (e.g. Stamp's tile variant) | The entry shows its new state at once |
| Mode has a group with two variants (lasso) | Flyout lists both |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 500 ms hold, 10 CSS px slop | Common long-press values (Android default ~400–500 ms) and the same 10 px slop `tablet-gestures` uses | Low; constants are easy to tune |
| The flyout is not a modal dialog (no backdrop) | It is a menu; outside clicks close it like the menu bar's menus | Low |
| The flyout closes on resize, rail scroll or an outside tool change instead of following its button | It is a transient menu open for a moment; following a moving anchor adds code for a case users rarely hit, and closing never changes the document | Low |
| Mouse hold opens the flyout too, not only touch and pen | paint.net and Photoshop open tool flyouts on mouse hold; one rule for every pointer is simpler to learn and test | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/tool-flyout.spec.ts`: "pixel mode / image mode: a right-click on a group button lists its variants", "macOS › a Control-click opens and closes the flyout like a right-click" | built  |        |
| AC-02 | `ui/e2e/tool-flyout.spec.ts`: "single buttons, separators and the empty rail open nothing"; `ui/e2e/tool-flyout-model.spec.ts`: "hasFlyout: a single button and an all-disabled group open no flyout" | built  |        |
| AC-03 | `ui/e2e/tool-flyout.spec.ts`: "a mouse hold opens the flyout, release over an entry chooses it", "a pen hold opens the flyout and chooses on release", "touch › a touch hold opens the flyout; a cancelled or two-finger press does not"; `ui/e2e/tool-flyout-harness.spec.ts`: "releasing a long press over a disabled entry or empty space chooses nothing" | built  |        |
| AC-04 | `ui/e2e/tool-flyout.spec.ts`: "a short press, a drag or a moved press selects the shown variant", "touch › … a cancelled or two-finger press does not", "touch › a quick tap selects the shown variant"; `ui/e2e/tool-flyout-model.spec.ts`: "pressTarget: …" | built  |        |
| AC-05 | `ui/e2e/tool-flyout.spec.ts`: "header, checked entry and keyboard choice"; `ui/e2e/tool-flyout-model.spec.ts`: "flyoutStep: arrows skip disabled entries …", "flyoutFocus: …"; `ui/e2e/tool-flyout-harness.spec.ts`: "a disabled variant is shown and announced disabled and cannot be chosen", "releasing a long press over a disabled entry …", "an entry enabled or disabled while open shows its new state at once" | built  |        |
| AC-06 | `ui/e2e/tool-flyout.spec.ts`: "a mouse hold opens the flyout, release over an entry chooses it", "header, checked entry and keyboard choice" | built  |        |
| AC-07 | `ui/e2e/tool-flyout.spec.ts`: "Escape, Tab, outside clicks and other buttons close the flyout", "macOS › a Control-click opens and closes …" | built  |        |
| AC-08 | `ui/e2e/tool-flyout.spec.ts`: "header, checked entry and keyboard choice"; `ui/e2e/tool-flyout-model.spec.ts`: "flyoutStep: …", "flyoutFocus: …" | built  |        |
| AC-09 | `ui/e2e/tool-flyout.spec.ts`: "Shift+F10 and the context-menu key open the flyout" | built  |        |
| AC-10 | `ui/e2e/tool-flyout.spec.ts`: "shortcuts while open › the open flyout blocks editor keys, Ctrl and ⌘ alike" | built  |        |
| AC-11 | `ui/e2e/tool-flyout.spec.ts`: "the flyout sits right of the vertical rail", "phone strip › the flyout sits above the strip, inside the window", "short window › a flyout taller than the window is limited to it and scrolls"; `ui/e2e/tool-flyout-model.spec.ts`: "placeFlyout: …" (3 tests) | built  |        |
| AC-12 | `ui/e2e/tool-flyout.spec.ts`: "pixel mode / image mode: a right-click on a group button lists its variants" (role menu named after the group, menuitemradio names and aria-checked), "header, checked entry and keyboard choice" (checked state); `ui/e2e/tool-flyout-model.spec.ts`: disabled rules; `ui/e2e/tool-flyout-harness.spec.ts`: "a disabled variant is shown and announced disabled and cannot be chosen" (ARIA snapshot: `menu "Variants"`, `menuitemradio` with `checked` / `disabled`) | built  |        |
| AC-13 | `ui/e2e/tool-flyout.spec.ts`: "German UI › header and accessible name are German" | built  |        |
| AC-14 | `ui/e2e/tool-flyout.spec.ts`: "closing from outside › resize, rail scroll and an outside tool change close the flyout", "closing from outside › replacing the document closes the flyout"; `ui/e2e/tool-flyout.spec.ts`: "closing from outside › closing the editor closes its flyout"; `ui/e2e/tool-flyout-harness.spec.ts`: "the flyout closes when the document is replaced or the rail unmounts" | built  |        |

## Decisions

- 2026-11-04: The flyout is added next to the options-bar variant control and the key cycling, not instead of them — because the owner asked why the triangle and right-click do nothing (they open the browser menu); the triangle now does what it promises in paint.net-style editors, and the touch path without a hold stays.
- 2026-11-04: Written without a user dialogue while the owner's goal "all planned specs implemented" was active; the plan the owner approved chose "Flyout with variants" for this request. Thresholds and keyboard access follow common menu conventions (WAI-ARIA menu button pattern).
- 2026-11-04: Clarification pass settled without a dialogue (owner goal active): mouse hold opens the flyout too; releasing a long press over an entry chooses it; focus returns to the group button only after Escape or a choice; Tab closes the flyout; the context-menu check is the cancelled `contextmenu` event; the flyout closes on resize, scroll and outside tool changes (AC-14).
- 2026-11-04: owner: macOS uses ⌘/⌥; Control-click = right-click — the key blocking of AC-10 covers ⌘+Z / ⌘+S as well as Ctrl+Z / Ctrl+S, and a Control-click on a group button on macOS opens and closes the flyout like a right-click.
- 2026-11-04 (spec-build): The pure rail model (`hasFlyout`, `pressTarget`, `flyoutStep`, `flyoutFocus`, `placeFlyout` in `ui/src/lib/components/layout/tools.ts`) is tested in the Playwright runner without a browser (`tool-flyout-model.spec.ts`) — because Vitest is not set up and adding it is a new dependency; no shipping multi-variant group has a disabled variant, so the disabled-entry rules of AC-05 / AC-08 and the all-disabled group of AC-02 are proven on a made-up group there.
- 2026-11-04 (spec-build, round 2): The rendered disabled entry (AC-05, AC-12) and the close on document identity / unmount (AC-14) are tested on the real `ToolRail` mounted alone with made-up groups (`ui/e2e/harness/`), served by a Vite dev server `tool-flyout-harness.spec.ts` starts with the existing dev dependencies — because no shipping group has a disabled variant, and a component-test runner would be a new dependency.
- 2026-11-04 (spec-build): A long press on a single button selects nothing (its click is swallowed) — because AC-02 requires the active tool not to change after a long press on a single button.
- 2026-11-04 (spec-build): While the flyout is open, a capture-phase `keydown` listener on the window owns the keyboard and stops every key from reaching the editors (both modes, no editor flag), and cancels the browser default for editor chords and Ctrl / ⌘ chords — because Ctrl+S would otherwise open the browser's "save page" in place of the blocked save.
- 2026-11-04 (spec-build): Tab / Shift+Tab close the flyout and move focus to the tab stop after / before the group button — because the flyout is not in the rail's tab order, so "the next element" is taken relative to its button.
- 2026-11-04 (spec-build): `ToolRail` gets an optional `resetKey` prop (Pixel: the document object, Image: the document handle); the flyout closes when it changes — because in Pixel mode Open… replaces a PNG / sprite in place, without remounting the editor.
- 2026-11-04 (spec-build): Group buttons carry `touch-action: pan-y` on the vertical rail and `pan-x` on the phone strip — because sliding a finger from the button onto the flyout runs across the rail's scroll direction, and the browser must not take that slide as a scroll.
- 2026-11-04 (spec-build): No new i18n keys — because the header and the accessible name are the already translated group label plus its single-key shortcut; no chord label is shown. A Control-click counts as a context press only where `isMac()` (`ui/src/lib/platform`) holds, so a Ctrl+click on Windows / Linux stays a plain click.
