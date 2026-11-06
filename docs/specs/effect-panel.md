---
status: implemented
created: 2026-11-04
size: M
mode: both
design: [pincel.md §9.2, pincel.md §15 (2026-10-11 "One effect wire format", 2026-11-04 docked effect panel)]
---

# Docked effect panel and resizable sidebar

## Context

An effect or adjustment with parameters opens a floating box pinned to the top-right corner of the window, fixed at 320 px wide. It cannot be moved or resized, and it covers the canvas right where the user wants to judge the preview (owner: "the modal cannot be moved or resized — how can we do this better?"). The owner chose a docked side panel: the effect's controls live in the right sidebar, next to the canvas instead of over it, and the sidebar's width can be changed with a splitter.

Decision Log 2026-11-04 ("The effect UI becomes a docked panel") records this; it amends 2026-10-11 ("One effect wire format"), whose dialog is a floating panel. What the controls do — parameters, Preview, Reset, OK, Cancel, Enter, Escape — stays as `effects-adjustments` and `effect-catalog-v2` specify.

**Revision 2026-11-05.** The owner found the docked panel misleading: the drawing tools and panels beside it looked usable but did nothing while an effect was open (owner: "make the effects a small modal, otherwise it is misleading when the tools do not work meanwhile"). The effect UI becomes a small dialog over the canvas that the user can drag by its title bar; the rest of the UI is dimmed while it is open, the canvas is not, and zoom and pan keep working. The splitter stays. Decision Log 2026-11-05 ("The effect UI becomes a small movable dialog") amends 2026-11-04.

**Terms.** *Effect panel*: since the 2026-11-05 revision, the small dialog showing one effect's name and controls; it floats over the canvas area at window widths of 640 px and more, and is docked to the bottom edge of the window below 640 px. *Title bar*: the effect panel's header row with the effect's name and the close button (×). *Canvas area*: the part of the editor between the menu / tab bars, the tool rail, the sidebar and the panels docked under the canvas, where the canvas and the stage around it are shown. *Dimmed*: covered by a translucent dark shade, so that the content stays visible but reads as unavailable.

The revision supersedes these 2026-11-04 decisions: the frame showing the sidebar, drawer or sheet for the effect; the extra bottom-sheet tab; and the non-modal `aria-modal="false"` panel. *Sidebar*: the right-hand column of docked panels (Colors, Layers, …) at window widths ≥ 1024 px; the *drawer* is the same column shown over the canvas at 640–1023 px; the *bottom sheet* is the phone layout below 640 px (pincel.md §9.2). *Open effect*: an effect whose panel is shown, between choosing its menu entry and OK or Cancel. *Splitter*: the draggable left edge of the docked sidebar.

## Scope

### In scope

- The effect panel in place of the floating effect box, in both modes and all three layouts; since 2026-11-05 a small dialog that can be dragged by its title bar, with its position kept per mode
- Dimming the rest of the UI (top bar, tool rail, sidebar, panels under the canvas) while an effect is open, but not the canvas and not the Effects and Adjustments menus
- A splitter that changes the docked sidebar's width, kept across reloads per mode
- Viewing the preview while an effect is open: zoom and pan stay available
- en + de strings for the new controls

### Out of scope

- Detachable or draggable panels other than the effect panel — because only the effect panel is modal; the other panels keep their docked layout (pincel.md §9.2)
- Resizing the effect panel — because the owner asked for a small dialog; its height follows its content and scrolls when it does not fit
- Dragging the effect panel below 640 px — because on a phone the dialog spans the window's width, so there is nowhere to drag it
- Resizing the drawer (640–1023 px) or the bottom sheet — because they overlay the canvas only while open and close again after use; on those screens every pixel of a wider overlay hides canvas, so a fixed width is the better default
- Faster previews for heavy effects — because moving effect work off the main thread is `performance-background-jobs`
- Several effects open at once, or effect layers — because an effect is applied destructively one at a time (non-destructive layer effects are in the backlog)
- Reordering panels in the sidebar — because the order is part of each mode's layout design (pincel.md §9.2), and a user-defined order needs its own persistence and drag interaction

