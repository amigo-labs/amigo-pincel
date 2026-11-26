---
status: todo
created: 2026-11-26
size: L
mode: both
design: [pincel.md §9.2, pincel.md §9.4]
---

# Tool rail: colour well and one set of tool groups

## Context

Part of the UI refresh (owner request 2026-11-25: ease of use, logical arrangement, familiar patterns; Decision Log 2026-11-25 "Tool rail"; mock-up `docs/design/ui-refresh-mockup.html`).

Three things about the tool rail differ from what Photoshop, paint.net and Aseprite taught users:

- The foreground and background colour are only shown in the Colors panel. With the sidebar hidden, the Colors panel collapsed or removed (`panel-visibility`), nothing on screen says which colour the next stroke gets.
- The two modes list different groups for the same concept. Image mode has Hand and Zoom tools, Pixel mode has neither. Pixel mode keeps Fill and Gradient in one group on `G`, Image mode has two groups.
- Pixel mode's Shapes group lists Rectangle and Filled Rectangle, Ellipse and Filled Ellipse as separate tools, although the options bar's Mode option (Outline, Fill, Fill + Outline) already says how a shape is drawn.

Terms:

- **Rail**: the tool rail — the vertical strip left of the canvas on windows of 640 px and wider, the horizontal strip along the bottom below that.
- **Tool list**: the rail's group buttons.
- **Colour well**: the control this spec adds at the rail's end: the foreground swatch over the background swatch, a swap control and a reset control.
- **Colors panel**: the sidebar panel "Colors" with the same two swatches (`color-picker`).
- **Colour picker**: the popover a swatch opens (`color-picker`).
- **Step**: one zoom step as Zoom In / Zoom Out make it.
- **Hand**: the view tool Image mode calls "Pan" (German "Verschieben (Ansicht)"), key `H`; this spec says Hand for the tool and uses its UI name in the criteria about names.

## Scope

### In scope

- The colour well at the rail's end in both modes and both rail orientations
- Pixel mode: the tools Hand and Zoom
- Image mode: Gradient joins the Fill group; `N` selects Gradient in both modes
- Pixel mode: the Shapes group without the two filled variants
- The rail when the window is too low or too narrow for every tool
- The tool lists of the shortcut overview and the command palette follow the groups

### Out of scope

- A zoom box drag in Pixel mode — because Pixel mode zooms in fixed steps; a click is enough.
- Tolerance, contiguous and width options for Pixel mode's Bucket and Line — because they need core work (backlog "Pixel tool options").
- Reordering or regrouping tools beyond the points above — because the remaining groups already agree.
- A hex field or alpha slider in the well — because the Colors panel and the picker have them; the well has to fit a 48 px wide rail.
- Changing the Colors panel — because it keeps its job; the well is a second place for the same colours.

## Acceptance criteria

- [ ] AC-01 (always): The editor shows the colour well at the end of the rail in both modes: below the tool list at the foot of the vertical rail, right of the tool list at the end of the horizontal strip. It holds the foreground swatch, overlapping the background swatch, a swap control and a reset control.
- [ ] AC-02 (always): The well's swatches show the current foreground and background colour, whichever way they change: the well itself, the Colors panel, the Eyedropper, a palette swatch, Swap Colors (`X`), Reset Colors (`D`).
- [ ] AC-03 (event): When the user clicks a swatch of the well, then the colour picker for that colour opens next to the rail, wholly inside the window; a colour chosen there is the editor's foreground or background colour, as from the Colors panel. Escape closes the picker and returns the focus to the swatch.
- [ ] AC-04 (event): When the user clicks the well's swap control, then foreground and background colour swap; when they click its reset control, then the colours are the default pair (black on white), as `X` and `D` do.
- [ ] AC-05 (state): While the sidebar is hidden, the Colors panel is collapsed or the Colors panel is removed, the well is shown and works.
- [ ] AC-06 (state): While an effect is open or an Assistant answer runs, the well takes no input, like the tool list.
- [ ] AC-07 (always): The well's controls are named for assistive technology: "Foreground color", "Background color", "Swap foreground and background colors", "Reset to default colors" (the Colors panel's texts, in the UI language), and the swap and reset controls show the shortcut in their tooltip. They are reached with Tab after the tool list.
- [ ] AC-08 (state): While the window is too low for the vertical rail's tool list and well together, the tool list scrolls and the well stays in view at the rail's foot; while the horizontal strip is too narrow, the tool list scrolls sideways and the well stays in view at its end. At 1280 × 720 the vertical rail shows every tool and the well without scrolling, in both modes, in the browser and the desktop app.
- [ ] AC-09 (always): Pixel mode's rail lists Hand and Zoom directly after Move, with the keys `H` and `Z`, as in Image mode.
- [ ] AC-10 (event): When the user drags on the canvas area with the Hand tool in Pixel mode, then the view pans by the drag; the document does not change and no undo step is added.
- [ ] AC-11 (event): When the user clicks on the canvas area with the Zoom tool in Pixel mode, then the view zooms in one step around the pointer; with Alt held, or with the right button, it zooms out one step. The document does not change. At the largest or smallest zoom the click does nothing.
- [ ] AC-12 (state): While an effect is open or an Assistant answer runs, the Hand and Zoom tools still pan and zoom in Pixel mode (the canvas only zooms and pans then).
- [ ] AC-13 (always): Image mode's rail has one Fill group with the variants Fill and Gradient, and no separate Gradient button. `G` selects the group and cycles its variants.
- [ ] AC-14 (event): When the user presses `N` in either mode, then the Gradient tool is selected.
- [ ] AC-15 (always): Pixel mode's Shapes group lists Rectangle, Ellipse, Rounded Rectangle and Polygon, in this order; there is no Filled Rectangle and no Filled Ellipse tool. With the Mode option on Fill, the Rectangle and Ellipse tools draw a filled shape, its interior in the other colour (background for a left-button drag), as the Mode option's hint says.
- [ ] AC-16 (always): The shortcut overview and the command palette list the tools of the rail: Hand, Zoom and Gradient (`N`) in Pixel mode, no Filled Rectangle or Filled Ellipse.
- [ ] AC-17 (option): Where the UI language is German, the well's names read "Vordergrundfarbe", "Hintergrundfarbe" and the swap and reset texts of the Colors panel; Hand and Zoom carry Image mode's German tool names in Pixel mode.

