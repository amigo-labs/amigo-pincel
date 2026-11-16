---
status: implemented
created: 2026-11-15
size: L
design: [pincel.md §9.2, pincel.md §9.5, pincel.md §15]
mode: both
---

# The shared amigo theme and a light mode

## Context

Every amigo app gets the same look: one theme, kept in amigo-labs/website (`theme/`), and one accent per app, the `accent` of its `docs/site/project.yml`. Pincel so far has its own dark editor theme (the `pc-*` tokens of §9.2) with an accent that is not its brand colour, and no light mode ("light theme" in the backlog).

This spec moves Pincel's colours, font, radii and focus ring onto the shared theme and adds the light mode. Its controls (buttons, fields, sliders, menus, dialogs) keep their own shape here; `amigo-theme-controls` rebuilds them on the shared components.

Decided by the owner (Decision Log 2026-11-15): the shared theme with Pincel's brand accent `#29adff`; dark and light in every app, dark the default; the workspace around the document follows the theme; Inter is bundled as the UI font.

Terms. **UI**: everything the app draws around and over the document except the document's own pixels and the canvas overlays (selection outline, slice marks, grid, onion skin, cursors). **Theme files**: `tokens.css`, `components.css` and `theme.json`, written by amigo-labs/website `scripts/theme.ts`. **Token**: a named value of the theme files (`bg`, `surface`, `text`, `accent-ink`, …; their meaning is in the website's `theme/README.md`). **Theme choice**: the user's setting Dark, Light or System. **Active palette**: dark or light, what the theme choice resolves to. **Workspace**: the area of the canvas viewport around the document.

## Scope

### In scope

- The theme files in the repository, checked in CI against the generator.
- Every UI colour of the start screen, both editors and all dialogs from the tokens; Pincel's accent in its roles.
- A light palette, and the theme choice in the Settings dialog.
- Inter as the UI font, bundled.
- Radii and the focus ring from the tokens.

### Out of scope

- Rebuilding the controls on the shared component classes — because that is `amigo-theme-controls`; here they only change colour, font, radius and focus ring.
- Renaming the `pc-*` class names in the components — because it changes nothing a user sees; the names stay as Pincel's aliases for the tokens (backlog).
- The native window title bar and the native macOS menus following the theme choice — because they follow the operating system, which is also what System and most users have; a follow-up if Dark on a light system looks wrong in the desktop app.
- Reading or setting the theme choice through the agent tools (MCP) — because the agent tools cover editing, and no agent task depends on the palette.
- The colours of document content and of canvas overlays (selection outline, slice marks, grid, onion skin, cursors) — because they are drawn to be visible on any image, not on the UI; they are only checked to stay visible on the light workspace (AC-09).
- The Text tool's font on the canvas — because Inter is the UI font, not a document font.

## Acceptance criteria