## Acceptance criteria

- [x] AC-01 (event): When the user chooses an Effects or Adjustments entry in a window at least 640 px wide, then the effect panel appears as a dialog between 280 and 360 px wide over the canvas area, titled with the effect's name, with the same controls, Preview checkbox and Reset / Cancel / OK buttons as before and a close button (×) in its title bar; the docked sidebar and the drawer keep the panels they showed.
- [x] AC-02 (event): When an effect opens or ends, then the sidebar's, drawer's and bottom sheet's visibility and selected tab do not change, and neither does the stored sidebar visibility.
- [x] AC-03 (state): While an effect is open, everything outside the canvas area and the effect panel (the top bar with its menu bar, undo / redo and panels toggle, document tab bar, tool rail, sidebar, drawer, bottom sheet, status bar and the panels docked under the canvas) is dimmed and inert: its controls cannot be focused or clicked, and a click on another layer in the Layers panel does not change the active layer. The one exception are the menu bar's Effects and Adjustments menus — on phones the ☰ button and those two menus in its sheet — which stay undimmed and usable, so another effect can be chosen (AC-05); the other menus (File, Edit, View, …), undo / redo and the panels toggle are inert. No shade covers the canvas area. The canvas does not react to drawing tools, and shortcuts stay blocked as before except Space; zooming and panning the canvas (wheel, pinch, the zoom control in the canvas area, Space-drag, middle-button drag) keep working.
- [x] AC-04 (event): When the user presses OK (or Enter outside a multi-line field), then the effect is applied as before, the effect panel disappears and the dimming ends; when the user presses Cancel, the close button or Escape, then the document is unchanged and the panel disappears the same way.
- [x] AC-05 (event): When the user chooses an Effects or Adjustments entry while an effect is open — another one or the same one again — then the open effect is cancelled (no change to the document, its preview removed) and the panel shows the chosen effect at its defaults, with focus on its first control.
- [x] AC-06 (event): When the effect panel opens, then keyboard focus moves to its first control, or to OK when the effect has no parameters (so Enter applies it at once, as in a confirmation dialog); when it closes, focus moves to the canvas area — the menu entry that opened it no longer exists.
- [x] AC-07 (event): When the user drags the splitter, then the docked sidebar's width follows the pointer, limited to at least 240 px and at most 560 px or half the window width, whichever is smaller; the canvas area takes the remaining width.
- [x] AC-08 (event): When the user double-clicks the splitter, then the sidebar returns to its default width of 288 px.
- [x] AC-09 (event): When the splitter has keyboard focus and the user presses Arrow Left / Arrow Right, then the sidebar grows / shrinks by 16 px within the same limits (fine enough to tune, coarse enough to cross the range in about 20 presses); the splitter is a focusable element with the accessible role "separator", vertical orientation, and its current, minimum and maximum width in px as value, value minimum and value maximum.
- [x] AC-10 (event): When the user changes the sidebar width and reloads the app, then the sidebar opens at that width again in the same mode; the other mode keeps its own width.
- [x] AC-11 (error): If a stored sidebar width is missing or not a finite number, then the sidebar uses the default width; if it lies outside the limits for the current window — on load or because the window is resized — then the sidebar uses the nearest limit while the stored value stays unchanged until the user changes the width; no error is shown in either case.
- [x] AC-12 (event): When an effect opens in a window 640–1023 px wide, then the effect panel floats over the canvas area as in AC-01, and the drawer stays as it was (open or closed).
- [x] AC-13 (event): When an effect opens in a window narrower than 640 px, then the effect panel lies over the bottom of the window across its full width, covering the bottom sheet, and cannot be dragged; its content scrolls when it is taller than half the window, because the other half must keep showing the canvas for judging the preview.
- [x] AC-14 (event): When the window is resized across the 640 px boundary while an effect is open, then the effect stays open with its current values, and the panel switches between floating (AC-01) and docked to the bottom (AC-13); a floating panel that the resize would push out of the window moves back inside it (AC-19).
- [x] AC-16 (error): If the effect fails while previewing or on OK, then the panel stays open with its values, the error is shown where each mode shows errors today (status bar in Pixel mode, notice in Image mode), and the document is unchanged.
- [x] AC-17 (always): The preview is never part of the document: an autosave or a save while an effect is open writes the document without the preview, and an action that triggers the unsaved-changes prompt or replaces the document cancels the open effect first.
- [x] AC-18 (event): When the user drags the effect panel's title bar with mouse, pen or touch (a press on the close button does not start a drag), then the panel follows the pointer; when the user releases it, or the drag ends with `pointercancel` or outside the window, the panel stays where the drag last put it, and that position, measured from the top-right corner of the canvas area to the panel's top-right corner, is kept for that mode across later effects and reloads.
- [x] AC-19 (error): If the stored panel position is missing or not a pair of finite numbers, then the panel opens at its default place, 16 px inside the top-right corner of the canvas area; if a stored or dragged position would put any part of the panel outside the canvas area, then the panel is shown the shortest distance away that puts it wholly inside, and the stored position stays unchanged until the user moves the panel; no error is shown.
- [x] AC-20 (event): When the effect panel's title bar has keyboard focus and the user presses an arrow key, then the panel moves 16 px in that direction within the limits of AC-19, and the new position is kept as in AC-18.
- [x] AC-21 (always): The effect panel is a modal dialog for assistive technology (role `dialog`, `aria-modal="true"`, named after the effect); Tab and Shift+Tab move focus only among its controls, including the title bar, and wrap around at the ends.
- [x] AC-15 (option): Where the UI language is German, the panel's close button, the title bar's accessible name ("Move dialog" / "Dialog verschieben") and the splitter's accessible name are German.

