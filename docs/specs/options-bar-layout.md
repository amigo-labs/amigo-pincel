---
status: todo
created: 2026-11-26
size: M
mode: both
design: [pincel.md §9.2]
---

# Options bar: tool name first, no sideways scrolling

## Context

Part of the UI refresh (owner request 2026-11-25; mock-up `docs/design/ui-refresh-mockup.html`: "always built the same way … rare things under More, no sideways scrolling").

The options bar above the canvas shows the active tool's options. Two things make it hard to read:

- It does not say which tool the options belong to. The rail shows an icon, the status bar the name — at the other end of the window.
- When the options are wider than the bar, the bar scrolls sideways and has no scrollbar. On a 1280 px window Image mode's Text and Gradient options run out of view; on a phone almost every tool's do. What is out of view cannot be seen to exist.

Terms:

- **Options bar**: the bar between the tab bar and the canvas area (`frame-layout`), on every window width.
- **Tool name**: the active tool's name as the rail's tooltip and the status bar give it.
- **Option**: one control or hint of the active tool in the options bar, the variant switcher included.
- **More control**: the control this spec adds at the bar's end.
- **Expanded bar**: the options bar while it shows every option in as many rows as they need, over the canvas area.

## Scope

### In scope

- The tool name at the start of the options bar in both modes
- No sideways scrolling: options that do not fit one row are reached through the More control
- Keyboard, phone and inert states of the expanded bar
- Strings: the More control's name in every shipped UI language

### Out of scope

- Which options a tool has, and new options — because this spec is about the bar's layout.
- Reordering the options of single tools — because both modes already put size before opacity before shape and mode, and share their labels (`opt.*`); a deviation found while building is fixed there and noted under Decisions.
- A different name for Image mode's Fill variant ("Fill" inside the group "Fill") — because renaming a tool also renames it in the status bar, the palette and the shortcut overview; it gets its own decision.
- Remembering the expanded state — because the bar should come back in one row.

## Acceptance criteria

- [ ] AC-01 (always): The options bar of both modes starts with the tool name, in the text colour of labels that matter (not the muted option colour), followed by the tool's options. It changes with the tool, however the tool is chosen (rail, key, flyout, variant switcher, command palette).
- [ ] AC-02 (always): The options bar never scrolls sideways and never shows part of an option: an option is wholly in the bar's row or not in it.
- [ ] AC-03 (state): While every option fits the bar's row, there is no More control.
- [ ] AC-04 (state): While at least one option does not fit the row, the bar ends with the More control (a button named "More options", with `aria-expanded`); the tool name and the options before the first one that does not fit stay in the row.
- [ ] AC-05 (event): When the user activates the More control, then the bar expands: every option of the tool is shown, in the same order, in as many rows as needed, over the canvas area and not moving it; the More control stays at the end of the first row and reads as expanded.
- [ ] AC-06 (event): When the user activates the More control again, presses Escape with the focus in the bar, presses on the canvas area or the tool rail, or chooses another tool, then the bar is one row again.
- [ ] AC-07 (event): When keyboard focus moves to an option that is not in the row (Tab from the last visible option), then the bar expands, so the focused control is in view.
- [ ] AC-08 (state): While the bar is expanded, the options keep working as in the row: a changed value applies at once and the bar stays expanded.
- [ ] AC-09 (state): While an effect is open or an Assistant answer runs, the options bar takes no input, the More control included; an expanded bar is one row again.
- [ ] AC-10 (state): While the window is narrower than 640 px, AC-01 – AC-08 hold as on wider windows; the tool name is not cut off while the bar is wider than 200 px.
- [ ] AC-11 (event): When the window or the sidebar changes its width, then the row shows as many options as fit the new width, and the More control appears or goes accordingly.
- [ ] AC-12 (option): Where the UI language is German, the More control is named "Weitere Optionen".

## Technical notes

Hints, not contracts.

