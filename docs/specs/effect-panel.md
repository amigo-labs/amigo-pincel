---
status: todo
created: 2026-11-04
size: M
mode: both
design: [pincel.md §9.2, pincel.md §15 (2026-10-11 "One effect wire format", 2026-11-04 docked effect panel)]
---

# Docked effect panel and resizable sidebar

## Context

An effect or adjustment with parameters opens a floating box pinned to the top-right corner of the window, fixed at 320 px wide. It cannot be moved or resized, and it covers the canvas right where the user wants to judge the preview (owner: "the modal cannot be moved or resized — how can we do this better?"). The owner chose a docked side panel: the effect's controls live in the right sidebar, next to the canvas instead of over it, and the sidebar's width can be changed with a splitter.

Decision Log 2026-11-04 ("The effect UI becomes a docked panel") records this; it amends 2026-10-11 ("One effect wire format"), whose dialog is a floating panel. What the controls do — parameters, Preview, Reset, OK, Cancel, Enter, Escape — stays as `effects-adjustments` and `effect-catalog-v2` specify.

**Terms.** *Effect panel*: the panel this spec adds, showing one effect's name and controls. *Sidebar*: the right-hand column of docked panels (Colors, Layers, …) at window widths ≥ 1024 px; the *drawer* is the same column shown over the canvas at 640–1023 px; the *bottom sheet* is the phone layout below 640 px (pincel.md §9.2). *Open effect*: an effect whose panel is shown, between choosing its menu entry and OK or Cancel. *Splitter*: the draggable left edge of the docked sidebar.

## Scope

### In scope

- The effect panel in place of the floating effect box, in both modes and all three layouts
- A splitter that changes the docked sidebar's width, kept across reloads per mode
- Viewing the preview while an effect is open: zoom and pan stay available
- en + de strings for the new controls

### Out of scope

- Free-floating, draggable or detachable panels — because the owner chose docking, and one docked column keeps the layout code and the phone layout simple
- Resizing the drawer (640–1023 px) or the bottom sheet — because they overlay the canvas only while open and close again after use; on those screens every pixel of a wider overlay hides canvas, so a fixed width is the better default
- Faster previews for heavy effects — because moving effect work off the main thread is `performance-background-jobs`
- Several effects open at once, or effect layers — because an effect is applied destructively one at a time (non-destructive layer effects are in the backlog)
- Reordering panels in the sidebar — because the order is part of each mode's layout design (pincel.md §9.2), and a user-defined order needs its own persistence and drag interaction

## Acceptance criteria