## Technical notes

Hints, not contracts — paths drift; grep for the names (verified 2026-11-04).

- Floating box today: `ui/src/lib/components/EffectDialog.svelte` (`fixed inset-0 … justify-end`, `w-80`), mounted by `ImageEditor.svelte` (`effectOpen`) and `PixelEditor.svelte` (via `modes/pixel/image-ops.ts`). Its body (params, Preview, Reset / Cancel / OK, rAF-coalesced preview, Enter / Escape) moves into the panel.
- Frame: `ui/src/lib/components/layout/EditorFrame.svelte` (sidebar `w-72` = 288 px, drawer, bottom sheet with tabs), panels `PanelDef` and persisted `FrameLayout` in `layout/panels.svelte.ts` (`layout:<mode>` pref in IndexedDB via `lib/idb/prefs`; never `localStorage`, AGENTS.md §9). The width can join `FrameLayout`.
- Splitter drag: pointer capture as in `ui/src/lib/components/layers/LayersPanel.svelte` (`setPointerCapture`).
- Input while open: both editors set `ui.modalOpen` while `effectOpen` is set and skip shortcuts then; the canvas pointer path must still let zoom / pan through. "Inert" panels: the `inert` attribute on the other panels' container is one way.
- E2e specs that drive the effect box (`ui/e2e/effects.spec.ts`) look up `role="dialog"` with the effect title; the panel keeps an accessible region named after the effect.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Window narrower than 2 × 240 px at ≥ 1024 px | Not reachable; the 1024 px layout always leaves room for the 240 px minimum |
| Window shrunk so the stored width exceeds half the window | The sidebar uses half the window width; the stored value is kept for a wider window later (AC-11) |
| Splitter drag ends with `pointercancel` or the pointer is released outside the window | The width stays where the drag last put it and is stored |
| Effect without parameters (Invert Colors) | The panel shows "This effect has no parameters." and OK has focus |
| Splitter dragged while an effect is open | Not possible: the sidebar is inert while an effect is open (AC-03) |
| Effect panel dragged over the canvas spot the user wants to judge | The user drags it elsewhere; the preview underneath is unchanged |
| Pixel and Image mode both used | Each mode keeps its own panel position (AC-18) |
| Effect panel taller than the canvas area | Its controls scroll inside the panel; the title bar and the OK / Cancel row stay visible |
| Sidebar resized or window shrunk with the panel near the edge | The panel is shown inside the canvas area (AC-19); the stored position stays for a larger canvas area later |
| Second finger touches down while the title bar is dragged | The drag continues with the first pointer; the second pointer is ignored by the title bar |
| Document closed or replaced while an effect is open | The effect is cancelled and the panel disappears |
| Undo key pressed while an effect is open | Nothing happens (shortcuts stay blocked as today; Space is the one exception, for panning) |
| File menu, Undo / Redo or the panels toggle clicked while an effect is open | Nothing happens: they are dimmed and inert (AC-03); only the Effects and Adjustments menus answer |
| Save, close or open wanted while an effect is open | Web build: not reachable (File menu inert, shortcuts blocked); the timer-driven autosave still runs (AC-17). Desktop app: the native menu and the window's close button reach them, and AC-17 holds |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 240–560 px limits, 288 px default | 288 px is today's width; below 240 px sliders become unusable; above 560 px the canvas loses too much on a 1280 px screen | Low; constants |
| Other panels are inert while an effect is open | Switching the layer or changing the colour during a preview would change what the preview shows; paint.net's effect dialogs are modal for the same reason | Medium: a user may want to switch layers mid-preview |
| The effect panel is 280–360 px wide, as small as the controls allow | The floating box before 2026-11-04 was 320 px wide and its sliders fit; "small" was the owner's word | Low; a constant |
| The panel position is stored in the IndexedDB layout prefs of the mode, next to the sidebar width | The sidebar width already lives there (AC-10); `localStorage` is forbidden (AGENTS.md §9) | Low |
| The canvas area is not dimmed | The preview must be judged at its true colours; dimming only the rest shows which part is still live | Low |
| Zoom / pan stay available | Judging a preview needs a closer look; this is what the dialog keeps from the docked panel | Low |
| 16 px default margin and 16 px arrow-key step | The same step as the splitter's (AC-09): fine enough to place, coarse enough to cross the canvas area in a few dozen presses | Low; constants |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pixel: …` / `image: the effect panel is a small dialog over the canvas area (AC-01)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-02 | `the sidebar keeps its visibility, stored too, while an effect is open (AC-02)`, `tablet › the panel floats over the canvas area and the drawer stays as it was (AC-12, AC-02)`, `phone › the panel is docked to the bottom across the window, over the sheet (AC-13, AC-02)`, `phone › a closed bottom sheet stays closed (AC-02)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-03 | `pixel: …` / `image: other panels are inert and the canvas only zooms and pans (AC-03)` — `ui/e2e/effect-panel-input.spec.ts` (top bar included: File / Edit / View and undo / redo take no focus or click, Effects and Adjustments still open); `pixel: …` / `image: a two-finger pinch zooms while an effect is open and paints nothing (AC-03)` — `ui/e2e/effect-panel-touch.spec.ts` | verified | f9d85ce |
| AC-04 | `pixel: …` / `image: OK applies and Cancel, × and Escape leave the document alone (AC-04)`, `pixel: …` / `image: keys on the menu or a panel button do what that control does (AC-04, AC-05)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-05 | `pixel: …` / `image: choosing an effect while one is open restarts at the defaults (AC-05)` — `ui/e2e/effect-panel.spec.ts`; `pixel: …` / `image: keys on the menu or a panel button do what that control does (AC-04, AC-05)` — `ui/e2e/effect-panel.spec.ts` | verified | c2536b6 |
| AC-06 | `pixel: …` / `image: focus goes to the first control, or OK, and back to the canvas area (AC-06)` — `ui/e2e/effect-panel.spec.ts` | verified | c2536b6 |
| AC-07 | `dragging the splitter resizes the sidebar within its limits (AC-07)` — `ui/e2e/sidebar-splitter.spec.ts` | verified | 852277d |
| AC-08 | `a double-click on the splitter restores the default width (AC-08)` — `ui/e2e/sidebar-splitter.spec.ts` | verified | 852277d |
| AC-09 | `the splitter is a focusable separator the arrow keys move (AC-09)` — `ui/e2e/sidebar-splitter.spec.ts` | verified | 852277d |
| AC-10 | `the width survives a reload, per mode (AC-10)` — `ui/e2e/sidebar-splitter.spec.ts` | verified | 852277d |
| AC-11 | `a missing, broken or out-of-range stored width falls back silently (AC-11)` — `ui/e2e/sidebar-splitter.spec.ts` | verified | 852277d |
| AC-12 | `tablet › the panel floats over the canvas area and the drawer stays as it was (AC-12, AC-02)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-13 | `phone › the panel is docked to the bottom across the window, over the sheet (AC-13, AC-02)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-14 | `resizing across 640 px keeps the effect, its values and the panel inside (AC-14)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-15 | `close button, title bar and splitter are German (AC-15)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-16 | `pixel: …` / `image: a failing effect keeps the panel and its values (AC-16)` (OK) — `ui/e2e/effect-panel-input.spec.ts`; `pixel: a failing preview keeps the panel open with its values (AC-16)`, `image mode › a failing preview keeps the panel open with its values (AC-16)` — `ui/e2e/effect-panel-failure.spec.ts` | verified | a7bcb71 |
| AC-17 | `pixel: …` / `image: a save while an effect is open writes the document without the preview (AC-17)`; `pixel: …` / `image: the unsaved-changes prompt cancels the open effect first (AC-17)`; `pixel: opening a file while an effect is open cancels the effect (AC-17)` — `ui/e2e/effect-panel-desktop.spec.ts` (native menu); `pixel: …` / `image: an autosave while an effect is open stores the document without the preview (AC-17)` — `ui/e2e/effect-panel-input.spec.ts` | verified | a7bcb71 |
| AC-18 | `the title bar drags the panel; the position is kept per mode (AC-18)`, `a pen drag and a release outside the window keep the last position (AC-18)`, `touch › a touch drag moves the panel; a cancelled one stays where it got (AC-18)` — `ui/e2e/effect-panel-move.spec.ts` | verified | 5b32777 |
| AC-19 | `a broken or out-of-area stored position falls back silently (AC-19)` — `ui/e2e/effect-panel-move.spec.ts`; `resizing across 640 px keeps the effect, its values and the panel inside (AC-14)` — `ui/e2e/effect-panel.spec.ts` | verified | 5b32777 |
| AC-20 | `the arrow keys move the focused title bar by 16 px (AC-20)` — `ui/e2e/effect-panel-move.spec.ts` | verified | 5b32777 |
| AC-21 | `the panel is a modal dialog whose Tab order wraps (AC-21)` — `ui/e2e/effect-panel-move.spec.ts` | verified | 5b32777 |

## Summary

> Completed: 2026-11-06 (first completed 2026-11-04 as a docked panel; reopened by the 2026-11-05 revision)

- The effect UI is a small modal dialog (`ui/src/lib/components/EffectPanel.svelte`, 320 px wide) floating over the canvas area at window widths of 640 px and more, and docked to the bottom edge across the window below 640 px (at most half its height, scrolling); its values live in an `EffectSession`, so they survive a layout switch.
- The title bar drags the dialog with mouse, pen or touch and moves it in 16 px steps with the arrow keys; the position is kept per mode in the `layout:<mode>` pref and shown clamped into the canvas area; Tab and Shift+Tab wrap inside the dialog.
- While an effect is open everything outside the canvas area is dimmed and inert (the top bar except the Effects and Adjustments menus, tab bar, tool rail, sidebar / drawer / sheet, status bar and the panels under the canvas), and the canvas only zooms and pans (wheel, pinch, zoom control, Space and middle-button drag).
- The docked sidebar has a focusable splitter (240–560 px or half the window, default 288 px, 16 px arrow steps, double-click reset) whose width persists per mode and clamps silently.
- Failed previews and applies keep the dialog open with its values; the unsaved-changes prompt, tab switches and native Open cancel the effect first; saves and autosaves never contain the preview.

## Decisions

- 2026-11-05: Owner decision on AC-03 vs AC-05 (open question of the 2026-11-05 spec-build): option (c). While an effect is open the whole top bar is dimmed and inert too — the other menus, undo / redo, the panels toggle (still disabled) — except the Effects and Adjustments menus, which stay live so AC-05 keeps working. AC-03 reworded to say so (owner-approved reword); the edge-case table gained the top-bar and save / close / open rows.
- 2026-11-06 (spec-build): `MenuDef.liveWithEffect` marks the menus that stay live (set by `effectMenus`); `MenuBar` takes `restricted` while an effect is open and makes the other triggers `inert`. The header carries the shade (`data-dimmed`) and its non-menu parts are `inert`; the live triggers sit above the shade (`z-[46]`, the shade is 45) on an opaque `bg-pc-panel`, since a transparent trigger let the shade show through. On phones the ☰ button is lifted the same way, and each other menu's section in its sheet is `inert` and shaded. Arrow Left / Right in an open menu move only between the live menus.
- 2026-11-06 (spec-build): AC-17 does not need the web File menu: the autosave is timer-driven, and in the desktop app the native menu still saves, closes and opens while an effect is open. Its save, prompt and open tests therefore moved to `ui/e2e/effect-panel-desktop.spec.ts` and drive the native menu against the mocked Tauri bridge (`menu:save`, `menu:close`, `menu:open`); what they assert is unchanged. The open test opens an `.aseprite` file: the native Open routes by format, so a PNG would open as an Image-mode tab instead of a Pixel one. `effects.spec.ts` (effects-adjustments AC-03) read the document under the preview through Edit ▸ Copy, which is now inert; it compares native saves instead (see that spec's Decisions).
- 2026-11-06 (spec-build): Red run of the extended AC-03 tests: both modes failed at the File trigger's brightness (`file`, not dimmed) on the unchanged code; green after the change. The first draft also failed in Pixel mode for a test-path reason — undoing the added layer moved the active layer, so Invert previewed an empty layer; the test now reselects the filled layer.
- 2026-11-06 (spec-build): The 2026-11-05 note that AC-05's and AC-04's keyboard test drive the menu bar "because the top bar stays live for now" is superseded: they drive the Adjustments menu, which stays live under option (c).
- 2026-11-05 (spec-build): The frame hosts the panel: floating in the canvas area (`<main>`) at ≥ 640 px, 320 px wide (`w-80`), as high as its content up to the canvas area's height; below 640 px fixed to the window's bottom edge across its width, at most `50vh` tall. The frame passes `floating` and the title-bar wiring to `EffectPanel` through an `EffectHost` context (`layout/effect-host.ts`), as `PanelHost` does for panels.
- 2026-11-05 (spec-build): The position is `FrameLayout.effectPos` (`{x, y}`: the panel's top-right corner from the canvas area's top-right corner, `x` to the left, `y` down) in the `layout:<mode>` pref; `effectPanelPos()` (`layout/panels.svelte.ts`) clamps it to the measured canvas area and panel size. A drag or arrow key stores the clamped position it shows.
- 2026-11-05 (spec-build): The title bar is a `role="group"` named "Move dialog" around the heading and ×; the dialog keeps its name from the heading. It is made focusable and wired by a Svelte attachment (`mover`), for the same reason as the splitter: Svelte's a11y check flags `tabindex` and key handlers on a `group` in markup. A press on a button inside it starts no drag; one pointer drags at a time.
- 2026-11-05 (spec-build): Dimming is `inert` plus `data-dimmed` on each region outside the canvas area, which `app.css` shades with a 50 % black `::after`; each region got a positioned wrapper, and the sidebar's scrolling moved to an inner element so the shade covers the whole column. The panel sits at `z-index` 35: above the drawer and the bottom sheet (30), below the menu backdrop (40) and the menus (50), so a menu opened over the panel still takes the click.
- 2026-11-05 (spec-build): Tab and Shift+Tab are handled in the capture phase on the window while the panel is open (AC-21): within the panel they move to the next / previous visible stop and wrap; from outside it (the canvas area after a zoom click) they go to the first / last stop. A press inside the panel no longer moves focus to the canvas area.
- 2026-11-05 (spec-build): The sidebar, drawer and bottom sheet now show only their own state, so the 2026-11-04 sheet tab for the effect and the disabled sheet tabs are gone (AC-02, AC-13). The AC-03 e2e test arms the Eraser before the effect opens, since the tool rail is inert while it is open.
- 2026-11-05 (spec-build): The floating panel can overlap the canvas, and canvas screenshots include overlays (AGENTS.md Gotchas); `effect-params.spec.ts` masks the panel (`mask: [dialog]`) in its canvas screenshots, so a changed field in the panel does not count as a changed preview.
- 2026-11-05 (spec-build): Red runs: AC-01, AC-02, AC-04, AC-12 – AC-14 and AC-18 – AC-21 failed against the docked panel for the expected reasons (no title bar, panel in the sidebar, no dimming, `aria-modal="false"`). The first AC-15 red run stopped at the language menu (a test-path error); after fixing the path, its red run used a temporary break (the German title-bar string set to English), reverted afterwards.
- 2026-11-05 (spec-build): AC-05's and AC-04's keyboard test keep driving the menu bar while an effect is open; that works because the top bar stays live for now (see Open questions).
- 2026-11-05: Revision. The effect UI becomes a small dialog over the canvas that can be dragged by its title bar and is docked to the bottom on phones; while it is open the rest of the UI is dimmed, the canvas is not, and zoom / pan keep working; the splitter stays. Changed: AC-01 – AC-04, AC-12 – AC-15; new: AC-18 – AC-20. Requested by the owner because the inert tools beside a docked panel looked usable; the dialog plus dimming shows the modal state, and dragging answers the old complaint that the box covered the preview. Status back to `todo`.
- 2026-11-04: The owner chose "docked side panel" over a draggable / resizable floating window. Written without a further dialogue while the owner's goal "all planned specs implemented" was active; thresholds and the inert-panels rule are recorded as assumptions.
- 2026-11-04: Clarification pass settled without a dialogue: bottom-docked panels are inert too; a close button (×) is new; focus goes to the canvas area on close; re-choosing the same effect restarts it; stored widths outside the limits clamp without being overwritten; effect failures keep the panel open (AC-16); the preview never reaches the document (AC-17). The inert-panels rule stays an assumption the owner can revisit.
- 2026-11-04 (spec-build): The panel keeps `role="dialog"` (`aria-modal="false"`) named after the effect — a non-modal dialog docked in the sidebar — so assistive tech and the existing effect e2e specs find it by name as before.
- 2026-11-04 (spec-build): The open effect's values live in an `EffectSession` (`ui/src/lib/effects/session.svelte.ts`) the editor owns, not in the panel component: the frame re-mounts the panel when the window crosses a layout boundary, and AC-14 needs the values to survive that. A new session per opening (its `token` keys the panel) is what restarts a re-chosen effect at its defaults (AC-05).
- 2026-11-04 (spec-build): `EditorFrame` takes the panel as an `effectPanel` prop and only *shows* the sidebar / drawer / sheet for it (derived from its own state OR the open effect) instead of changing that state, so each layout returns to exactly what it had when the effect ends (AC-02, AC-12 – AC-14). To keep that state stable, the top-bar panels toggle is disabled while an effect is open, and in the bottom sheet the other tabs are disabled and the sheet's own close button is hidden — Cancel, × or OK end the effect.
- 2026-11-04 (spec-build): The sheet's effect tab comes first in the tab row (it is the "topmost" panel there too) and uses the existing `wand` icon; the panel's × is labelled with the shared `common.close` string ("Close" / "Schließen").
- 2026-11-04 (spec-build): On close, focus goes to the frame's `<main>` canvas area (`tabindex="-1"`), the one element both editors share.
- 2026-11-04 (spec-build): The splitter is a 4 px strip that replaces the sidebar's 1 px left border. Its input lives in a Svelte attachment (`ui/src/lib/components/layout/splitter.ts`): a focusable `role="separator"` is a widget in WAI-ARIA's window-splitter pattern, but Svelte's a11y check counts `separator` as non-interactive and flags `tabindex` / handlers on it in markup; the attachment avoids a suppression without dropping the role AC-09 asks for.
- 2026-11-04 (spec-build): The width limits follow `window.innerWidth` (tracked in `viewport.svelte.ts`); half the window is floored to whole px.
- 2026-11-04 (spec-build): Pixel mode had no middle-button pan; AC-03 lists it among the gestures that keep working, so Pixel mode now pans on a middle-button drag everywhere, as Image mode already did.
- 2026-11-04 (spec-build): While an effect is open the Pixel canvas starts only pinch, Space-drag and middle-button pans — the Move tool's pan fallback counts as tool input and is blocked; in Image mode the Hand and Zoom tools stay usable, since they move the view only (they are served before the guard in `MainCanvas.svelte`).
- 2026-11-04 (spec-build): `effect-params.spec.ts` (effect-catalog-v2 AC-11) checked that the Pencil paints while the dialog is open and picking is off. AC-03 here replaces that behaviour; the step now expects no stroke while the panel is open and a stroke after Cancel.
- 2026-11-04 (spec-build): The shell's `guard()` (`App.svelte`) — the one path of New, Open, Open Recent, open-file events and Paste as New Image, native menu included — cancels the open effect through `cancelActiveEffect()` (`lib/effects/session.svelte.ts`), which the mounted editor registers while an effect is open. Pixel mode's own `guardUnsaved` and `disposeDoc`, and Image mode's watch on the document handle, cover replacements inside an editor.
- 2026-11-04 (spec-build): Saves and autosaves never contained the preview (both cores preview without mutating the document); the AC-17 save and autosave tests were green at once after fixing their own setup and pin that behaviour.
- 2026-11-04 (spec-build): A failed run shows "{effect} failed: {error}" / "Preview failed: {error}" (existing `px.effectFailed` / `status.failed` strings); Image mode clears its failure notice on the next successful preview or apply. A failed OK leaves the preview as it was.
- 2026-11-04 (spec-build, rework round 1): Image mode previews even a locked layer, and every parameter vector the panel sends passes `EffectSpec::from_params` (it only checks name and count; values clamp), so no real input makes an Image-mode preview fail. The AC-16 preview test forces the failure at the core boundary instead: an init script wraps the wasm export `preview_effect` so it throws while a page flag is set. Pixel mode uses a real failure (a locked layer).
- 2026-11-04 (spec-build, rework round 1): The pinch and failing-preview tests pass on the existing code; their red runs came from temporary mutations, reverted afterwards: the effect guard moved above the pinch handling (Pixel `pointer.ts`, Image `MainCanvas.svelte`), and the panel closed on a failed preview (both modes).
- 2026-11-04 (spec-build, rework round 2): "Enter outside a multi-line field is OK" (AC-04) applies only where the focused control does not handle Enter itself — buttons, links, menu entries, tabs. Otherwise Enter on another effect's menu entry applied the open effect instead of switching (AC-05), and Enter on Cancel applied instead of cancelling. Enter still applies from fields, sliders, selects and checkboxes, and from the armed Pick button (effect-catalog-v2 AC-11). A key another handler already took (`defaultPrevented`, such as Escape closing an open menu) no longer also ends the effect.
- 2026-11-05 (integration with document-tabs): Opening a file no longer replaces the document but adds a tab, and only closing a tab or the app asks about unsaved changes. AC-17 therefore holds at the two places that remain: the shell's unsaved-changes prompt cancels the open effect before it shows (`ask` in `lib/shell/unsaved.svelte.ts`), and switching to another tab ends the effect of the tab being left (Pixel `tabs.svelte.ts` `show`, Image through its handle change). The AC-17 prompt test closes the tab instead of choosing New….