- The bar is drawn by `ui/src/lib/components/layout/EditorFrame.svelte` (`data-frame="options"`, `role="toolbar"`); the editors pass its content as the `options` snippet. Today it is `overflow-x-auto` with a fade on phones.
- The options are DOM the editors own, with bound state and focus. Showing them in a second place would mean rendering them twice; letting the one container wrap and clipping it to one row keeps a single copy, and "expanded" is then the same container without the clip.
- The tool name: Image mode has `toolLabel` (`ImageEditor.svelte`), Pixel mode the label of the active tool in `railGroups`; the status bar gets the same text through `StatusInfo`.
- `.opt` and `.opt-hint` (`ui/src/app.css`) style the options; a hint is as wide as its text.
- e2e: Image mode's Text tool has the longest bar.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A single option is wider than the whole bar (a long hint on a phone) | It is not in the row; the More control shows it, wrapped to the bar's width |
| The tool has no options | The bar shows the tool name alone; no More control |
| The expanded bar is taller than the canvas area | It scrolls vertically inside the canvas area's height |
| The user changes the tool from the expanded bar's variant switcher | The tool changes and the bar is one row again (AC-06) |
| A select's list or a colour popover opens from an expanded option | It opens as from the row; closing it leaves the bar expanded |
| Toggle UI | The bar goes and returns as today, in one row |
| German or Japanese labels make the options wider | Fewer options fit the row; the rule is the same |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| An expanding bar is an acceptable form of the mock-up's "More" | It shows the same controls in the same order without a second copy, and keeps focus and bound state | Medium — if a separate popover is wanted, the options have to be rendered twice |
| The tool name fits every bar of 200 px and more | The longest names ("Rounded Rectangle", "Verschieben (Ansicht)") are under 160 px at the bar's text size | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/options-bar-layout.spec.ts`: "wide › {pixel, image}: the bar starts with the tool name, however the tool is chosen", "wide › image: a tool without options shows its name alone" | built |        |
| AC-02 | `ui/e2e/options-bar-layout.spec.ts`: "wide › {pixel, image}: options that fit show no More control and nothing is cut", "wide › image: Text has more options than fit; More expands the bar over the canvas and back" | built |        |
| AC-03 | `ui/e2e/options-bar-layout.spec.ts`: "wide › {pixel, image}: options that fit show no More control and nothing is cut" | built |        |
| AC-04 | `ui/e2e/options-bar-layout.spec.ts`: "wide › image: Text has more options than fit; More expands the bar over the canvas and back" | built |        |
| AC-05 | `ui/e2e/options-bar-layout.spec.ts`: "wide › image: Text has more options than fit; More expands the bar over the canvas and back" | built |        |
| AC-06 | `ui/e2e/options-bar-layout.spec.ts`: "wide › image: Text has more options than fit; More expands the bar over the canvas and back" (More again, Escape in the bar and on the control, canvas press, rail press, another tool) | built |        |
| AC-07 | `ui/e2e/options-bar-layout.spec.ts`: "wide › image: Tab onto an option outside the row expands the bar; a value changed there applies and the bar stays" | built |        |
| AC-08 | `ui/e2e/options-bar-layout.spec.ts`: "wide › image: Tab onto an option outside the row expands the bar; a value changed there applies and the bar stays" | built |        |
| AC-09 | `ui/e2e/options-bar-layout.spec.ts`: "wide › image: while an effect is open the bar takes no input and is one row" | built |        |
| AC-10 | `ui/e2e/options-bar-layout.spec.ts`: "narrow › {pixel, image}: on a phone the bar names the tool, does not scroll sideways and expands" | built |        |
| AC-11 | `ui/e2e/options-bar-layout.spec.ts`: "wide › image: the row follows the bar’s width" | built |        |
| AC-12 | `ui/e2e/options-bar-layout.spec.ts`: "wide › German: the More control reads \"Weitere Optionen\"" | built |        |

## Decisions

- 2026-11-25 (owner, mock-up): the options bar is built the same way for every tool, names the tool, and does not scroll sideways.
- 2026-11-26 (spec): "More" expands the bar itself instead of opening a separate popover with a second copy of the options: one copy keeps focus, bound state and order.
- 2026-11-26 (build): `OptionsBar.svelte` wraps the options and clips every line but the first: the tool name is as high as the row, so the first line is the row, and a row gap larger than the row keeps wrapped lines out of the clip. "Expanded" is the same element without the clip, positioned over the canvas area. A More control is a three-dots button beside the bar, shown while any option sits below the first line.
- 2026-11-26 (build): Image mode's `ToolOptions` lost its wrapper element, so its options are direct children of the bar like Pixel mode's and wrap one by one; they now take the bar's text size (12 px instead of 14 px), as Pixel mode's always did.
- 2026-11-26 (build): Pixel mode showed the tool name as a hint for tools without options; that hint is gone, the name stands first for every tool.
- 2026-11-26 (build): Escape closes the expanded bar from the More control too (it sits beside the bar, and "focus in the bar" would otherwise exclude the control that opened it). A press closes the bar only on the canvas area, a toolbar or a frame bar: a colour popover or a select list that an option opened lies outside those.
- 2026-11-26 (build): red run — the 12 tests of `ui/e2e/options-bar-layout.spec.ts` all failed against the bundle built before the change.
- 2026-11-26 (build): New public API, for review: the component `layout/OptionsBar.svelte`, `EditorFrame`'s `toolName` prop, the string `frame.moreOptions`.
- 2026-11-26 (build): the row clips with `overflow: clip`, which nothing can scroll. With `overflow: hidden` a script's or the browser's scroll-into-view moved a wrapped line into the row's place. For e2e specs this means a click on an option outside the row no longer finds it: `showAllOptions` in `ui/e2e/helpers.ts` expands the bar first (`image-antialias`, `image-clone-stamp`, `pressure`).
