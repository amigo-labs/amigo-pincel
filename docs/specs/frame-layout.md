---
status: todo
created: 2026-11-25
size: L
mode: both
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §15 (2026-10-11 "One editor frame", 2026-11-04 "Tabs mix both modes", 2026-11-18 "macOS shows the native menu bar only", 2026-11-25 "Frame layout")]
---

# Frame layout

## Context

The editor frame grew region by region. Four things make it harder to use than it has to be (owner request 2026-11-25, "UI refresh: ease of use, logical arrangement, familiar patterns"):

- The document name, its unsaved marker and the mode are shown twice: in the top bar and in the active tab.
- The tab bar sits below the tool options bar, so the options of a tool stand between a document and its tab. Editors with tabs (browsers, Photoshop, paint.net) put the tabs above everything that belongs to the document.
- Zoom floats over the canvas corner. A click on the zoom value fits the image to the window, which nothing on screen says. Undo and Redo sit at the right end of the top bar beside the title.
- The status bar differs per mode: Pixel mode shows size, frame, undo depth, a message, the render backend and the app version; Image mode shows the tool name, the pointer position, the size, a fixed "sRGB" and a notice.

The owner approved a mock-up on 2026-11-25 that orders the frame from general to specific, top to bottom: app, document, tool, canvas, status. This spec builds the frame part of it. It is the first spec of the UI refresh; `sidebar-tab-groups` and `panel-visibility` (side panels) follow, later specs cover the tool rail, the options bar's content, dialogs and notices (Decision Log 2026-11-25).

Terms:

- **App bar**: the top row of the editor, called "top bar" in older specs. In the macOS desktop app on a medium or wide window the **top row** of AC-14 stands in for app bar and tab bar; statements about "the app bar" do not cover the tabs inside that row.
- **Tab bar**: the row of document tabs of `document-tabs`.
- **Options bar**: the row with the active tool's options.
- **Status bar**: the bottom row of the editor.
- **Zoom control**: one group of three controls — Zoom Out, the zoom value, Zoom In.
- **Wide** window: at least 1024 px wide. **Medium**: 640–1023 px. **Narrow**: below 640 px. These are the breakpoints of `pincel.md` §9.2 and do not change.
- **macOS desktop app**: the Tauri build on macOS, where the operating system draws the menus and the editor draws none (`macos-native-menu`). **Other platforms**: the browser and the desktop app on Windows and Linux.
- **Notice**: the one-line message an editor shows after an action or a failure: Pixel mode's status text; in Image mode the load error if there is one, else the notice. When a notice appears and when it ends does not change with this spec.
- **Active tool**: the tool selected in the tool rail (or tool strip), not a tool held for a moment with a modifier key.
- **Command palette**: the searchable command list of `power-user-shortcuts-palette`, opened with Ctrl+K (⌘K on macOS).
- **Tour hint**: the one-time hint that offers the first-run tour (`first-run-tutorial`); it shows in the app bar's free space and keeps that place.
- **Frame-time probe**: Pixel mode's diagnostic readout of frame rate and compose time, switched with Help ▸ Frame-Time Probe (F2, `menu-structure` AC-05).
- **Agent indicator**: the mark the status bar shows while an agent works in the editor (`mcp-live-desktop`); this spec only gives it a place.
- **Panel sheet**: the bottom sheet that holds the panels on a narrow window. **Tool strip**: the horizontal row of tools at the bottom of a narrow window (`app-shell` AC-17).
- **Effect open**: an effect or adjustment panel is open over the canvas (`effect-panel`). **Answer running**: the Assistant is working on an answer (`ai-chat-panel`). In both states parts of the frame are dimmed and inert today.
- **Zoom factor**: the zoom as a multiple of actual size. Pixel mode allows the whole factors 1 to 64 and writes them `8×`; Image mode allows 0.02 to 64 and writes them in percent (`800 %`).
- **Mock-up**: `docs/design/ui-refresh-mockup.html`, the page the owner approved on 2026-11-25.

## Scope

### In scope