- [ ] AC-01 (event): When the user chooses an Effects or Adjustments entry in a window at least 1024 px wide, then the effect panel appears as the topmost panel of the docked sidebar, titled with the effect's name, with the same controls, Preview checkbox and Reset / Cancel / OK buttons as before plus a close button (×) in its header; nothing of the effect UI overlaps the canvas area — in Pixel and in Image mode. (Topmost because it sits next to the menu bar the effect was chosen from, where the eye already is.)
- [ ] AC-02 (event): When the sidebar was hidden at the moment an effect opens, then the sidebar is shown for as long as the effect is open and hidden again after OK or Cancel; the stored sidebar visibility does not change.
- [ ] AC-03 (state): While an effect is open, every other panel — in the sidebar and docked under the canvas (timeline) — stays visible but inert: its controls cannot be focused, and a click on another layer in the Layers panel does not change the active layer; the canvas does not react to drawing tools, and shortcuts stay blocked as today except Space; zooming and panning the canvas (wheel, pinch, zoom control, dragging with Space held or with the middle button) still work, and the preview stays shown at the new view.
- [ ] AC-04 (event): When the user presses OK (or Enter outside a multi-line field), then the effect is applied as before, the effect panel disappears and the sidebar returns to the panels it showed before; when the user presses Cancel, the panel's close button or Escape, then the document is unchanged and the panel disappears the same way.
- [ ] AC-05 (event): When the user chooses an Effects or Adjustments entry while an effect is open — another one or the same one again — then the open effect is cancelled (no change to the document, its preview removed) and the panel shows the chosen effect at its defaults, with focus on its first control.
- [ ] AC-06 (event): When the effect panel opens, then keyboard focus moves to its first control, or to OK when the effect has no parameters (so Enter applies it at once, as in a confirmation dialog); when it closes, focus moves to the canvas area — the menu entry that opened it no longer exists.
- [ ] AC-07 (event): When the user drags the splitter, then the docked sidebar's width follows the pointer, limited to at least 240 px and at most 560 px or half the window width, whichever is smaller; the canvas area takes the remaining width.
- [ ] AC-08 (event): When the user double-clicks the splitter, then the sidebar returns to its default width of 288 px.
- [ ] AC-09 (event): When the splitter has keyboard focus and the user presses Arrow Left / Arrow Right, then the sidebar grows / shrinks by 16 px within the same limits (fine enough to tune, coarse enough to cross the range in about 20 presses); the splitter is a focusable element with the accessible role "separator", vertical orientation, and its current, minimum and maximum width in px as value, value minimum and value maximum.
- [ ] AC-10 (event): When the user changes the sidebar width and reloads the app, then the sidebar opens at that width again in the same mode; the other mode keeps its own width.
- [ ] AC-11 (error): If a stored sidebar width is missing or not a finite number, then the sidebar uses the default width; if it lies outside the limits for the current window — on load or because the window is resized — then the sidebar uses the nearest limit while the stored value stays unchanged until the user changes the width; no error is shown in either case.
- [ ] AC-12 (event): When an effect opens in a window 640–1023 px wide, then the drawer opens with the effect panel as its topmost panel, and closes again after OK or Cancel if it was closed before.
- [ ] AC-13 (event): When an effect opens in a window narrower than 640 px, then the bottom sheet opens with an extra tab named after the effect, selected, showing the effect panel; the tab disappears after OK or Cancel, and the sheet returns to the tab it showed before (or closes if it was closed).
- [ ] AC-14 (event): When the window is resized across a layout boundary while an effect is open, then the effect stays open with its current values and the panel moves to the new layout's sidebar, drawer or bottom sheet; when the effect then ends, the current layout's sidebar, drawer or sheet returns to the state it had before the effect opened (closed again if it was opened only for the effect).
- [ ] AC-16 (error): If the effect fails while previewing or on OK, then the panel stays open with its values, the error is shown where each mode shows errors today (status bar in Pixel mode, notice in Image mode), and the document is unchanged.
- [ ] AC-17 (always): The preview is never part of the document: an autosave or a save while an effect is open writes the document without the preview, and an action that triggers the unsaved-changes prompt or replaces the document cancels the open effect first.
- [ ] AC-15 (option): Where the UI language is German, the panel's close button, the splitter's accessible name and the effect sheet tab are German.

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
| Splitter dragged while an effect is open | Allowed; the preview stays |
| Document closed or replaced while an effect is open | The effect is cancelled and the panel disappears |
| Undo key pressed while an effect is open | Nothing happens (shortcuts stay blocked as today; Space is the one exception, for panning) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 240–560 px limits, 288 px default | 288 px is today's width; below 240 px sliders become unusable; above 560 px the canvas loses too much on a 1280 px screen | Low; constants |
| Other panels are inert while an effect is open | Switching the layer or changing the colour during a preview would change what the preview shows; paint.net's effect dialogs are modal for the same reason | Medium: a user may want to switch layers mid-preview |
| Zoom / pan stay available | Judging a preview needs a closer look; this is the gain over the modal box | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pixel: …` / `image: the effect panel is the topmost sidebar panel, clear of the canvas (AC-01)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-02 | `a hidden sidebar shows for the effect only; the stored visibility stays (AC-02)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-03 |      | open   |        |
| AC-04 | `pixel: …` / `image: OK applies and Cancel, × and Escape leave the document alone (AC-04)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-05 | `pixel: …` / `image: choosing an effect while one is open restarts at the defaults (AC-05)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-06 | `pixel: …` / `image: focus goes to the first control, or OK, and back to the canvas area (AC-06)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-07 | `dragging the splitter resizes the sidebar within its limits (AC-07)` — `ui/e2e/sidebar-splitter.spec.ts` | built |        |
| AC-08 | `a double-click on the splitter restores the default width (AC-08)` — `ui/e2e/sidebar-splitter.spec.ts` | built |        |
| AC-09 | `the splitter is a focusable separator the arrow keys move (AC-09)` — `ui/e2e/sidebar-splitter.spec.ts` | built |        |
| AC-10 | `the width survives a reload, per mode (AC-10)` — `ui/e2e/sidebar-splitter.spec.ts` | built |        |
| AC-11 | `a missing, broken or out-of-range stored width falls back silently (AC-11)` — `ui/e2e/sidebar-splitter.spec.ts` | built |        |
| AC-12 | `tablet › the drawer opens with the effect panel on top and closes again (AC-12)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-13 | `phone › the bottom sheet gets a selected tab for the effect (AC-13)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-14 | `resizing across layouts keeps the effect and its values (AC-14)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-15 | `German UI › close button, splitter and sheet tab are German (AC-15)` — `ui/e2e/effect-panel.spec.ts` | built |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |

## Decisions

- 2026-11-04: The owner chose "docked side panel" over a draggable / resizable floating window. Written without a further dialogue while the owner's goal "all planned specs implemented" was active; thresholds and the inert-panels rule are recorded as assumptions.
- 2026-11-04: Clarification pass settled without a dialogue: bottom-docked panels are inert too; a close button (×) is new; focus goes to the canvas area on close; re-choosing the same effect restarts it; stored widths outside the limits clamp without being overwritten; effect failures keep the panel open (AC-16); the preview never reaches the document (AC-17). The inert-panels rule stays an assumption the owner can revisit.
- 2026-11-04 (spec-build): The panel keeps `role="dialog"` (`aria-modal="false"`) named after the effect — a non-modal dialog docked in the sidebar — so assistive tech and the existing effect e2e specs find it by name as before.
- 2026-11-04 (spec-build): The open effect's values live in an `EffectSession` (`ui/src/lib/effects/session.svelte.ts`) the editor owns, not in the panel component: the frame re-mounts the panel when the window crosses a layout boundary, and AC-14 needs the values to survive that. A new session per opening (its `token` keys the panel) is what restarts a re-chosen effect at its defaults (AC-05).
- 2026-11-04 (spec-build): `EditorFrame` takes the panel as an `effectPanel` prop and only *shows* the sidebar / drawer / sheet for it (derived from its own state OR the open effect) instead of changing that state, so each layout returns to exactly what it had when the effect ends (AC-02, AC-12 – AC-14). To keep that state stable, the top-bar panels toggle is disabled while an effect is open, and in the bottom sheet the other tabs are disabled and the sheet's own close button is hidden — Cancel, × or OK end the effect.
- 2026-11-04 (spec-build): The sheet's effect tab comes first in the tab row (it is the "topmost" panel there too) and uses the existing `wand` icon; the panel's × is labelled with the shared `common.close` string ("Close" / "Schließen").
- 2026-11-04 (spec-build): On close, focus goes to the frame's `<main>` canvas area (`tabindex="-1"`), the one element both editors share.
- 2026-11-04 (spec-build): The splitter is a 4 px strip that replaces the sidebar's 1 px left border. Its input lives in a Svelte attachment (`ui/src/lib/components/layout/splitter.ts`): a focusable `role="separator"` is a widget in WAI-ARIA's window-splitter pattern, but Svelte's a11y check counts `separator` as non-interactive and flags `tabindex` / handlers on it in markup; the attachment avoids a suppression without dropping the role AC-09 asks for.
- 2026-11-04 (spec-build): The width limits follow `window.innerWidth` (tracked in `viewport.svelte.ts`); half the window is floored to whole px.