## Technical notes

Hints, not contracts.

- `ui/src/lib/components/ColorPair.svelte` draws the Colors panel's swatches and owns the picker popover; a compact variant can serve the well. Both editors already pass it colours and handlers (`colorsBody` in `PixelEditor.svelte`, the Colors panel in `ImageEditor.svelte`).
- `ui/src/lib/components/layout/ToolRail.svelte` is the rail; today the rail element itself scrolls. The well needs a foot outside the scrolling tool list.
- Groups: `ui/src/modes/pixel/tools.ts` (`toolGroups`), `toolGroups` in `ImageEditor.svelte`; keys by group id in `TOOL_GROUP_KEYS` (`ui/src/lib/shell/shortcuts.ts`). `N` is the key of a group `gradient`, which neither mode has after this spec, so it needs its own handling.
- Pixel mode pans in `pointer.ts` (`view.panning`, Space-drag and middle button) and zooms in `view-ops.ts`; Image mode's Hand and Zoom are handled in `MainCanvas.svelte` before the tool dispatcher.
- `rectangle-fill` / `ellipse-fill` are tool ids in `modes/pixel/tools.ts`, `pointer.ts` and `overlay-painter.ts`.
- e2e: `swatch`, `setColor`, `colorOf` in `ui/e2e/helpers.ts` find the swatches by name; with the well there are two of each.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Toggle UI hides the rail | The well goes with it and returns with it |
| The picker of the well is open and the user opens the Colors panel's picker | Each closes as it does today when focus leaves it; the colours stay one pair |
| Hand or Zoom selected in Pixel mode, then a brush key | The brush is selected; nothing was drawn |
| Zoom tool, right click | Zooms out; no browser context menu |
| Zoom tool click outside the sprite, on the canvas area | Zooms around the pointer all the same |
| Hand tool with a selection | The view pans; the selection does not move (that is the Move tool) |
| Touch: one finger with the Hand tool | Pans; two fingers pinch as before |
| A recorded tool id `rectangle-fill` from an agent or an old session | Not offered; nothing stores the active tool across sessions |
| Phone strip with 17 groups | The tool list scrolls sideways; the well stays at the right end |
| Pixel mode `N` with an indexed sprite where Gradient is unavailable | As choosing the Gradient tool from the rail in that state |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| `H` and `Z` are free in Pixel mode | `shortcuts.ts` binds them only with a modifier (Shift+H, Ctrl+Z) | Low |
| The well fits the dense rail at 1280 × 720 in Image mode (17 groups) | 17 × 33 px plus separators leaves about 50 px | Medium — AC-08 lets the tool list scroll if not, but asks for no scrolling at this size |
| Nobody relies on the filled shape tools' own key cycle | `U` still walks the shapes; Mode is one click in the options bar | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: the well sits at the rail's foot below the tools, which all fit at 1280 × 720", "narrow › {pixel, image}: the well stays at the end of the strip while the tools scroll sideways" | built |        |
| AC-02 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: the well follows the colours and its swatches open the picker beside the rail"; "wide › pixel: the well follows the Eyedropper, a palette swatch and D" | built |        |
| AC-03 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: the well follows the colours and its swatches open the picker beside the rail", "narrow › {pixel, image}: the well stays at the end of the strip …" (picker inside the window) | built |        |
| AC-04 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: the well follows the colours and its swatches open the picker beside the rail" | built |        |
| AC-05 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: the well stays with the sidebar hidden, Colors collapsed or removed; Toggle UI takes it along" | built |        |
| AC-06 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: while an effect is open the well takes no input"; `ui/e2e/refresh-while-answering.spec.ts`: "1280 × 720 › {pixel, image}: while an answer runs the well takes no input; afterwards it does" | built |        |
| AC-07 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: the well is named, shows the keys and follows the tools in the tab order" | built |        |
| AC-08 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: the well sits at the rail's foot below the tools, which all fit at 1280 × 720", "low window › the tool list scrolls and the well stays at the rail’s foot", "narrow › {pixel, image}: the well stays at the end of the strip while the tools scroll sideways"; `ui/e2e/refresh-while-answering.spec.ts`: "1280 × 720 › {pixel, image}: in the desktop app the rail shows every tool and the well without scrolling" | built |        |
| AC-09 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › pixel: Pan and Zoom follow Move, with H and Z" | built |        |
| AC-10 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › pixel: a Pan drag moves the view and changes nothing; a selection stays put" | built |        |
| AC-11 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › pixel: a Zoom click zooms in around the pointer; Alt or the right button zooms out; the ends do nothing" | built |        |
| AC-12 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › pixel: Pan and Zoom still work while an effect is open"; `ui/e2e/refresh-while-answering.spec.ts`: "1280 × 720 › pixel: while an answer runs Pan and Zoom still pan and zoom" | built |        |
| AC-13 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › image: one Fill group with Fill and Gradient; G cycles"; `ui/e2e/tools.spec.ts`: "image mode: W, B, E, G and T select their tools" | built |        |
| AC-14 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › {pixel, image}: N selects the Gradient" | built |        |
| AC-15 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › pixel: Shapes lists four shapes and no filled tool; Mode Fill draws a filled shape in the other colour"; `ui/e2e/tool-flyout.spec.ts`: "pixel mode: a right-click on a group button lists its variants" | built |        |
| AC-16 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › pixel: the shortcut overview and the command palette list Pan, Zoom and Gradient (N), no filled shape"; `ui/e2e/shortcuts-overview.spec.ts`: "{mode} mode: rows carry labels and the menu's chords" | built |        |
| AC-17 | `ui/e2e/tool-rail-colors.spec.ts`: "wide › German: the well and the view tools carry German names" | built |        |

