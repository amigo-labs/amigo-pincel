---
status: todo
created: 2026-11-15
size: L
mode: both
design: [pincel.md §9.2]
---

# Sidebar tab groups

## Context

The sidebar stacks every panel as its own collapsible block: nine in Pixel mode's desktop app (Colors, Palette, Preview, Layers, Tilesets, Slices, History, Generate, Assistant), five in Image mode. Six of them start collapsed, so most of the stack is headers of panels that are rarely open. The owner asked for a better side panel structure (2026-11-15) and chose fixed groups: a panel that is rarely open becomes a tab beside a related one instead of a header of its own, while Colors and Palette stay separate blocks because pixel work needs both in view at once.

Second of three specs; `menu-structure` comes first, `panel-visibility` builds on this one.

Terms:

- **Sidebar**: the panel column — docked beside the canvas on windows of 1024 px and wider, a drawer over the canvas from 640 to 1023 px. **Bottom sheet**: the phone layout's panel area (below 640 px), which shows one tab per panel today.
- **Group**: one of the sidebar blocks AC-01 / AC-02 name with several panels; it shows them behind tabs. **Single panel**: a block with one panel and that panel's header, as today.
- **Active tab**: the one panel of a group whose tab is marked and whose content the expanded group shows. Every group always has exactly one.
- **Choosing a tab**: clicking or tapping it, pressing Enter or Space on it, or moving to it with the arrow keys (AC-12).
- **Collapsed** group or single panel: only its header (for a group: the tab row and the collapse control) is drawn.
- **Not on the page**: the panel's content is not rendered at all — no element of it can be found, focused or run.
- **First use**: no layout is stored for the mode in this profile. A **profile** is one browser's storage for the app, or the desktop app's; layouts are stored per mode in each profile and never shared between profiles.
- **Desktop app**: the Tauri build; Generate and Assistant exist only there.

## Scope

### In scope

- Fixed groups in the sidebar (docked and drawer) of both modes
- A tab row per group, collapse per group, the active tab and the collapse state stored per mode
- Keyboard and screen-reader access to the tabs
- What a panel keeps while it is not the active tab
- Strings: none new (tab labels are the existing panel titles in every UI language)
- Documentation, no AC: `docs/specs/pincel.md` §9.2 layout sketch and panel paragraph

### Out of scope

- Moving panels between groups or reordering them by drag — because the owner chose fixed groups (2026-11-15); a free layout is a spec of its own if wanted later.
- The bottom sheet — because it already shows one tab per panel; its selected tab stays its own, per session, and is not tied to a group's active tab.
- The timeline — because it is docked under the canvas, not in the sidebar.
- Hiding panels — because `panel-visibility` adds it.
- Floating or detached panels — because the owner asked for a tidier sidebar, not for a window system.

## Acceptance criteria