- The order of the frame's regions and which of them span the whole window
- The app bar's content: no document name, unsaved marker or mode badge; Undo and Redo behind the menus; a command search control
- A New button at the end of the tabs; the unsaved tooltip moves to the tab
- One status bar layout for both modes; the pointer position in Pixel mode
- The zoom control: its place, its list of zoom steps
- The app version and the render backend leave Pixel mode's status bar; the About dialog shows them
- The macOS desktop app's single row of app bar and tabs
- Bar heights
- Strings for the new controls in every shipped UI language (en, de, es, fr, ja, pt-BR)
- Documentation, no AC: `docs/specs/pincel.md` §9.2 (sketch and text) describes the new frame
- This spec supersedes the wording of four earlier ACs where they name the old places: `document-tabs` AC-01 ("between the options bar and the canvas") and AC-04 ("top-bar name, mode badge"), `app-shell` AC-16 (name, marker and mode badge in the top bar) and `macos-native-menu` AC-01 (mode badge and document title in the top bar). Their other statements hold; their tests are adapted to the new places.

### Out of scope

- The side panels — because `sidebar-tab-groups` and `panel-visibility` cover them.
- The content and order of each tool's options, and a "More" overflow — because that is the later spec `options-bar-layout`; this spec only moves the bar and sets its height.
- A colour well in the tool rail and changes to the tool groups — because that is the later spec `tool-rail-colors`.
- A toast area, and notices, pointer position, size and frame on narrow windows — because that is the later spec `notifications`. Narrow windows keep having no status bar and show none of these, as before this spec.
- The menus, their order and the keyboard map — because `menu-structure` and `pincel.md` §9.4 settle them and the refresh keeps them.
- The breakpoints and what the tool rail and the panels become at each — because `app-shell` AC-17 settles it. On a narrow window the timeline stays a tab of the panel sheet.
- Each mode's zoom range and the format of its zoom value — because this spec only moves the control.
- What Toggle UI hides besides the regions AC-13 names, and what an open effect or a running answer make inert besides what AC-12 names — because those rules stay as `app-shell`, `effect-panel` and `ai-chat-panel` set them.
- Whether Ctrl+K works while an effect is open — because this spec changes no shortcut.
- The window title — because it already names the active document and stays.

## Acceptance criteria

