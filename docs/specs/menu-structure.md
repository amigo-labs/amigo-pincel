---
status: todo
created: 2026-11-15
size: M
mode: both
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §9.5]
---

# Menu structure

## Context

The menu bars of the two editors grew entry by entry. Four things are off (owner request 2026-11-15, "optimize the menu and side panel structure"):

- View still holds entries that show nothing on the canvas: AI Agents…, Image Generation… and, in Pixel mode, the Frame-Time Probe. `settings-dialog` (2026-11-12) took the app settings out of View and left the AI entries for a later `ai-panel` spec; the owner now puts them into the Tools menu instead (Decisions, 2026-11-15).
- Tools exists in Image mode only and holds one entry, Recognize Text….
- The Layer menu lists five flips and rotations plus a rotate dialog flat below the layer commands, about fifteen rows. Both modes have all five (Flip Horizontal, Flip Vertical, Rotate 90° CW, Rotate 90° CCW, Rotate 180°); the dialog is Rotate… in Pixel mode and Rotate / Zoom… in Image mode.
- Entries both modes have stand in different orders. Today that is the Select menu only: Invert Selection is third in Pixel mode and last in Image mode.

This spec is the first of three; `sidebar-tab-groups` and `panel-visibility` cover the side panels.

Terms:

- **Menu bar**: the editor's in-window menu — dropdowns on windows of 640 px and wider, the ☰ sheet below that. It exists on every platform, macOS included; on macOS the operating system additionally draws a native menu bar, and the in-window bar has no Help menu there.
- **Desktop app**: the Tauri build (Windows, Linux, macOS). **Browser**: the PWA.
- **Entry**: one row of a menu, named in this spec by its English label. Two entries are **the same entry** in both modes when they have the same English label.
- **Inert** menu: its title is dimmed and it cannot be opened. While an effect is open every menu except Effects and Adjustments is inert; while an Assistant answer runs every menu except View is inert. Both hold today.
- AI Agents… and Image Generation… exist only in the desktop app today (the browser can start neither an agent endpoint nor a ComfyUI server); the Frame-Time Probe exists only in Pixel mode.

## Scope

### In scope

- The Tools menu in both editors of the desktop app, with AI Agents… and Image Generation…; both leave View
- Recognize Text… moves from Tools to the Image menu (Image mode)
- Frame-Time Probe moves from View to Help (Pixel mode)
- Layer ▸ Transform: a submenu with the flips, the fixed rotations and the mode's rotate dialog
- One order for the entries both modes share
- The paths the command palette shows follow the new places
- Strings: the new label "Transform" in every shipped UI language (en, de, es, fr, ja, pt-BR)
- Documentation, no AC: `docs/specs/pincel.md` §9.2 names the new menu bar

### Out of scope

- The macOS native menu bar — because none of the moved entries is in it today (it has File, Edit, View with zoom, Help), and changing it needs a macOS check this spec does not otherwise need.
- New commands, new shortcuts, changed shortcut keys — because this spec only moves entries.
- View ▸ Panels — because `panel-visibility` adds it.
- Setup buttons inside the Generate and Assistant panels — because the owner chose the Tools menu (2026-11-15); the panels keep the links to the dialogs they already show.
- Reordering File and Edit — because their shared entries already agree in order and their mode-specific entries (Import PNG…, Print…, Fill Selection) sit in groups of their own.
- UI texts that name a menu path — because none exists (no string in `en.ts` / `de.ts` contains "▸").

## Acceptance criteria