- [ ] AC-01 (always): Pixel mode's sidebar shows, top to bottom: Colors (single), Palette (single), a group Layers | History, a group Preview | Tilesets | Slices and, in the desktop app, a group Generate | Assistant.
- [ ] AC-02 (always): Image mode's sidebar shows, top to bottom: Colors (single), a group Layers | History and, in the desktop app, a group Generate | Assistant.
- [ ] AC-03 (always): A group's header shows one tab per panel, labelled with the panel's title in the order of AC-01 / AC-02, and a collapse control; an expanded group shows the active tab's panel below it, and the group's other panels are not on the page.
- [ ] AC-04 (event): When the user chooses a tab of an expanded group, then that panel becomes the group's active tab and replaces the one shown; choosing the tab that is already active changes nothing.
- [ ] AC-05 (event): When the user chooses a tab of a collapsed group, then the group expands and shows that panel.
- [ ] AC-06 (event): When the user uses a group's collapse control, then the group collapses or expands; collapsed, the tab row stays visible, the active tab stays marked and none of the group's panels is on the page.
- [ ] AC-07 (state): While it is first use, every group the mode has on the platform starts like this: Layers | History expanded with Layers active; Preview | Tilesets | Slices collapsed with Preview active; Generate | Assistant expanded with Assistant active. Colors and Palette are expanded.
- [ ] AC-08 (always): Each group's active tab and collapse state, and each single panel's collapse state, are kept per mode across a reload and across documents; the docked sidebar and the drawer show the same state.
- [ ] AC-09 (state): While the stored layout is one written before this spec (collapse states per panel, no group state), Colors and Palette keep their stored collapse state and every group starts as in AC-07.
- [ ] AC-10 (error): If the stored layout cannot be read as a layout at all, then the sidebar starts as in AC-07 and no error is shown; if a stored active tab names a panel the group does not have on this platform, then the group's first-use active tab is active, or its first panel where that one is missing too.
- [ ] AC-11 (state): While the Generate tab is not active or its group is collapsed, no ComfyUI server is started by the panel (`comfyui-connect` AC-03 holds as it does for the collapsed Generate panel today).
- [ ] AC-12 (always): The tab row is a tab list for assistive technology: each tab has the role tab and its selected state; while the group is expanded its body has the role tab panel and is named by the active tab. With focus on a tab, Left / Right move focus to the neighbouring tab and choose it, without wrapping at the ends; Home / End go to the first / last tab.
- [ ] AC-13 (always): Expanded groups Layers | History and Generate | Assistant share the sidebar height the other blocks leave, in equal parts; a collapsed one takes only its header and the other takes the rest. Preview | Tilesets | Slices takes the height of its panel's content. If the blocks need more height than the window has, the sidebar scrolls vertically, as today.
- [ ] AC-14 (state): While the docked sidebar is at its narrowest width (240 px), in English and in German, the sidebar does not scroll sideways, and every tab of every group can be brought fully into view (the tab row may scroll) and chosen.
- [ ] AC-15 (state): While the window is narrower than 640 px, the bottom sheet shows one tab per panel as before this spec.
- [ ] AC-16 (event): When the user switches away from a tab and back, then the panel shows what it showed before: the Assistant its conversation and the text typed into its input, Generate its prompt, settings and results, History its list, Layers its tree.
- [ ] AC-17 (state): While an Assistant answer runs, the tabs of Generate | Assistant can still be chosen; the answer keeps running while Generate is active, the Generate panel takes no input as today, and on return the Assistant shows the answer as far as it has come — or the finished or failed answer.
- [ ] AC-18 (state): While a generation runs in the Generate panel, choosing the Assistant tab does not stop it; on return the Generate panel shows its progress or its result.
- [ ] AC-19 (state): While an effect is open, the tab rows, collapse controls and panels take no pointer or key input, like the rest of the sidebar today.

## Technical notes

Hints, not contracts.