## Decisions

- 2026-11-25 (owner, Decision Log "Tool rail"): colour well at the rail's end; Hand and Zoom in Pixel mode; Fill and Gradient one group on `G` with `N` selecting the gradient; no filled shape variants in Pixel mode.
- 2026-11-26 (spec): the well has swatches, swap and reset only — the rail is 48 px wide; hex and alpha stay in the Colors panel and the picker.
- 2026-11-26 (spec): the filled tools painted the interior in the stroke colour, the Mode option paints it in the other colour. The Mode option's behaviour stays (it is what Image mode does, and the options bar says so); a solid shape in the foreground colour is Fill + Outline with both colours equal, or Swap Colors first.
- 2026-11-26 (spec): Pixel mode's Zoom tool zooms by clicks, without Image mode's zoom box: Pixel mode zooms in fixed steps.
- 2026-11-26 (build): `N` needs no handling of its own: a tool key whose id names no group but a tool inside one selects that tool (`cycleGroup` in `layout/tools.ts`), and the shortcut overview lists such a key as a row of its own. The key stays rebindable as `tool:gradient`.
- 2026-11-26 (build): the well is `ColorPair` with `compact`, so swatch, picker and handlers are the Colors panel's; the rail's scroller is the tool list (`[data-tool-list]`) and the well sits in a foot below it. At 1280 × 720 both modes fit without scrolling (the well is 36 px, Image mode has one group less).
- 2026-11-26 (build): Image mode's Fill variant keeps its name "Fill", so its rail button reads "Fill: Fill" to assistive technology (Pixel mode: "Fill: Bucket"). One wording for both modes is part of `options-bar-layout`.
- 2026-11-26 (build): the Pan and Zoom tools take their press before the "canvas only zooms and pans" guard and before the locked-layer check, so they work on a locked layer and while an effect is open (AC-12).
- 2026-11-26 (build): red run — the 23 tests of `ui/e2e/tool-rail-colors.spec.ts` all failed against the bundle built before the change. Existing specs changed with the feature: the shape tests without the filled tools (`pixel-tools`, `color-recent`, `tool-flyout`), specs about the Colors panel now look at the panel's pair (`colorPair` in `ui/e2e/helpers.ts`) because the well's swatches carry the same names, `command-palette` counts the group buttons only.
- 2026-11-26 (build): New public API, for review: `ColorPair`'s `compact` prop, `ToolRail`'s `foot` snippet, `zoomAt` (`modes/pixel/view-ops.ts`), the tool ids `hand` and `zoom` in Pixel mode (and no `rectangle-fill` / `ellipse-fill`), `modes/image/components/ColorWell.svelte`.
- 2026-11-26 (acceptance, round 1: NOT READY): proof gaps only — the Eyedropper, a palette swatch and D against the well (AC-02), the "answer runs" halves of AC-06 and AC-12 and the desktop app at 1280 × 720 (AC-08). Tests added; they passed at once, the behavior was there.
