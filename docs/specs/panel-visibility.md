---
status: implemented
created: 2026-11-15
size: M
mode: both
design: [pincel.md §9.2]
---

# Panel visibility

## Summary

> Completed: 2026-11-26

- View ▸ Panels in both modes: one checkable entry per panel the mode has on the platform, the Timeline included, then Reset Panels; also in the command palette and the phone menu.
- A removed panel leaves the sidebar, drawer, bottom sheet or dock but stays mounted, so it shows what it showed when enabled again; its group hands the active tab to a neighbour and is drawn like a single panel with one enabled tab.
- Enabling a panel brings it into view (sidebar, drawer or bottom sheet); the removed set is stored per mode, and Reset Panels restores the first-use layout while keeping width, effect position and sidebar visibility.
- Removing the Assistant or Generate, and Reset Panels, is blocked while an answer or a generation runs.

## Context

Every panel a mode has is always in the sidebar; the user can collapse it but not remove it. Someone who never uses tilesets, slices or image generation keeps their tabs and headers, and nothing in the menu bar says which panels exist. The owner asked for panels that can be switched on and off from the menu (2026-11-15) and chose a submenu in View over a new Window menu, because the menu bar is already long (ten menus, eleven with Tools in the desktop app).

Third of three specs; it builds on `sidebar-tab-groups` (groups, tabs, active tab, first use, "not on the page") and on `menu-structure` (the View menu's content).

Terms — **sidebar**, **drawer**, **bottom sheet**, **group**, **single panel**, **active tab**, **collapsed**, **first use**, **not on the page**, **desktop app** as in `sidebar-tab-groups`, and:

- **Panel**: one of the mode's sidebar panels or, in Pixel mode, the Timeline. The Timeline counts because it is the largest piece of chrome a still-image pixel artist never uses, and the bottom sheet already lists it as a tab beside the others. On windows of 640 px and wider it is docked under the canvas.
- **Enabled** / **removed**: every panel is one or the other. A removed panel has no header, no tab and no content anywhere (sidebar, drawer, bottom sheet, dock) and is not on the page. An enabled panel has its header or tab; it can still be out of view (tab not active, group collapsed, sidebar closed).
- **Panels toggle**: the top-bar button that shows or hides the docked sidebar, opens the drawer, or opens the bottom sheet, depending on the window width.
- **Effect panel**: the floating panel of an open effect (`effect-panel`); its position is stored with the layout.

## Scope

### In scope

- View ▸ Panels in both editors: one checkable entry per panel, and Reset Panels
- Removed panels leave the sidebar, the drawer, the bottom sheet and (Timeline) the dock under the canvas
- Enabling a panel from the menu brings it into view
- The enabled / removed state stored per mode
- The commands in the command palette
- Strings: "Reset Panels" in every shipped UI language (en, de, es, fr, ja, pt-BR); the submenu label reuses the panels toggle's word

### Out of scope

- Shortcuts for single panels — because no key was asked for; the entries can be reached from the command palette.
- A close button on each panel — because the menu is the one place that also brings a panel back.
- Hiding the tool rail, the options bar or the status bar one by one — because Toggle UI covers them together.
- The macOS native View menu — because the in-window menu bar has View on every platform; the native menu keeps its zoom entries.
- Panels removed by default — because every panel was reachable so far, and a first-time user should see what exists.

## Acceptance criteria

- [x] AC-01 (always): The View menu of both modes has the submenu "Panels" directly before Toggle UI, in the same separator group. It lists one entry per panel the mode has on this platform, labelled with the panel's title, in sidebar order top to bottom (tabs of a group left to right), then Timeline in Pixel mode; a separator; "Reset Panels".
- [x] AC-02 (always): Each panel entry carries a check mark exactly while its panel is enabled — whether or not the panel is in view.
- [x] AC-03 (event): When the user chooses the entry of an enabled panel, then the panel is removed.
- [x] AC-04 (event): When the removed panel was its group's active tab, then the next enabled tab to the right, or else the nearest to the left, becomes the active tab; the group stays expanded or collapsed as it was. The same rule picks the active tab when a stored layout names a removed panel as active, and the bottom sheet's selected tab when the open tab's panel is removed; with no tab left the bottom sheet closes.
- [x] AC-05 (state): While exactly one panel of a group is enabled, the group is drawn like a single panel — that panel's title, no tab row — with the group's collapse state; when a second panel is enabled again, the tab row returns. While none is enabled, the group is not drawn.
- [x] AC-06 (event): When the user chooses the entry of a removed sidebar panel, then the panel is enabled and brought into view: it is its group's active tab, its group or single panel is expanded, and the sidebar is shown if it was hidden (1024 px and wider), the drawer opens (640 to 1023 px) or the bottom sheet opens on that panel's tab (below 640 px).
- [x] AC-07 (event): When the user chooses the entry of the removed Timeline, then it is docked under the canvas again (640 px and wider) or the bottom sheet opens on its tab (below 640 px).
- [x] AC-08 (state): While the Timeline is removed in Pixel mode, nothing is docked under the canvas and the canvas area takes that height; play / pause and the frame steps still work from the Frame menu and their shortcuts.
- [x] AC-09 (always): Which panels are enabled is kept per mode across a reload and across documents.
- [x] AC-10 (state): While it is first use, or the stored layout carries no enabled / removed state, every panel is enabled.
- [x] AC-11 (event): When the user chooses Reset Panels, then every panel is enabled and every group and single panel has the first-use collapse state and active tab of `sidebar-tab-groups` AC-07. The sidebar's width, the effect panel's position and whether the sidebar, drawer or bottom sheet is open stay as they are — they are not about which panels exist.
- [x] AC-12 (state): While every sidebar panel is removed, a window of 1024 px and wider draws no sidebar (the canvas area takes its width), and the panels toggle is disabled at 640 px and wider; below 640 px it is disabled only while the Timeline is removed too (Pixel mode).
- [x] AC-13 (state): While a panel is removed, commands outside it keep working: undo and redo with History removed, the Layer menu's commands with Layers removed, color swap and reset with Colors removed; a removed Generate panel starts no ComfyUI server.
- [x] AC-14 (event): When the user removes a panel and enables it again in the same session, then it shows what it showed before, as after a tab switch (`sidebar-tab-groups` AC-16).
- [x] AC-15 (state): While an Assistant answer runs, the Assistant's entry and Reset Panels are disabled; while a generation runs in the Generate panel, Generate's entry and Reset Panels are disabled. (Removing the panel would take away the only place to follow or stop that work.)
- [x] AC-16 (state): While the Layers | History group is not drawn, the Generate | Assistant group takes the shared height alone where it is expanded (`sidebar-tab-groups` AC-13); otherwise the remaining blocks keep the height of their content.
- [x] AC-17 (always): The command palette lists each panel entry as "View ▸ Panels ▸ {title}" and "View ▸ Panels ▸ Reset Panels".
- [x] AC-18 (state): While the window is narrower than 640 px, the ☰ sheet shows View ▸ Panels with the same entries and check marks.
- [x] AC-19 (option): Where the UI language is German, the submenu reads "Bedienfelder" (the panels toggle's word), Reset Panels reads "Bedienfelder zurücksetzen", and the entries carry the panels' German titles.

## Technical notes

Hints, not contracts.

- `FrameLayout` (`ui/src/lib/components/layout/panels.svelte.ts`) gets a record of removed panel ids; `loadLayout` treats a missing record as "all enabled" (AC-10). `EditorFrame.svelte` owns the layout state today, while the menus are built in the editors — the View ▸ Panels items need the frame's panel list and layout, so either the frame appends the submenu to the View menu it is handed, or the layout moves into a small store both can reach.
- `MenuAction.checked` already renders a check mark (`ui/src/lib/components/layout/menu.ts`).
- The German word: `frame.panels` in `ui/src/lib/i18n/de.ts` ("Bedienfelder").
- While an Assistant answer runs, View is the one menu that stays usable (`EditorFrame.svelte`, `ai-chat-panel` AC-10) — hence AC-15.
- e2e: `storedPref(page, 'layout:pixel')` (`ui/e2e/helpers.ts`) reads the stored layout; viewport sizes for drawer and bottom sheet as in `history-panel.spec.ts`.
- If a button or link elsewhere in the app opens a panel (none is known on 2026-11-15), it enables a removed panel the way AC-06 does.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Browser | No Generate and Assistant entries (the mode does not have those panels there) |
| A stored layout removes a panel the platform does not have | Ignored; no entry, no error |
| Layers removed, then a layer is added from the Layer menu | The layer is added; the panel stays removed |
| Entry of an enabled panel that is out of view (inactive tab, collapsed group, closed sidebar) is chosen | The panel is removed (AC-03): a check mark always toggles, so the menu stays predictable; choosing it again brings it into view (AC-06) |
| An effect is open | View is inert (`menu-structure`), so no entry can be chosen |
| Every panel removed, then Toggle UI twice | The tool rail and bars return; the sidebar stays absent |
| Two windows of the app change the same mode's layout | The last change stored wins on the next load, as today |
| IndexedDB unavailable | Removing works for the session; nothing is stored |
| App restarted after a panel with typed text was removed | The text is gone, as after any restart; AC-14 covers the session only |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The in-window menu bar shows View on macOS too | Only Help is left to the native menu (`helpMenus`), and the editors build View without a platform check | Low — if not, the submenu also needs a native twin |
| Enabling a panel opens the sidebar, drawer or sheet | An entry that changes nothing visible would look broken | Low |
| Reset Panels does not open or close the sidebar | Reset is about which panels exist and how they start, not about the window's current view | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/panel-visibility.spec.ts`: "wide › pixel: View ▸ Panels sits before Toggle UI and lists every panel, checked, then Reset Panels", "wide › image: the entries are Colors, Layers, History; …"; `ui/e2e/panel-visibility-desktop.spec.ts`: "the entries follow the sidebar order, Generate and Assistant included, the Timeline last"; `ui/e2e/menu-structure.spec.ts`: "browser › {mode}: no Tools menu, menus in order, a View of view entries only" | verified | b30f28e |
| AC-02 | `ui/e2e/panel-visibility.spec.ts`: "wide › image: the entries are Colors, Layers, History; a check mark does not depend on the panel being in view", "wide › choosing an enabled panel removes it, also while it is out of view; its entry loses the check mark" | verified | b30f28e |
| AC-03 | `ui/e2e/panel-visibility.spec.ts`: "wide › choosing an enabled panel removes it, also while it is out of view; …", "narrow › a removed panel has no tab in the bottom sheet; …" | verified | b30f28e |
| AC-04 | `ui/e2e/panel-visibility-layout.spec.ts`: "the tab that stands in for a removed one (AC-04)" (6 tests); `ui/e2e/panel-visibility.spec.ts`: "wide › a removed active tab hands over to the right, else to the left; the group keeps its collapse state", "wide › a stored layout that names a removed panel as active shows its neighbour", "narrow › a removed panel has no tab in the bottom sheet; removing the open tab moves on, the last one closes it" | verified | 2a45670 |
| AC-05 | `ui/e2e/panel-visibility.spec.ts`: "wide › a group with one enabled panel is drawn like a single panel, with none it is not drawn"; "wide › a lone panel takes the collapse state of its group, and the tab row takes the lone panel’s (AC-05)" | verified | b30f28e |
| AC-06 | `ui/e2e/panel-visibility.spec.ts`: "wide › choosing a removed panel enables it and brings it into view: active tab, expanded, sidebar shown", "medium › enabling a removed panel opens the drawer; …", "narrow › enabling a panel opens the bottom sheet on its tab; the Timeline too"; `ui/e2e/panel-visibility-desktop.spec.ts`: "a removed Generate panel starts no ComfyUI server; enabling it shows it and connects" | verified | b30f28e |
| AC-07 | `ui/e2e/panel-visibility.spec.ts`: "wide › the Timeline: removed, nothing is docked and the canvas area takes its height; frames still step and play", "narrow › enabling a panel opens the bottom sheet on its tab; the Timeline too" | verified | b30f28e |
| AC-08 | `ui/e2e/panel-visibility.spec.ts`: "wide › the Timeline: removed, nothing is docked and the canvas area takes its height; frames still step and play" | verified | b30f28e |
| AC-09 | `ui/e2e/panel-visibility.spec.ts`: "wide › which panels are enabled is stored per mode and survives a reload and a new document"; `ui/e2e/panel-visibility-layout.spec.ts`: "the stored list (AC-09, AC-10)" (4 tests) | verified | b30f28e |
| AC-10 | `ui/e2e/panel-visibility-layout.spec.ts`: "the stored list (AC-09, AC-10)" (4 tests); `ui/e2e/panel-visibility.spec.ts`: "wide › a stored layout without the list, or one that removes a panel the platform lacks, enables every panel", "wide › pixel: View ▸ Panels … lists every panel, checked, …" | verified | 2a45670 |
| AC-11 | `ui/e2e/panel-visibility-layout.spec.ts`: "Reset Panels: all enabled, first-use groups; sidebar, width and effect position stay"; `ui/e2e/panel-visibility.spec.ts`: "wide › Reset Panels enables everything in its first-use state and keeps the width and the hidden sidebar" | verified | 2a45670 |
| AC-12 | `ui/e2e/panel-visibility.spec.ts`: "wide › with every sidebar panel removed there is no sidebar and the panels toggle is disabled", "medium › … with no sidebar panel the toggle is disabled", "narrow › a removed panel has no tab in the bottom sheet; …", "narrow › enabling a panel opens the bottom sheet on its tab; the Timeline too" | verified | b30f28e |
| AC-13 | `ui/e2e/panel-visibility.spec.ts`: "wide › commands outside a removed panel keep working: undo, Layer menu, color swap"; `ui/e2e/panel-visibility-desktop.spec.ts`: "a removed Generate panel starts no ComfyUI server; …" | verified | b30f28e |
| AC-14 | `ui/e2e/panel-visibility.spec.ts`: "wide › a panel removed and enabled again shows what it showed before"; `ui/e2e/panel-visibility-desktop.spec.ts`: "the Assistant removed and enabled again keeps its conversation and draft; Generate keeps its prompt" | verified | b30f28e |
| AC-15 | `ui/e2e/panel-visibility-desktop.spec.ts`: "while an answer runs the Assistant entry and Reset Panels are disabled", "while a generation runs the Generate entry and Reset Panels are disabled" | verified | b30f28e |
| AC-16 | `ui/e2e/panel-visibility-desktop.spec.ts`: "without Layers \| History the Generate \| Assistant group takes the shared height alone"; `ui/e2e/panel-visibility.spec.ts`: "wide › without Layers \| History the remaining blocks keep the height of their content" | verified | b30f28e |
| AC-17 | `ui/e2e/panel-visibility.spec.ts`: "wide › the command palette lists every panel entry and Reset Panels, and runs them" | verified | b30f28e |
| AC-18 | `ui/e2e/panel-visibility.spec.ts`: "narrow › the ☰ sheet shows View ▸ Panels with the entries and check marks" | verified | b30f28e |
| AC-19 | `ui/e2e/panel-visibility.spec.ts`: "wide › German: Bedienfelder, Bedienfelder zurücksetzen and the panels’ German titles" | verified | b30f28e |

## Decisions

- 2026-11-15 (owner): the panel switches live in View ▸ Panels, not in a new Window menu — the menu bar is already long.
- 2026-11-15 (clarification pass): the spec says "enabled / removed" instead of "shown / hidden", because an enabled panel can be out of view; the Timeline counts as a panel; work in progress blocks removing its panel (AC-15).
- 2026-11-26 (build): A removed panel stays mounted and its `Panel` chrome draws nothing — the mechanism a tab that is not shown already uses — so it shows what it showed when it comes back (AC-14) and a removed Generate panel connects to no server (AC-13). "What it showed" is what a tab switch keeps: state in the panel's own component and in stores, not a value typed into a field and never committed. With every sidebar panel removed the sidebar element stays mounted and hidden for the same reason.
- 2026-11-26 (build): A group's lone enabled panel draws its own header (title, count, collapse control), exactly like a single panel; the collapse control changes the group's state (AC-05). The group wrapper stays in the tree, so no panel remounts when the tab row goes or returns.
- 2026-11-26 (build): The frame publishes the submenu's entries per mode (`setPanelMenu`) and the editors place `panelsSubmenu(mode)` in View — the second option of the technical notes, without moving the layout out of the frame.
- 2026-11-26 (build): The assumption "the in-window menu bar shows View on macOS too" no longer holds: since `macos-native-menu` the macOS desktop app has only the native bar, which mirrors the editor menus as data, so View ▸ Panels appears there without a twin.
- 2026-11-26 (build): `expandPanel` (generation info's "show the Generate panel") enables a removed panel the way AC-06 does (technical notes, last point).
- 2026-11-26 (build): "A generation runs in the Generate panel" (AC-15) is true while any tab's generation runs: the panel is shared by the mode's tabs, and removing it would hide the running tab's progress as soon as that tab is shown.
- 2026-11-26 (build): Removing the active tab stores its stand-in as the group's active tab, so the stored layout never names a removed panel after a change made in the app; a layout that does (written by hand or an older build) is read with the same rule.
- 2026-11-26 (build): Red runs: the layout tests failed on the missing exports; the e2e specs were written beside the code and run against main's source afterwards (27 of 27 failed). `menu-structure`'s View test now expects "Panels" before Toggle UI, as its AC-03 foresees.
- 2026-11-26 (build): New public API, for review: `FrameLayout.removed`, `nextEnabled`, `resetPanels` (`layout/groups.ts`); `PanelSlot.removed`, `setPanelMenu`, `panelsSubmenu` (`layout/panels.svelte.ts`); `generationBusy` (`comfyui/generation.svelte.ts`); the string `menu.resetPanels`.