- [x] AC-01 (always, CI: site-data): The theme files in the repository are exactly what the website's theme command writes for Pincel's `docs/site/project.yml`; a pull request in which they differ fails the Site data check.
- [x] AC-02 (always): The editor draws its UI in the tokens of the active palette, checked on the start screen, in both editors with every panel open and in the Settings, New, About, Keyboard Shortcuts, PNG export and Rename dialogs: the window ground is `bg`, panels, bars, menus and dialogs are `surface`, fields are `surface-raised`, body text is `text`, and hairlines are `border`.
- [x] AC-03 (always): The editor's accent is Pincel's brand colour in its roles: the active tool, a switched-on toggle and the primary button are filled with `accent-solid` and carry `on-accent`; links and accent-coloured text are `accent-ink`; a selected layer, frame, history or list row is `accent-subtle`; slider thumbs and progress fills are `accent`. No UI element on the surfaces of AC-02 keeps the former accent blue (`#3b82f6`, `#2563eb`, `#7eaaff`).
- [x] AC-04 (state): While no theme choice is stored and none was picked since the app started, the active palette is dark, whatever the system prefers.
- [x] AC-05 (event): When the user opens the Settings dialog, then it shows a setting "Theme" with the choices Dark, Light and System, the stored choice selected (Dark while none is stored).
- [x] AC-06 (event): When the user picks a theme choice, then the whole UI (the open editor, the dialog itself, and the start screen once shown) has switched to its palette before the next input is handled, without a reload, and the choice is kept after a reload.
- [x] AC-07 (state): While the theme choice is System, the active palette is light where the system prefers light and dark otherwise (also where the system states no preference), and it switches when the system preference changes while the app is open.
- [x] AC-08 (event): When the app starts with a stored theme choice, then from the moment the start screen (or a document restored at startup) first appears in the page, it is in that choice's palette; the app renders nothing of its UI in the other palette before. The splash the operating system or browser shows before the app runs is not part of this.
- [x] AC-09 (state): While the active palette is light, the workspace is the light `bg`; the transparency checkerboard is the same in both palettes, because colours in a document must look the same whichever palette the UI has; a selection outline that crosses the workspace, and the document's edge, differ from the workspace in an element screenshot in both palettes.
- [x] AC-10 (always): In both palettes, text holds a contrast of at least 4.5:1 against the fill actually behind it in that state (hover, selected and accent fills included), and icons and control outlines at least 3:1, for: menu bar entries, an open menu's items and shortcuts, panel titles and labels, secondary and faint text, the status bar, tooltips, the active tool's icon, a selected layer row, the labels of primary, default, ghost and danger buttons (enabled), field text and placeholders, and error text.
- [x] AC-11 (always): UI text is set in Inter, which ships with the app: it shows without a network connection, in the browser and in the desktop app, and the app requests no font from another host.
- [x] AC-12 (always): Buttons, fields, menu items, panels and dialogs take their corner radius from the tokens (`radius-sm` for controls, `radius-md` for panels and menus, `radius-lg` for dialogs), and keyboard focus shows as a ring in the `focus` colour on every focusable button, field, menu item, tab and row in both palettes (the canvas itself shows no ring, as today).
- [x] AC-13 (state): While the active palette is light, the page's `theme-color` is the light `bg`; while it is dark, the dark `bg`.
- [x] AC-14 (option): Where the UI language is German, the setting reads "Erscheinungsbild" with "Dunkel", "Hell" and "System"; each of the other shipped languages has the four strings in that language, none falling back to English.
- [x] AC-15 (error): If the app cannot store a picked theme choice, then the pick still applies until the app is closed, and the next start uses what was stored before (dark if nothing was). If the stored choice cannot be read at startup, then the app starts dark and stays usable.
- [x] AC-16 (state, manual: visual design): While the active palette is light, the start screen, both editors with every panel open, and the Settings, New, Export and Keyboard Shortcuts dialogs look complete: no dark block, no invisible icon, no unreadable text.

## Technical notes