- [ ] AC-01 (state): While the editor runs in the desktop app, the menu bar of both modes shows the menu "Tools" directly after View, with exactly two entries and no separator: AI Agents…, then Image Generation…. Choosing one opens the AI Agents dialog or the Image Generation dialog; the existing e2e tests of the two dialogs pass with only the menu path changed.
- [ ] AC-02 (state): While the editor runs in the browser, the menu bar of neither mode shows a Tools menu.
- [ ] AC-03 (always): The View menu of both modes lists no AI Agents…, Image Generation… or Frame-Time Probe entry. Its entries are, in this order: Zoom In, Zoom Out, Fit to Window, Actual Size; a separator; the view aids of the mode (Pixel: Show Pixel Grid, Show Grid, Grid Settings…, Tiled Mode; Image: Rulers, Pixel Grid, Units); a separator; Toggle UI. (`panel-visibility` later adds one submenu directly before Toggle UI.)
- [ ] AC-04 (always): In Image mode the Image menu ends with a separator followed by Recognize Text…. The entry is disabled while no document is open and opens the OCR dialog otherwise; the existing e2e tests of the OCR dialog pass with only the menu path changed.
- [ ] AC-05 (state): While the menu bar shows a Help menu (browser, Windows, Linux), Pixel mode's Help menu lists Frame-Time Probe as the last entry before the separator above About, with the hint F2 and a check mark while the probe is on; choosing it toggles the probe as F2 does. Image mode's Help menu has no such entry.
- [ ] AC-06 (state): While the menu bar shows no Help menu (macOS desktop app), F2 toggles the probe in Pixel mode and the command palette lists the command as "Frame-Time Probe", without a menu path.
- [ ] AC-07 (always): The Layer menu of both modes has the submenu "Transform" as its last entry, after a separator, holding in this order: Flip Horizontal, Flip Vertical; a separator; Rotate 90° CW, Rotate 90° CCW, Rotate 180°; a separator; Rotate… (Pixel mode) or Rotate / Zoom… (Image mode). None of these entries remains directly in the Layer menu.
- [ ] AC-08 (always): Each entry of Layer ▸ Transform shows the shortcut hint it showed in the Layer menu before (a rebound key included), is disabled while no document is open, and the existing e2e tests of these entries — effect, undo, disabled state — pass with only the menu path changed.
- [ ] AC-09 (always): For every menu that both modes show on the same platform, the entries that are the same entry in both modes, submenus included, appear in the same relative order in both.
- [ ] AC-10 (always): The Select menu holds exactly: Select All, Deselect, Invert Selection; a separator; Expand…, Contract… and, in Image mode only, Feather… as the last entry.
- [ ] AC-11 (always): The top-level menus appear in this order, a menu being left out where the mode or platform does not have it: File, Edit, Select, Image, Layer, Frame, Effects, Adjustments, View, Tools, Help.
- [ ] AC-12 (always): The command palette lists each moved command the platform's menu bar has under its new path — "Tools ▸ AI Agents…", "Tools ▸ Image Generation…", "Image ▸ Recognize Text…", "Layer ▸ Transform ▸ Flip Horizontal" (and the other Transform entries), "Help ▸ Frame-Time Probe" — and under none of the old paths. In the browser it lists no AI Agents… or Image Generation… command, as before.
- [ ] AC-13 (state): While the window is narrower than 640 px, the ☰ sheet lists the same menus and entries in the same order as the menu bar of a wider window does, Transform as a nested list.
- [ ] AC-14 (option): Where the UI language is German, the Tools menu keeps its existing label "Extras", the submenu reads "Transformieren", and every moved entry keeps its German label. Each of the other shipped UI languages (es, fr, ja, pt-BR) has its own text for "Transform".
- [ ] AC-15 (state): While an effect is open or an Assistant answer runs, the Tools menu is inert.

## Technical notes

Hints, not contracts.