- `PanelDef` (`ui/src/lib/components/layout/panels.svelte.ts`) gets a group id; `EditorFrame.svelte` builds the blocks from it (`sidebarBody`, `hosted`). `FrameLayout.collapsed` is keyed by panel id today; group state can be keyed by group id beside an `activeTab` record. `loadLayout` must accept the old shape (AC-09).
- `Panel.svelte` reads a `PanelSlot` from context (`title`, `host`, `collapsed`, `toggle`); inside a group the group header replaces the panel's own header.
- Panel bodies mount only while expanded today, which is why a collapsed Generate starts no server; AC-03 / AC-06 keep that. AC-16 – AC-18 then need the panels' state outside the mounted component — check where `ai-chat-panel` and `comfyui-generate` keep conversation, draft, prompt and job state, and what collapsing those panels does to them today.
- Many e2e specs expand a panel by clicking its header (`history-panel.spec.ts`, the preview, tileset, slices, generate and assistant specs). Add one helper to `ui/e2e/helpers.ts` that brings a panel into view by title — it chooses the tab or expands the single panel — and use it everywhere.
- Feature specs that say a panel is "collapsed by default" (`history-panel` AC-01, `animation-preview`, `tilemaps`, `slices`, `comfyui-generate` AC-01) get a dated line under `## Decisions` pointing here: the panel now starts as a tab that is not active, or in a collapsed group.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Browser (no Generate, no Assistant) | No third group; Pixel mode shows two groups, Image mode one; Layers \| History takes all the shared height (AC-13) |
| IndexedDB unavailable (private window) | Groups work for the session with first-use state; nothing is stored |
| Two windows of the app change the same mode's layout | The last change stored wins on the next load, as today |
| Tab switched while a rename field is open in Layers, Slices or Tilesets | The rename ends as it does when the field loses focus |
| Tab switched while a drag in the Layers panel is in progress | Cannot happen by pointer; by keyboard the drag ends without reordering |
| Sidebar hidden with the panels toggle, then shown again | Groups, active tabs and collapse states as before hiding |
| Toggle UI (Tab on the canvas) | Hides and shows the sidebar with its groups as today |
| Window resized between docked sidebar and drawer | The same state shows in both (AC-08); focus inside the sidebar may be lost |
| History tab not active, user undoes with Ctrl+Z | Undo works; the History list is up to date when its tab is chosen |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Layouts stored before this spec lose the collapse state of panels that are now in groups | A one-time reset to sensible defaults is simpler and safer than guessing a group state from several panel states | Low — the user re-collapses a group once |
| Layers and History share a group | History starts collapsed today and, like the layer list, shows the document's state; it costs a tab instead of a header, and Layers is one click away while it is open | Medium — someone who watches the history while reordering layers must switch tabs |
| Preview shares a group with Tilesets and Slices, and the group starts collapsed | All three start collapsed today and are used by task (animation, tiles, export regions); kept apart from Layers so an opened preview shows beside the layer list | Medium — working on tiles while watching the preview needs tab switches |
| Assistant is the first-use active tab of its group, Generate is not | Generate must start nothing until the user asks for it (`comfyui-connect` AC-03); the Assistant was expanded by default before | Low |
| Arrow keys choose a tab at once and expand a collapsed group | One rule for every way of choosing a tab; the common tab-list pattern | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `ui/e2e/sidebar-groups.spec.ts`: "wide › pixel: Colors, Palette, Layers | History, Preview | Tilesets | Slices, in their first-use state (AC-01, AC-03, AC-07)"; `ui/e2e/sidebar-groups-desktop.spec.ts`: "pixel: three groups, Generate | Assistant last with the Assistant active (AC-01, AC-07)" | built |        |
| AC-02 | `ui/e2e/sidebar-groups.spec.ts`: "wide › image: Colors and Layers | History (AC-02, AC-03, AC-07)"; `ui/e2e/sidebar-groups-desktop.spec.ts`: "image: Layers | History and Generate | Assistant share the height in equal parts (AC-02, AC-07, AC-13)" | built |        |
| AC-03 | `ui/e2e/sidebar-groups.spec.ts`: "wide › pixel: Colors, Palette, Layers | History, … (AC-01, AC-03, AC-07)", "wide › image: Colors and Layers | History (AC-02, AC-03, AC-07)" | built |        |
| AC-04 | `ui/e2e/sidebar-groups.spec.ts`: "wide › choosing a tab shows its panel, expands a collapsed group; the collapse control keeps the tab row (AC-04, AC-05, AC-06)" | built |        |
| AC-05 | `ui/e2e/sidebar-groups.spec.ts`: "wide › choosing a tab shows its panel, expands a collapsed group; … (AC-04, AC-05, AC-06)" | built |        |
| AC-06 | `ui/e2e/sidebar-groups.spec.ts`: "wide › choosing a tab shows its panel, expands a collapsed group; … (AC-04, AC-05, AC-06)" | built |        |
| AC-07 | `ui/e2e/sidebar-groups-layout.spec.ts`: "first use (AC-07)" (5 tests); `ui/e2e/sidebar-groups.spec.ts`: "wide › pixel: … in their first-use state (AC-01, AC-03, AC-07)", "wide › image: … (AC-02, AC-03, AC-07)"; `ui/e2e/sidebar-groups-desktop.spec.ts`: "pixel: three groups, … (AC-01, AC-07)" | built |        |
| AC-08 | `ui/e2e/sidebar-groups.spec.ts`: "wide › active tabs and collapse states are kept per mode across a reload, documents and the drawer (AC-08)"; `ui/e2e/sidebar-groups-layout.spec.ts`: "stored group state (AC-08)" | built |        |
| AC-09 | `ui/e2e/sidebar-groups-layout.spec.ts`: "a layout stored before the groups (AC-09)" (2 tests); `ui/e2e/sidebar-groups.spec.ts`: "wide › a layout stored before the groups keeps the single panels and starts the groups fresh (AC-09)" | built |        |
| AC-10 | `ui/e2e/sidebar-groups-layout.spec.ts`: "unreadable or stale values (AC-10)" (10 tests); `ui/e2e/sidebar-groups.spec.ts`: "wide › a stored value that is no layout, or names a tab the group lacks, starts as on first use without an error (AC-10)" | built |        |
| AC-11 | `ui/e2e/sidebar-groups-desktop.spec.ts`: "no ComfyUI server is asked for while the Generate tab is not active or its group is collapsed (AC-11)"; `ui/e2e/comfyui-generate.spec.ts`: "desktop › the desktop app has a Generate panel, a tab that is not active at first (AC-01)" | built |        |
| AC-12 | `ui/e2e/sidebar-groups.spec.ts`: "wide › the tab row is a tab list: roles, the tab panel named by the active tab, arrow keys without wrapping, Home and End (AC-12)" | built |        |
| AC-13 | `ui/e2e/sidebar-groups.spec.ts`: "wide › Layers | History takes the height the other blocks leave; Preview | Tilesets | Slices that of its content (AC-13)"; `ui/e2e/sidebar-groups-desktop.spec.ts`: "image: Layers | History and Generate | Assistant share the height in equal parts (AC-02, AC-07, AC-13)" | built |        |
| AC-14 | `ui/e2e/sidebar-groups.spec.ts`: "240 px sidebar, {English, Deutsch} › no sideways scroll; every tab can be brought into view and chosen (AC-14)" | built |        |
| AC-15 | `ui/e2e/sidebar-groups.spec.ts`: "narrow › the bottom sheet shows one tab per panel, as before (AC-15)" | built |        |
| AC-16 | `ui/e2e/sidebar-groups-desktop.spec.ts`: "a panel shows what it showed before a tab switch: conversation, draft, prompt, history, layers (AC-16)" | built |        |
| AC-17 | `ui/e2e/sidebar-groups-desktop.spec.ts`: "while an answer runs the tabs can be chosen; the answer goes on behind Generate, which takes no input (AC-17)", "an answer that is still running shows as far as it has come on return (AC-17)" | built |        |
| AC-18 | `ui/e2e/sidebar-groups-desktop.spec.ts`: "a running generation is not stopped by the Assistant tab; on return Generate shows its progress, then its result (AC-18)" | built |        |
| AC-19 | `ui/e2e/sidebar-groups.spec.ts`: "wide › while an effect is open the tab rows and collapse controls take no input (AC-19)" | built |        |