- [ ] AC-01 (state): While the window is medium or wide, the editor shows these regions from top to bottom: the app bar, the tab bar, a middle row, the status bar (macOS desktop app: the single top row of AC-14 in place of app bar and tab bar). The app bar, the tab bar and the status bar each span the full window width. The middle row holds, from left to right, the tool rail, a centre column and, while the window is wide and the sidebar is shown, the sidebar; the centre column holds, from top to bottom, the options bar, the canvas and (Pixel mode) the timeline. On a medium window the panels stay a drawer over the canvas.
- [ ] AC-02 (state): While the window is medium or wide on the other platforms, the app bar shows, from left to right: the "Pincel" wordmark (wide windows only, because a medium window needs the room for the menus, as before), the menu bar, Undo, Redo, free space (it holds the tour hint while that shows), the command search control, the panels toggle. Outside the tabs, no bar of the editor shows a document name, an unsaved marker or a mode badge, at any window width on any platform.
- [ ] AC-03 (always): Each tab's tooltip is the document name, or "<name> — unsaved changes" while the document has unsaved changes; the tab keeps its mode icon with the mode in its accessible name and its unsaved marker (`document-tabs` AC-01).
- [ ] AC-04 (event): When the user activates the command search control, then the command palette opens as it does for its shortcut. While the window is wide the control shows the text "Search commands" and the palette's shortcut in the platform's notation; while the window is medium or narrow it is an icon button with the accessible name "Search commands" and the shortcut in its tooltip.
- [ ] AC-05 (event): When the user activates the New button, then the New-document dialog opens as it does for File ▸ New…. The button has the accessible name "New document" and a tooltip with that name and the shortcut of File ▸ New…. While the tabs fit their bar the button follows the last tab; while they do not fit, the tabs scroll and the button stays fully visible at the right end of the tabs without scrolling (checked at 390 px and at 1280 px, each with as many tabs as it takes to overflow).
- [ ] AC-06 (state): While the window is medium or wide and the document is loaded, the status bar of both modes shows, from left to right: a text region; the frame-time probe readout while the probe is on (Pixel mode only); the agent indicator while an agent works (either mode); a place for the pointer position; the document size (Pixel mode: "W × H px"; Image mode: in the unit chosen under View ▸ Units, as its status bar shows it before this spec); in Pixel mode the frame as "Frame n/m"; the zoom control. The status bar shows no undo or redo depth, no render backend, no app version and no colour profile. Until the document is loaded it shows only the text region (empty, or with the load error) and a disabled zoom control.
- [ ] AC-07 (state): While the editor has a notice, the status bar's text region shows it; while it has none, the region shows the name of the active tool. The region's tooltip is its full text. The items to its right keep their full width at every window width from 640 px; the text region takes the rest, cuts a longer text with an ellipsis and may shrink to nothing. If those items do not fit even then (a long size in a physical unit, a high frame count), the status bar leaves out the pointer position first and the probe readout next; the document size, the frame and the zoom control always show in full.
- [ ] AC-08 (state): While the pointer (mouse, pen, or a touch in contact) is over the canvas itself, with no panel, drawer, sheet, dialog or menu under it, the status bar shows its position. In Pixel mode that is the column and row of the document pixel under the pointer, counted from 0 at the top left ("x 5, y 3"), shown while the pointer is on a point inside the document — also during a drag — and not beside it. In Image mode it is what `image-view-aids` AC-10 defines (the image pixel under the pointer in the unit chosen under View ▸ Units, also beside the image while the pointer is over the canvas area). Otherwise the place is empty, and the size, the frame and the zoom control stay where they are when the position appears or disappears.
- [ ] AC-09 (always): The zoom control consists of Zoom Out, a button showing the current zoom in the mode's format, and Zoom In. Zoom Out and Zoom In change the zoom as View ▸ Zoom Out and View ▸ Zoom In do, each is disabled at its end of the mode's zoom range, and their tooltips name their shortcuts. The control has no separate 1:1 button (the zoom menu's actual-size step replaces it), and a click on the zoom value does not change the zoom.
- [ ] AC-10 (event): When the user activates the zoom value, then a menu opens with Fit to Window (with its shortcut hint), a separator, and every zoom factor of 0.25, 0.5, 1, 2, 4, 8, 16, 32 and 64 that the mode's zoom range includes (both ends count), written in the mode's format: Pixel mode lists 1× to 64×, Image mode 25 % to 6400 %. The factor-1 entry carries the shortcut hint of View ▸ Actual Size. An entry carries a check mark exactly while the zoom factor equals it. Choosing Fit to Window does what the View menu entry does; choosing a factor sets that zoom and keeps the document point at the centre of the canvas area in place; either choice closes the menu and returns the focus to the zoom value. Escape or a click outside closes the menu without a change, as for the menu bar's menus; Escape returns the focus to the zoom value. While the menu is open and the zoom changes by wheel or shortcut, the menu stays open and the check mark follows the zoom. The menu also closes without a change when the active tab changes, an effect opens, or the window changes between narrow and medium.
- [ ] AC-11 (state): While the window is medium or wide, the zoom control is the last item of the status bar and no zoom control lies over the canvas. While the window is narrow, the zoom control floats over the bottom right corner of the canvas, and while the panel sheet is open it stays fully visible and can be operated (the sheet does not cover it).
- [ ] AC-12 (state): While an effect is open or an answer is running, the zoom control stays operable and undimmed in its place (the canvas keeps zoom and pan in both states), and the New button and the command search control are inert and look as the File menu's title does in that state — because the first runs a File command and the second runs any command, and both states block those. While an effect is open, the rest of the status bar and the tab bar are dimmed and inert too, as the app bar is before this spec.
- [ ] AC-13 (state): While the UI is hidden with View ▸ Toggle UI, the app bar, the tab bar, the status bar and the zoom control stay; the tool rail, the options bar and the panels are hidden, as before this spec.
- [ ] AC-14 (state): While the editor runs as the macOS desktop app and the window is medium or wide, one top row of 40 px takes the place of app bar and tab bar: the wordmark (wide windows only), the tabs followed by the New button, the tour hint's place, Undo, Redo, the command search control and the panels toggle. The tabs take the room the other items leave and scroll when they do not fit; the New button and the controls to the right of the tabs stay fully visible. The regions below that row are those of AC-01. While the window is narrow, the layout is that of AC-15 without ☰.
- [ ] AC-15 (state): While the window is narrow on the other platforms, the editor shows from top to bottom: the app bar with ☰, Undo, Redo, free space, the command search control and the panels toggle; the tab bar; the options bar; the canvas; the tool strip. There is no status bar.
- [ ] AC-16 (state): While the window is medium or wide, the app bar (macOS desktop app: the top row of AC-14) is 40 px high, the tab bar 36 px, the options bar 40 px and the status bar 28 px (CSS pixels, each within 1 px). While the window is narrow, the app bar and the options bar are 44 px high and the tab bar 36 px.
- [ ] AC-17 (always): The About dialog shows the app version. While a Pixel-mode tab is active and its editor has chosen a render backend, the dialog also shows that backend with the text the status bar showed before this spec (e.g. "webgpu"); otherwise it shows no backend.
- [ ] AC-18 (error): If the menus, Undo, Redo, the command search control and the panels toggle do not fit the app bar (a medium window, a language with long menu names), then Undo, Redo, the command search control and the panels toggle stay fully visible and every menu can still be opened with the mouse and the keyboard, the menu bar scrolling as before this spec (`app-shell` AC-17). Checked at 640 px in each of the six shipped languages.
- [ ] AC-19 (always): While they are enabled and not inert, the New button, the command search control and the three parts of the zoom control can be reached with the Tab key, show the focus ring and are pressed by Space; Enter presses them in Image mode, and in Pixel mode Enter stays Play / Pause as for every focused button there. In the open zoom menu, in both modes, the arrow keys move between the entries and Enter chooses the focused one.
- [ ] AC-20 (option): Where the UI language is German, the command search control reads "Befehle suchen", the New button is named "Neues Dokument" and the zoom menu's first entry reads "An Fenster anpassen" (as in the View menu). Each of the six language dictionaries defines every string this spec adds to the English dictionary.
- [ ] AC-21 (state, manual: visual design): While the editor shows a document at 1280, 768 and 390 px in the dark and the light theme, in both modes, the frame's regions stand in the order and alignment the mock-up shows with its switch set to "Entwurf" (the new design; "Heute" is the state before this spec), leaving aside what the mock-up shows of later specs: tool name in the options bar, colour well, tab groups, toast; the content of every bar is centred vertically, and no text or control is clipped other than by the ellipsis of AC-07 and the scrolling of tabs and menu bar.
- [ ] AC-22 (state, manual: real Mac): While the macOS desktop app shows an editor tab, the single top row of AC-14 does not overlap the window's own controls and its tabs, New button and right-hand controls can be clicked. Checked together with `macos-native-menu` AC-16.

## Technical notes

- Frame: `ui/src/lib/components/layout/EditorFrame.svelte` (region order, heights), `TabBar.svelte` (New button, tooltip, height), `ZoomControl.svelte` (today rendered by each editor inside its own stage: `modes/pixel/PixelEditor.svelte`, `modes/image/components/canvas/MainCanvas.svelte`; the frame needs to place it, in the status bar or over the canvas). The menu for the zoom steps can reuse the shared menu classes (`am-menu`) and the focus handling of `layout/menu-focus.ts`.
- Status bar content is a snippet each editor passes today (`statusbar`); one shared layout means the editors pass data (notice, tool name, position, size, frame) or the frame provides the slots in a fixed order.
- Command search: the palette already exists (`power-user-shortcuts-palette`, opened by the shell on Ctrl+K); the control only opens it. Editors reach the shell through `EditorProps` (`ui/src/lib/shell/types.ts`).
- Tests: about 55 files under `ui/e2e/` address the app bar's document name with `getByTitle(name, { exact: true })` (AGENTS.md, Gotchas "The tab bar repeats each document name"). With the name gone from the app bar they read the active tab instead; a helper in `ui/e2e/helpers.ts` keeps that in one place. That Gotchas entry is rewritten in the same change. Specs that read the backend or version from the status bar (`ui/e2e/renderer.spec.ts`) read it from the About dialog.
- The first-run tour anchors (`data-tour`, e.g. `quick` on Undo / Redo) keep their names; the tour hint keeps its place in the app bar's free space (macOS: between the tabs and Undo, AC-14).
- Zoom ranges: `ui/src/modes/pixel/view-ops.ts` (`MIN_ZOOM` 1, `MAX_ZOOM` 64), `ui/src/modes/image/stores/editor.svelte.ts` (0.02, 64).
- macOS: the tests use the mocked Tauri bridge with a Mac user agent (`installTauriMock`, `ui/e2e/macos-native-menu.spec.ts`).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 12 tabs in a 390 px window | The tabs scroll; the New button stays visible (AC-05) |
| The zoom equals none of the listed factors (Image mode after Fit to Window: 137 %) | No entry has a check mark; the zoom value shows the real zoom (AC-10) |
| Pixel mode, whose zoom range starts at 1× | The menu lists 1× to 64×, no smaller factor (AC-10) |
| The zoom changes by wheel or shortcut while the zoom menu is open | The menu stays open and the check mark follows the zoom (AC-10) |
| The window is resized across 640 px while the zoom menu is open | The menu closes (AC-10); the zoom control shows at its place for the new width (AC-11) |
| A notice longer than the text region | Cut with an ellipsis, full text in the tooltip, right-hand items stay (AC-07) |
| A 640 px window with probe readout, agent indicator, position, size, frame and zoom all showing | They keep their width; the text region may be empty (AC-07) |
| A drag starts on the canvas and leaves the document | The position disappears while the pointer is outside the document and returns when it comes back (AC-08) |
| The pointer moves from the canvas onto the status bar | The position disappears without moving the size, frame and zoom items (AC-08) |
| An effect is open and the user changes the zoom from the status bar | The zoom changes; the effect panel and its preview stay (AC-12) |
| macOS desktop app with 12 tabs in a 1024 px window | The tabs scroll inside the top row; Undo, Redo, search and panels toggle stay visible (AC-14) |
| The About dialog is opened from the start screen or with an Image-mode tab active | It shows the version and no render backend (AC-17) |
| An Assistant answer runs | The zoom control works; the New button and the command search control are inert (AC-12) |
| Several Pixel tabs are open | They share one editor and so one render backend; the About dialog shows it (AC-17) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The text region shows the active tool's name while there is no notice | Image mode's status bar names the tool today, and the options bar will only name it once `options-bar-layout` is built | Low: one line of text |
| The zoom steps are the powers of two from 0.25 to 64, cut to the mode's range | Photoshop and Figma offer a short fixed list beside Fit; powers of two are whole factors in Pixel mode and reach both modes' upper limit of 64 | Low: the list is data |
| The 1:1 button goes | The actual-size entry of the zoom menu and View ▸ Actual Size with its shortcut do the same; the status bar has no room for a fourth zoom button at 640 px | Low |
| Bar heights 40 / 36 / 40 / 28 px, 44 px for app bar and options bar on narrow windows | The frame's controls are 24 px high (`amigo-theme-controls`) and the shared spacing scale steps in 4 px; today's 44 / 44 / 32 / 28 gives the canvas less room and makes the tab bar the thinnest row. Narrow windows keep 44 px for touch | Low: AC-21 checks the look |
| The menu's focus goes back to the zoom value after a choice or Escape | The menu bar's menus return the focus to their trigger | Low |
| Adapting about 55 e2e files fits this spec | Each file changes one locator to a shared helper; no test logic changes | Medium: a long but mechanical commit |
| The version and the render backend go to the About dialog rather than away | Both are diagnostic information tests and bug reports use; an About dialog is where users look for a version | Low |
| Undo / redo depth needs no replacement | The History panel (`history-panel`) lists every step in both modes, and Undo / Redo are disabled when there is nothing to undo or redo | Low |
| Image mode's fixed "sRGB" is dropped | It is a constant text, not a state of the document | Low |
| On macOS the tabs share the row with the app bar's controls | Without menus the app bar would be almost empty; one row less gives the canvas the space | Medium: needs a look on a real Mac, with `macos-native-menu` AC-16 |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/frame-layout.spec.ts`: "{pixel,image}, wide › app bar, tab bar, middle row and status bar stack in that order; the bars span the window (AC-01, AC-16)" | built |        |
| AC-02 | `ui/e2e/frame-layout.spec.ts`: "{pixel,image}, wide › the app bar holds wordmark, menus, Undo, Redo, search and panels toggle; no name, marker or mode badge (AC-02, AC-03)", "medium › no wordmark; the search control is an icon button with its shortcut in the tooltip (AC-02, AC-04)"; `ui/e2e/shell.spec.ts`: "AC-16: {pixel,image}: the tab shows mode, name and an unsaved marker until saved" | built |        |
| AC-03 | `ui/e2e/frame-layout.spec.ts`: "wide › an unsaved document shows in its tab tooltip (AC-03)", "… no name, marker or mode badge (AC-02, AC-03)"; `ui/e2e/shell.spec.ts`: "AC-16: with several tabs the active tab shows its mode, name and marker" | built |        |
| AC-04 | `ui/e2e/frame-layout.spec.ts`: "wide › the search control opens the command palette (AC-04, AC-19)", "medium › no wordmark; the search control is an icon button with its shortcut in the tooltip (AC-02, AC-04)", "narrow › app bar, tab bar, options bar, canvas, tool strip; no status bar (AC-15, AC-16)" | built |        |
| AC-05 | `ui/e2e/frame-layout.spec.ts`: "wide › the New button opens the New-document dialog and names its shortcut (AC-05, AC-19)", "wide › the New button stays visible when the tabs overflow (AC-05)", "narrow › the New button stays visible when the tabs overflow (AC-05)" | built |        |
| AC-06 | `ui/e2e/frame-layout-status.spec.ts`: "{pixel,image}, wide › text, size, frame and zoom in one order; the position comes and goes without moving them (AC-06, AC-07, AC-08, AC-11)"; the not-loaded state has no test (Decisions) | built |        |
| AC-07 | `ui/e2e/frame-layout-status.spec.ts`: "status bar drop rule (AC-07)" (three tests of `statusDrop`), "640 px › the items right of the text region stay fully visible (AC-07)", "{pixel,image}, wide › text, size, frame and zoom in one order; … (AC-06, AC-07, AC-08, AC-11)" (tool name in Image mode, tooltip) | built |        |
| AC-08 | `ui/e2e/frame-layout-status.spec.ts`: "{pixel,image}, wide › text, size, frame and zoom in one order; the position comes and goes without moving them (AC-06, AC-07, AC-08, AC-11)"; Image mode beside the image: `ui/e2e/image-view-aids-pointer.spec.ts` (unchanged) | built |        |
| AC-09 | `ui/e2e/frame-layout-status.spec.ts`: "{pixel,image}, wide › the zoom value opens a menu of Fit to Window and the steps of the mode (AC-09, AC-10)", "{pixel,image}, wide › keyboard: Space opens, arrows move, Enter chooses; the limits disable (AC-09, AC-19)" | built |        |
| AC-10 | `ui/e2e/frame-layout-status.spec.ts`: "… the zoom value opens a menu of Fit to Window and the steps of the mode (AC-09, AC-10)", "wide › image: a step from the menu keeps the centre of the canvas area in place (AC-10)", "wide › pixel: a step from the menu keeps the centre of the canvas area in place (AC-10)", "wide › the zoom menu closes when the tab changes or the window becomes narrow (AC-10)" | built |        |
| AC-11 | `ui/e2e/frame-layout-status.spec.ts`: "{pixel,image}, wide › text, size, frame and zoom in one order; … (AC-06, AC-07, AC-08, AC-11)", "narrow › the zoom control floats over the canvas corner and stays clear of the panel sheet (AC-11)" | built |        |
| AC-12 | `ui/e2e/frame-layout-status.spec.ts`: "wide › while an effect is open the zoom control works; the rest of the status bar, the tabs, New and search are inert (AC-12)"; a running answer: `ui/e2e/assistant-chat.spec.ts` (zoom while an answer runs, unchanged) | built |        |
| AC-13 | `ui/e2e/frame-layout.spec.ts`: "wide › Toggle UI keeps app bar, tab bar and status bar (AC-13)" | built |        |
| AC-14 | `ui/e2e/frame-layout.spec.ts`: "macOS desktop app › one top row holds the tabs and the app bar controls (AC-14, AC-16)", "macOS desktop app › many tabs scroll inside the top row; the controls stay visible (AC-14)", "macOS desktop app › narrow: app bar without ☰, then the tab bar (AC-14)" | built |        |
| AC-15 | `ui/e2e/frame-layout.spec.ts`: "narrow › app bar, tab bar, options bar, canvas, tool strip; no status bar (AC-15, AC-16)" | built |        |
| AC-16 | `ui/e2e/frame-layout.spec.ts`: "{pixel,image}, wide › app bar, tab bar, middle row and status bar stack in that order; … (AC-01, AC-16)", "narrow › app bar, tab bar, options bar, canvas, tool strip; no status bar (AC-15, AC-16)", "macOS desktop app › one top row holds the tabs and the app bar controls (AC-14, AC-16)" | built |        |
| AC-17 | `ui/e2e/frame-layout-status.spec.ts`: "wide › the About dialog shows the version, and the render backend for a Pixel tab only (AC-17)"; `ui/e2e/renderer.spec.ts` (reads the backend from the About dialog) | built |        |
| AC-18 | `ui/e2e/frame-layout.spec.ts`: "640 px › {English, Deutsch, Español, Français, 日本語, Português (Brasil)}: Undo, Redo, search and panels toggle stay visible; the last menu opens (AC-18)" | built |        |
| AC-19 | `ui/e2e/frame-layout.spec.ts`: "wide › the search control opens the command palette (AC-04, AC-19)", "wide › the New button opens the New-document dialog and names its shortcut (AC-05, AC-19)"; `ui/e2e/frame-layout-status.spec.ts`: "… keyboard: Space opens, arrows move, Enter chooses; the limits disable (AC-09, AC-19)", "wide › pixel: Enter chooses the focused zoom entry and starts no playback (AC-19)" | built |        |
| AC-20 | `ui/e2e/frame-layout.spec.ts`: "wide › German names the new controls (AC-20)"; `ui/e2e/frame-layout-status.spec.ts`: "wide › German: the zoom menu starts with \"An Fenster anpassen\" (AC-20)"; every dictionary is typed against `en.ts` (`pnpm check`) | built |        |
| AC-21 |      | open   |        |
| AC-22 |      | open   |        |

## Decisions

- 2026-11-25: The owner approved the mock-up of the UI refresh (app bar, tabs above the options bar, one status bar with the zoom control). The frame-level decisions are in the Decision Log (`pincel.md` §15, 2026-11-25).
- 2026-11-25 (build, revision): AC-08 reworded for Image mode. As first written it said both "inside the document only" and "as before this spec"; Image mode shows the position also beside the image while the pointer is over the canvas area (`image-view-aids` AC-10, with its own tests). Image mode keeps that; "inside the document only" holds for Pixel mode, which had no position before.
- 2026-11-25 (build): AC-06's last sentence (the bar before the document is loaded) has no test. The shell shows an editor only once its wasm module is there, and the document follows within the same start-up; a test cannot hold that state open (a held-back wasm fetch keeps the whole editor hidden). The frame draws the state from `status.ready`.
- 2026-11-25 (build): The three `statusDrop` tests were green at once: the function is new, so there was no earlier state to fail against; the other tests of both spec files were red against the unchanged app (block 1: 22 of 22; block 2: 16 of 16, run against block 1's build).
- 2026-11-25 (build): The English frame readout is now "Frame 1/4" as AC-06 writes it (it was "frame 1/4"); the timeline keeps its own lower-case readout.
- 2026-11-25 (build): Image mode writes zoom steps as it writes the zoom value ("6400%", no thousands separator); AC-10's "25 % to 6400 %" names the range, not the spelling.
- 2026-11-25 (build): In Pixel mode the editor's status text is never empty after start-up ("ready", then the last action), so the tool name shows there only while that text is empty; Image mode shows the tool name whenever it has no notice.
- 2026-11-25 (build): The zoom menu opens upwards from the value, in the status bar and over the canvas alike. A menu stepped to with the arrow keys scrolls its title into view in a scrolling menu bar (AC-18).
- 2026-11-25 (build): Test helpers changed with the frame: `switchLanguage` now finds the Settings dialog in all six languages (it knew English and German and passed for the others only by timing); `zoomReadout`, `zoomFit` and `zoomActual` in `ui/e2e/helpers.ts` replace the old Fit / 1:1 buttons; `activeTab` in `ui/e2e/tab-helpers.ts` replaces reading name and mode from the top bar.
- 2026-11-25 (build): New public API, for review: `EditorFrame` props `status` (`StatusInfo`), `statusExtra`, `zoom` (`ZoomProps`), `onSearch`, `searchHint` replace `title`, `dirty` and the `statusbar` snippet; `TabBarProps.onNew` / `newHint`; `statusDrop` (`layout/status.ts`); Pixel `zoomTo` (`view-ops.ts`), `noteHover` (`hover.ts`), the `hover` store; Image `viewCommands.set`; `renderInfo` (`lib/shell/render-info.svelte.ts`).
- 2026-11-25: Earlier ACs that name the old places are superseded in their wording, not revised (`document-tabs` AC-01 and AC-04, `app-shell` AC-16, `macos-native-menu` AC-01), as `menu-structure` did with `settings-dialog` AC-07: revising them would set two accepted specs back to `todo` for a change this spec proves itself.