- Image mode builds its menus inline in `ui/src/modes/image/ImageEditor.svelte` (about 300 lines from `id: 'file'`); Pixel mode has `ui/src/modes/pixel/menus.ts` with a `PixelMenuContext`. First commit of the build: move Image mode's menus into `ui/src/modes/image/menus.ts` the same way, with no visible change (`refactor(ui): …`). The shared-order rule (AC-09) is then easiest to hold with shared builders for the parts both modes have (Select, Layer ▸ Transform, View's zoom block) under `ui/src/lib/`.
- Entry sources: `agentsMenuItems()` (`ui/src/lib/agent/access.svelte.ts`), `imageGenerationItems()` (`ui/src/lib/comfyui/state.svelte.ts`; it prepends a separator today, which AC-01 does not want), `toolsMenu()` (`ui/src/modes/image/ocr/menu.ts`), `helpMenus()` (`ui/src/lib/help/menu.ts`, returns nothing where `hasNativeMenu()`; the Tour entry is conditional).
- The command palette takes commands the menus lack (`ui/src/lib/shell/command-palette.ts`); AC-06 uses that on macOS.
- AC-09 can be tested by reading both modes' menus in one e2e spec and comparing the label sequences.
- e2e calls to update (grep `menu(page, 'View', 'Image Generation…'`, `'Tools', 'Recognize Text…'`, `'Layer', 'Rotate…'`, `'Layer', 'Rotate 90° CW'`, `'Layer', 'Rotate / Zoom…'`, `'Layer', 'Flip …'`): `comfyui-connect.spec.ts`, `ocr-dialog*.spec.ts`, `rotsprite.spec.ts`, `shortcuts-coverage.spec.ts`, the rotate / zoom spec, the MCP live specs that open AI Agents…. `menu()` and `menuHint()` in `ui/e2e/helpers.ts` already take a path of any depth.
- Desktop-only entries are tested against the mocked Tauri bridge (`installTauriMock`, `ui/e2e/tauri-mock.ts`; Gotchas in AGENTS.md); macOS with `test.use({ userAgent })`.
- Feature specs whose text names an old path get a dated line under their `## Decisions` pointing here; their AC wording and Evidence stay (the behavior behind the entry is unchanged): `comfyui-connect` AC-01, `mcp-live-desktop` AC-02, `ocr-dialog` AC-01 / AC-02, `rotsprite` AC-07, `image-transform-handles-rotate-zoom` AC-01, `power-user-shortcuts-coverage` AC-07. `settings-dialog` already has its line.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Browser, Pixel mode | No Tools menu; View ends with Toggle UI; Help has the probe |
| Desktop app on macOS | The in-window menu bar shows Tools (AC-01) and no Help; the probe is reachable with F2 and the command palette (AC-06) |
| AI Agents… chosen while its dialog is already open | Cannot happen: the dialog is modal and covers the menu bar |
| Window resized across 640 px while a menu is open | As today; this spec does not change how the menu bar and the ☰ sheet swap |
| A user-rebound shortcut for Flip Horizontal | The hint in Layer ▸ Transform shows the rebound key (AC-08) |
| F2 in Image mode | Nothing, as today |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Tools holds only the two AI entries and Recognize Text… moves to Image, although the owner chose both "AI entries into Tools" and "move Recognize Text… to Image, drop Tools" | Both answers of 2026-11-15 hold at once this way: Tools is the desktop app's AI setup, OCR sits with the image commands | Low — if OCR should stay in Tools, AC-04 changes and Tools shows in the browser's Image mode again |
| The probe goes to Help, not Tools | It is a diagnostic readout, and Tools does not exist in the browser, where the probe must stay reachable | Low |
| Tools stays directly after View | That is where Image mode shows it today | Low |
| Select takes Pixel mode's order (Invert Selection third) | Select All, Deselect and Invert belong together; the radius commands (Expand, Contract, Feather) form the second group | Low |
| Only Select differs in shared order today | Read from both menu definitions on 2026-11-15 | Low — AC-09's test finds any other difference |
| The macOS native menu bar contains none of the moved entries | `src-tauri/src/main.rs` `build_menu` has File, Edit, View and Help submenus and no AI, OCR or probe item | Low |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | desktop › `{mode}: Tools follows View and holds the two AI entries` (`ui/e2e/menu-structure.spec.ts`); the dialogs' own tests open them from Tools (`ui/e2e/mcp-live-settings.spec.ts`, `comfyui-connect.spec.ts`, `comfyui-install.spec.ts`) | built |        |
| AC-02 | browser › `{mode}: no Tools menu, menus in order, a View of view entries only` (`ui/e2e/menu-structure.spec.ts`); `the PWA has no Tools ▸ Image generation…` (`ui/e2e/comfyui-connect.spec.ts`); `AC-13 the web app shows neither the setting nor the indicator` (`ui/e2e/mcp-live-settings.spec.ts`) | built |        |
| AC-03 | browser › `{mode}: no Tools menu, menus in order, a View of view entries only`; desktop › `{mode}: Tools follows View and holds the two AI entries` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-04 | `image: Image ends with Recognize Text…, which opens the OCR dialog` (`ui/e2e/menu-structure.spec.ts`); the OCR dialog's tests open it from Image (`ui/e2e/ocr-helpers.ts` `openOcr`, `ocr-dialog*.spec.ts`) | built |        |
| AC-05 | `pixel: Help lists the Frame-Time Probe before About and toggles it`, `image: Help has no Frame-Time Probe` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-06 | desktop on macOS › `pixel: no in-window Help; F2 and the palette reach the probe` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-07 | browser › `{mode}: Layer ends with the Transform submenu; its entries keep their hints` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-08 | browser › `{mode}: Layer ends with the Transform submenu; its entries keep their hints` (`ui/e2e/menu-structure.spec.ts`); hints and effect through the new path in `ui/e2e/shortcuts-coverage.spec.ts`, `image-rotate-zoom.spec.ts` (incl. `without a document the item cannot be chosen`), `rotsprite.spec.ts` | built |        |
| AC-09 | `entries both modes share stand in the same order` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-10 | browser › `{mode}: Select holds All, Deselect, Invert, then the radius commands` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-11 | browser › `{mode}: no Tools menu, menus in order, a View of view entries only`; desktop › `{mode}: Tools follows View and holds the two AI entries`; desktop on macOS › `pixel: no in-window Help; F2 and the palette reach the probe` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-12 | `the palette lists the moved commands under their new paths only`; desktop › `{mode}: Tools follows View and holds the two AI entries` (`ui/e2e/menu-structure.spec.ts`); `ui/e2e/command-palette.spec.ts` | built |        |
| AC-13 | phone › `{mode}: the ☰ sheet lists the menus and entries of the menu bar` (`ui/e2e/menu-structure.spec.ts`) | built |        |
| AC-14 | `German: Extras and Transformieren; the other languages have their own Transform` (`ui/e2e/menu-structure.spec.ts`); German paths in `ui/e2e/mcp-live-settings.spec.ts`, `comfyui-connect.spec.ts`, `image-rotate-zoom.spec.ts` | built |        |
| AC-15 | desktop › `{mode}: Tools is inert while an effect is open` (`ui/e2e/menu-structure.spec.ts`); `ui/e2e/assistant-chat.spec.ts` (Tools inert while an answer runs) | built |        |