- The theme files live in `ui/src/theme/`; regenerate with `pnpm theme amigo-labs/amigo-pincel --out ../amigo-pincel/ui/src/theme` in amigo-labs/website. `.github/workflows/site-data.yml` passes `theme-dir: ui/src/theme` to the reusable check and also triggers on that folder.
- `tokens.css` is dark on `:root`, light under `data-theme="light"`, and follows the system when the root has no `data-theme`. Dark as the default therefore means: the root carries `data-theme="dark"` until the choice says otherwise; System removes the attribute.
- The `pc-*` Tailwind colours stay as names and are defined from the tokens, so the components need no rename: `bg`→`bg`, `panel`→`surface`, `raised`→`surface-raised`, `border`→`border`, `hover`→`hover`, `active`→`active`, `text`→`text`, `muted`→`text-secondary`, `faint`→`text-muted`, `control` and `track`→`border-strong`, `accent`→`accent`, `accent-text`→`accent-ink`, `accent-fg`→`on-accent`, `selected`→`accent-subtle`, `primary`→`accent-solid`, `danger`→`danger-solid`, `danger-text`→`danger-ink`. In the shared theme `accent` is decorative only: a place that puts text or an icon on an accent fill moves to `accent-solid` with `on-accent`.
- Raw colours in components and render code that belong to the UI (not to document content or overlays) move to tokens.
- The theme choice is a pref in IndexedDB (§9, no localStorage). Today the app renders first and reads its prefs afterwards (the language switches after mount); AC-08 needs the theme choice before the first render of the UI, so reading it is the one pref the first render waits for, with a short time limit after which the app starts dark.
- Inter comes from `@fontsource-variable/inter` (latin subset), a new dependency approved in the Decision Log entry.
- Tests: Playwright specs read computed colours and compute contrast from them (they may, §7.4; app code never reads canvas pixels); `page.emulateMedia({ colorScheme })` drives the system preference.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A stored theme choice has an unknown value (older or newer build) | Treated as no choice: dark |
| The system preference changes while the choice is Dark or Light | Nothing changes |
| Two tabs of the app are open and the choice changes in one | The other tab keeps its palette until it is reloaded, because no other setting syncs between open tabs either |
| The Settings dialog is open while the palette switches | The dialog itself switches, stays open and keeps focus on the chosen option |
| A document with a fully transparent or a white canvas in the light palette | The document's edge stays visible against the workspace (the checkerboard, or the document outline) |
| Printing (File ▸ Print, the shortcut sheet) | Unchanged: print output is black on white in both palettes |
| The system asks for forced colours (Windows high contrast) | Not handled here, because Pincel has no forced-colours support today and it is its own accessibility topic; the browser's forced colours apply as before |
| Inter cannot be loaded (a damaged install) | UI text falls back to the system UI font; nothing else changes |
| The UI language is Japanese | Latin text is Inter; Japanese glyphs come from the system font, as today |
| Reading the stored choice takes longer than the time limit | The app starts dark; a choice that arrives late applies then |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The order of the choices is Dark, Light, System | Dark is the default and comes first | Low: wording only |
| Tokens map one to one onto today's `pc-*` roles as listed in the technical notes | The roles were compared name by name | A role that fits badly shows in AC-10 or AC-16 and is corrected there |
| The PWA manifest's start colours stay dark | A manifest has one static colour; the default palette is dark | A light-palette user sees the operating system's dark splash before the app runs (not the app's UI, AC-08) |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `.github/workflows/site-data.yml` passes `theme-dir: ui/src/theme` to amigo-labs/website `validate-project.yml` (CI: site-data, green on 049f3e4, the last commit that changed the theme files: `site-data / validate` on PR #187); locally `node scripts/theme.ts ../amigo-pincel/docs/site/project.yml --out ../amigo-pincel/ui/src/theme --check` in the website checkout | verified | 96f3f02 |
| AC-02 | `ui/e2e/amigo-theme.spec.ts` › pixel / image, Dark / Light: panels, bars, an open menu and the dialogs are drawn in the tokens (AC-02) — every panel open, the bars, an open menu, the New, About, Keyboard Shortcuts, export and Rename dialogs; › pixel / image: the editor's surfaces, text and accent are the theme's tokens (AC-02, AC-03) — Settings; › the start screen is dark by default (AC-04) | verified | 7bf8a88 |
| AC-03 | `ui/e2e/amigo-theme.spec.ts` › pixel / image, Dark / Light: the accent sits in its roles, and no dialog keeps the former blue (AC-03) — primary button, accent text, selected layer / history / frame / list row, slider thumb, progress fill, the sweep with each dialog open; › the start screen keeps no former accent blue (AC-03); › pixel / image: the editor's surfaces, text and accent are the theme's tokens (AC-02, AC-03); › pixel / image: nothing keeps the former accent blue (AC-03) | verified | 7cb76e7, 7bf8a88 |
| AC-04 | `ui/e2e/amigo-theme.spec.ts` › the start screen is dark by default, whatever the system prefers (AC-04); › an unknown stored value counts as no choice (AC-04) | verified | 17aef06 |
| AC-05 | `ui/e2e/amigo-theme.spec.ts` › Settings shows Theme with Dark selected, and a pick applies at once and is kept (AC-05, AC-06, AC-13) | verified | 17aef06 |
| AC-06 | `ui/e2e/amigo-theme.spec.ts` › Settings shows Theme with Dark selected, and a pick applies at once and is kept (AC-05, AC-06, AC-13) | verified | 17aef06 |
| AC-07 | `ui/e2e/amigo-theme.spec.ts` › System follows the system preference, also while the app is open (AC-07, AC-13) | verified | 17aef06 |
| AC-08 | `ui/e2e/amigo-theme.spec.ts` › a stored choice is in place when the UI first appears (AC-08) | verified | 17aef06 |
| AC-09 | `ui/e2e/amigo-theme.spec.ts` › pixel / image: light: the workspace is light, a transparent document looks as in dark (AC-09); › pixel / image, Dark / Light: the document's edge and a selection outline on it differ from the workspace (AC-09) | verified | 7bf8a88 |
| AC-10 | `ui/e2e/amigo-theme.spec.ts` › pixel / image, Dark / Light: text holds 4.5:1, icons and control outlines 3:1 on what is behind them (AC-10); `ui/e2e/image-resize-autocrop.spec.ts` › an invalid used field blocks Apply and Enter (AC-07) — a wrong field keeps its danger edge | verified | 7cb76e7, 7bf8a88, 777f604, 6af5d89 |
| AC-11 | `ui/e2e/amigo-theme.spec.ts` › UI text is Inter from the app itself (AC-11) | verified | 17aef06 |
| AC-12 | `ui/e2e/amigo-theme.spec.ts` › pixel / image: radii come from the tokens (AC-12); › pixel / image: keyboard focus shows the focus ring on buttons, fields, menu items, tabs and rows in both palettes (AC-12) | verified | ff4046d, 7bf8a88, 777f604 |
| AC-13 | `ui/e2e/amigo-theme.spec.ts` › Settings shows Theme … (AC-05, AC-06, AC-13); › System follows the system preference … (AC-07, AC-13) | verified | 17aef06 |
| AC-14 | `ui/e2e/amigo-theme.spec.ts` › every language names the theme setting in its own words (AC-14) | verified | 17aef06 |
| AC-15 | `ui/e2e/amigo-theme.spec.ts` › a choice that cannot be stored applies until the app is closed (AC-15); › a stored choice that cannot be read starts dark and usable (AC-15) | verified | 17aef06 |
| AC-16 | manual: the owner checked the light palette on the start screen, in both editors with their panels and in the Settings, New, Export and Keyboard Shortcuts dialogs ("Hell ist OK"), 2026-11-15 | verified | e931441 |

## Summary

> Completed: 2026-11-15

- The UI is drawn in the shared amigo theme: generated token files under `ui/src/theme/` (checked by the Site data workflow), mapped onto the `pc-*` roles, with Pincel's brand colour as the only accent.
- A light palette next to the dark default: Settings ▸ Theme offers Dark, Light and System; the choice is stored, in place when the UI first appears, and mirrored in `theme-color`.
- UI text is Inter, shipped with the app; radii come from the tokens and keyboard focus shows a ring in the `focus` colour.
- Text holds 4.5:1 and icons, control outlines and field edges 3:1 in both palettes; a wrong value keeps its danger edge.
- The setting is named in all six languages; the light palette was checked by the owner.

## Decisions

- 2026-11-15: Dark stays the default instead of following the system, because the owner chose "dark is the default, light can be chosen" and every existing user has a dark Pincel today.
- 2026-11-15: The palette switches without a transition, because a fade of every surface is costly on large editors and adds nothing.
- 2026-11-15 (build): What shows through a transparent Pixel document stays one fixed colour (`pc-doc`, the former dark panel colour) in both palettes. Pixel mode has no checkerboard; the area behind the document was the panel colour and would have turned white in the light palette, which changes how a sprite looks. AC-09's reason (colours in a document look the same in both palettes) decides it.
- 2026-11-15 (build): The active tool and switched-on toggles change from an accent tint to the accent fill, as AC-03 says; with the shared theme's decorative `accent` a tint with accent-coloured icons would miss 4.5:1 in the light palette.
- 2026-11-15 (build): Text-entry fields and selects get the field ground and the control radius from one unlayered rule in `app.css` instead of a class on each of them, so no field can be missed.
- 2026-11-15 (build): Status colours written as Tailwind palette classes (`text-red-400`, `text-amber-400`, …) and raw `rgb()` values in component styles move to tokens (new aliases `pc-warning-text`, `pc-success-text`); they were readable on dark only.
- 2026-11-15 (build): The first render waits at most 400 ms for the stored choice (`initTheme` in `ui/src/lib/theme.svelte.ts`, new public API: `theme`, `setThemeChoice`, `initTheme`, `THEME_CHOICES`, `THEME_PREF`, `isThemeChoice`).
- 2026-11-15 (build): Red run before the implementation: all 17 tests of `ui/e2e/amigo-theme.spec.ts` failed against the unchanged build (no tokens, no Theme setting).
- 2026-11-15: The `pc-*` names stay as aliases of the tokens, because renaming them in 86 files changes nothing visible and would make this spec too large (§3.2).
- 2026-11-15 (rework): Menu items, the rows of the palette menus and the entries of a tool flyout show the focus ring inset (2 px, offset −2 px) and keep their fill, because their menus scroll or clip and a ring outside the row would be cut. Text fields and selects keep the 1 px edge in the `focus` colour with a soft glow (the build's choice: a typing field holds focus for long); the AC-12 test accepts that as their ring.
- 2026-11-15 (rework): New alias `pc-focus` (`--am-focus`) for the places that write the ring as a utility class (the sidebar splitter, the effect panel's title bar, the angle dial). The focused splitter is filled with `focus` and carries the ring without a gap, a 4 px bar with a 1 px gap reads as two lines.
- 2026-11-15 (rework): Placeholders take the faint text colour (`text-muted`) instead of the browser's grey, which held 4.2:1 (dark) and 3.2:1 (light) on a field. `<progress>` is drawn like the slider (rail `border-strong`, fill `accent`); the app's only progress bar is in the desktop app's ComfyUI install wizard, which e2e cannot reach without its backend, so the AC-03 test puts a `<progress>` into the page and reads the fill the app's style gives it.
- 2026-11-15 (rework): Image mode's dialogs (`Modal`, the OCR dialog) had Tailwind's `rounded-lg` (8 px) and plain 4 px buttons; they take `radius-lg` and `radius-sm` now. Where a component wrote Tailwind's `rounded-md` / `rounded-lg` for a control or a floating panel, it writes the token aliases `rounded-pc-control` / `rounded-pc-panel` (same size as before). The two bottom sheets keep `rounded-t-lg`: their lines carry a raw shadow colour that `ui-lint` reports once the line is touched, and an upward shadow has no token. The colour picker's popover and the floating effect panel move to the theme's `elev-3` shadow for the same reason. The menu bar's entries, the tab's close button and the zoom control's text buttons move from Tailwind's plain `rounded` (4 px) to `rounded-pc-control`. Other elements that write plain `rounded` (about 90: keycaps, swatches, list boxes, some buttons of Pixel mode's panels and dialogs) are left as they are; text fields among them already get the control radius from the field rule in `app.css`.
- 2026-11-15 (rework): AC-09 reads "differ" as a difference of at least 3 of 255 in a colour channel for every pixel along the document's edge and of the outline; in addition a third of the outline's pixels must hold 3:1 against the workspace, so the outline reads as a line (its other dashes are the opposite colour). In both modes a selection ends at the document's edge, so the outline is checked where it runs along that edge. The edge is also checked with a white document: in Image mode it differs from the light workspace by the document's own white (about 1.1:1) and its shadow, there is no outline around an Image document.
- 2026-11-15 (rework): Image mode exports a PNG without a dialog; its export dialog in the AC-02 and AC-12 tests is the JPEG one. Error text is the shell's alert in Pixel mode and the status bar's notice (`warning-ink`) in Image mode, each for a file that cannot be opened.
- 2026-11-15 (rework): Red runs before the fixes (same tests, build of e931441): AC-12 focus test — Foreground / Background colour buttons, splitter, menu item, palette menu row, tool flyout entry and the colour picker's sliders without the ring; AC-12 radii — Image mode's JPEG export dialog 8 px instead of 12 px; AC-10 — placeholder 4.21:1 (dark) and 3.18:1 (light); AC-03 — progress fill not `accent`. The new AC-02 and AC-09 tests and the other new AC-03 / AC-10 checks were green at once: there the evidence was missing, not the behaviour.
- Rework after the second acceptance (2026-11-15). AC-12: the last buttons, two dialogs (Recovery, Pixel mode's Text) and the font picker that kept Tailwind's fixed 4px `rounded` take the token radii; the radii test also reads a small icon button outside the dialogs (swap colours). Keycaps, swatches, list boxes and previews keep 4px: the AC does not name them, and `amigo-theme-controls` moves them onto the shared components. AC-10: a text field's and a select's edge is `border-strong` (4.2:1 dark, 3.9:1 light) instead of the hairline `border` (1.4:1), as the shared theme's own components draw it; the AC-10 test now checks a field outline. A field that states its own edge (transparent until hovered, the danger border of a wrong value) keeps it. The first version of that rule hid the danger edge of a wrong value (Image mode's size fields, the ComfyUI seed): the rule now leaves out fields with `aria-invalid` or a danger border class, and `image-resize-autocrop.spec.ts` reads the edge of a wrong size field. A tool-option field no longer changes its edge under the pointer: its hover colour was `border-strong` already, which is now its colour at rest.
