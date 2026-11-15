---
status: todo
created: 2026-11-15
size: M
mode: both
design: [pincel.md §9.2]
---

# Panel visibility

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

- [ ] AC-01 (always): The View menu of both modes has the submenu "Panels" directly before Toggle UI, in the same separator group. It lists one entry per panel the mode has on this platform, labelled with the panel's title, in sidebar order top to bottom (tabs of a group left to right), then Timeline in Pixel mode; a separator; "Reset Panels".
- [ ] AC-02 (always): Each panel entry carries a check mark exactly while its panel is enabled — whether or not the panel is in view.
- [ ] AC-03 (event): When the user chooses the entry of an enabled panel, then the panel is removed.
- [ ] AC-04 (event): When the removed panel was its group's active tab, then the next enabled tab to the right, or else the nearest to the left, becomes the active tab; the group stays expanded or collapsed as it was. The same rule picks the active tab when a stored layout names a removed panel as active, and the bottom sheet's selected tab when the open tab's panel is removed; with no tab left the bottom sheet closes.
- [ ] AC-05 (state): While exactly one panel of a group is enabled, the group is drawn like a single panel — that panel's title, no tab row — with the group's collapse state; when a second panel is enabled again, the tab row returns. While none is enabled, the group is not drawn.
- [ ] AC-06 (event): When the user chooses the entry of a removed sidebar panel, then the panel is enabled and brought into view: it is its group's active tab, its group or single panel is expanded, and the sidebar is shown if it was hidden (1024 px and wider), the drawer opens (640 to 1023 px) or the bottom sheet opens on that panel's tab (below 640 px).
- [ ] AC-07 (event): When the user chooses the entry of the removed Timeline, then it is docked under the canvas again (640 px and wider) or the bottom sheet opens on its tab (below 640 px).
- [ ] AC-08 (state): While the Timeline is removed in Pixel mode, nothing is docked under the canvas and the canvas area takes that height; play / pause and the frame steps still work from the Frame menu and their shortcuts.
- [ ] AC-09 (always): Which panels are enabled is kept per mode across a reload and across documents.
- [ ] AC-10 (state): While it is first use, or the stored layout carries no enabled / removed state, every panel is enabled.
- [ ] AC-11 (event): When the user chooses Reset Panels, then every panel is enabled and every group and single panel has the first-use collapse state and active tab of `sidebar-tab-groups` AC-07. The sidebar's width, the effect panel's position and whether the sidebar, drawer or bottom sheet is open stay as they are — they are not about which panels exist.
- [ ] AC-12 (state): While every sidebar panel is removed, a window of 1024 px and wider draws no sidebar (the canvas area takes its width), and the panels toggle is disabled at 640 px and wider; below 640 px it is disabled only while the Timeline is removed too (Pixel mode).
- [ ] AC-13 (state): While a panel is removed, commands outside it keep working: undo and redo with History removed, the Layer menu's commands with Layers removed, color swap and reset with Colors removed; a removed Generate panel starts no ComfyUI server.
- [ ] AC-14 (event): When the user removes a panel and enables it again in the same session, then it shows what it showed before, as after a tab switch (`sidebar-tab-groups` AC-16).
- [ ] AC-15 (state): While an Assistant answer runs, the Assistant's entry and Reset Panels are disabled; while a generation runs in the Generate panel, Generate's entry and Reset Panels are disabled. (Removing the panel would take away the only place to follow or stop that work.)
- [ ] AC-16 (state): While the Layers | History group is not drawn, the Generate | Assistant group takes the shared height alone where it is expanded (`sidebar-tab-groups` AC-13); otherwise the remaining blocks keep the height of their content.
- [ ] AC-17 (always): The command palette lists each panel entry as "View ▸ Panels ▸ {title}" and "View ▸ Panels ▸ Reset Panels".
- [ ] AC-18 (state): While the window is narrower than 640 px, the ☰ sheet shows View ▸ Panels with the same entries and check marks.
- [ ] AC-19 (option): Where the UI language is German, the submenu reads "Bedienfelder" (the panels toggle's word), Reset Panels reads "Bedienfelder zurücksetzen", and the entries carry the panels' German titles.

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
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |
| AC-18 |      | open   |        |
| AC-19 |      | open   |        |

## Decisions

- 2026-11-15 (owner): the panel switches live in View ▸ Panels, not in a new Window menu — the menu bar is already long.
- 2026-11-15 (clarification pass): the spec says "enabled / removed" instead of "shown / hidden", because an enabled panel can be out of view; the Timeline counts as a panel; work in progress blocks removing its panel (AC-15).