## Decisions

- 2026-11-15 (owner): fixed groups; Colors and Palette stay single and in view together; no free arrangement in this spec.
- 2026-11-15 (clarification pass): a panel keeps its content, drafts and running work across a tab switch (AC-16 – AC-18) — a tab that forgets is worse than the header it replaces.
- 2026-11-26 (build): Every panel of a group stays mounted; `Panel.svelte` draws nothing for one that is not the shown tab (no section, header or body). That is what a collapsed panel did before for its body, so a panel keeps what lived in its component — the Assistant's draft, folded layer groups, an open Add form — without moving state into stores, and the existing triggers keep working: Generate connects only when its body mounts (AC-11), the preview stops when its slot reports collapsed, the Assistant looks its CLIs up when it shows. Unmounting inactive tabs was rejected: the draft and the fold state would be lost (AC-16).
- 2026-11-26 (build): A grouped panel has no header of its own, so the count beside the title ("Layers 3", the palette keeps its own) is gone for panels inside a group; the tab is labelled with the panel title as AC-03 says. The panel's actions (Add layer, …) sit in a row above its content: a 240 px tab row has no room for them (AC-14).
- 2026-11-26 (build): The tab row is written with utilities in the look of the docked panel header (11 px, upper case, accent underline on the active tab). The shared `am-tabs` has no dense variant and the shared classes are not overridden here (`amigo-theme-controls`).
- 2026-11-26 (build): The collapse control and the tab list are named after the group's panel titles ("Layers, History"); no new string was needed.
- 2026-11-26 (build): Stored layout: `groups: { <group id>: { active, collapsed } }` beside the old `collapsed` record, which single panels keep using. Group ids are `document`, `assets`, `ai` (`ui/src/lib/components/layout/groups.ts`). A panel inside a group ignores its old `collapsed` entry (AC-09).
- 2026-11-26 (build): AC-15's test was green before the change — the criterion says the bottom sheet stays as it is. The desktop tests for AC-11 and AC-16 – AC-18 were first run after the frame was built and passed at once; before the groups they could not run (no Generate or Assistant tab existed). The other tests of `sidebar-groups.spec.ts` (11 of 12) and all of `sidebar-groups-layout.spec.ts` were red first.
- 2026-11-26 (build): Layers and History share a group (Assumptions), so about a dozen existing e2e tests that added a layer and read the history in one go now switch tabs (`addLayer` in `ui/e2e/history-helpers.ts`, `showPanel` / `hidePanel` in `ui/e2e/helpers.ts`). Specs that clicked a panel's own header use those helpers; tests that counted every `tab` on the page count inside the document tab bar.
- 2026-11-26 (build): As before, switching between the docked sidebar and the drawer mounts the panels anew, and state kept in a panel's component is lost there (Edge cases: "focus inside the sidebar may be lost"). Not changed by this spec.
- 2026-11-26 (build): New public API, for review: `PanelDef.group`, `PanelSlot.grouped`, `FrameLayout.groups`, `PANEL_GROUPS`, `groupState`, `normalizeLayout` (`layout/groups.ts`), the component `PanelGroup.svelte`.