## Decisions

- 2026-11-15 (owner): AI Agents… and Image Generation… go into the Tools menu, not into the Generate / Assistant panels. This replaces the plan noted in `settings-dialog` (out of scope: "they move into the AI panel (`ai-panel`)"); no `ai-panel` spec will be written for it. Reason: the dialogs must be reachable while a panel is hidden (`panel-visibility`).
- 2026-11-15 (owner): Recognize Text… moves to the Image menu, the Frame-Time Probe out of View, flips and rotations into Layer ▸ Transform, and shared entries get one order. Reasons for the places chosen: Assumptions.
- 2026-11-15 (clarification pass): German keeps "Extras" for Tools — the string exists and the menu is not new.
- 2026-11-15 (build): Red run — `pnpm exec playwright test e2e/menu-structure.spec.ts` against the build before the change: 16 of 19 failed for the old structure (probe in View, flat layer transforms, Tools holding Recognize Text…, Invert Selection last in Image mode, no Tools on the desktop). Three passed at once because the behavior existed: Pixel mode's Select order (AC-10), Image mode's Help without a probe (AC-05), and Tools inert while an effect is open in Image mode (AC-15, where Tools already existed).
- 2026-11-15 (build): The parts both modes share are built once in `ui/src/lib/components/layout/shared-menus.ts` (`selectMenu`, `layerTransformItems`, `toolsMenus`); the old `ui/src/modes/image/ocr/menu.ts` is gone. New exported API for review: those three functions with `SelectMenuContext` / `LayerTransformContext`, `imageMenus` / `ImageMenuContext` (`ui/src/modes/image/menus.ts`), `probeItem` (`ui/src/modes/pixel/menus.ts`), the `diagnostics` parameter of `helpMenus`, and `PaletteSource.extras` — commands without a menu path, which AC-06 needs on macOS.
- 2026-11-15 (build): The rotations in Layer ▸ Transform show no hint, as before: Ctrl+R-style keys belong to Image ▸ Rotate (the canvas), in both modes.
- 2026-11-15 (build): `ocr-dialog` AC-01 said "a Tools menu … holding Recognize Text…; Pixel mode has no Tools menu", which AC-01 / AC-04 here make false, so that AC was reworded there (contrary to the technical note "their AC wording stays"); the other listed specs only got a Decisions line.
- 2026-11-15 (build): AC-08's "disabled while no document is open" cannot occur in the browser (an editor is mounted only with a document); the existing test that holds the wasm back and checks the entry afterwards (`image-rotate-zoom.spec.ts`) is the closest observable and was moved to the new path.
