---
status: todo
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

- [ ] AC-01 (always, CI: site-data): The theme files in the repository are exactly what the website's theme command writes for Pincel's `docs/site/project.yml`; a pull request in which they differ fails the Site data check.
- [ ] AC-02 (always): The editor draws its UI in the tokens of the active palette, checked on the start screen, in both editors with every panel open and in the Settings, New, About, Keyboard Shortcuts, PNG export and Rename dialogs: the window ground is `bg`, panels, bars, menus and dialogs are `surface`, fields are `surface-raised`, body text is `text`, and hairlines are `border`.
- [ ] AC-03 (always): The editor's accent is Pincel's brand colour in its roles: the active tool, a switched-on toggle and the primary button are filled with `accent-solid` and carry `on-accent`; links and accent-coloured text are `accent-ink`; a selected layer, frame, history or list row is `accent-subtle`; slider thumbs and progress fills are `accent`. No UI element on the surfaces of AC-02 keeps the former accent blue (`#3b82f6`, `#2563eb`, `#7eaaff`).
- [ ] AC-04 (state): While no theme choice is stored and none was picked since the app started, the active palette is dark, whatever the system prefers.
- [ ] AC-05 (event): When the user opens the Settings dialog, then it shows a setting "Theme" with the choices Dark, Light and System, the stored choice selected (Dark while none is stored).
- [ ] AC-06 (event): When the user picks a theme choice, then the whole UI (the open editor, the dialog itself, and the start screen once shown) has switched to its palette before the next input is handled, without a reload, and the choice is kept after a reload.
- [ ] AC-07 (state): While the theme choice is System, the active palette is light where the system prefers light and dark otherwise (also where the system states no preference), and it switches when the system preference changes while the app is open.
- [ ] AC-08 (event): When the app starts with a stored theme choice, then from the moment the start screen (or a document restored at startup) first appears in the page, it is in that choice's palette; the app renders nothing of its UI in the other palette before. The splash the operating system or browser shows before the app runs is not part of this.
- [ ] AC-09 (state): While the active palette is light, the workspace is the light `bg`; the transparency checkerboard is the same in both palettes, because colours in a document must look the same whichever palette the UI has; a selection outline that crosses the workspace, and the document's edge, differ from the workspace in an element screenshot in both palettes.
- [ ] AC-10 (always): In both palettes, text holds a contrast of at least 4.5:1 against the fill actually behind it in that state (hover, selected and accent fills included), and icons and control outlines at least 3:1, for: menu bar entries, an open menu's items and shortcuts, panel titles and labels, secondary and faint text, the status bar, tooltips, the active tool's icon, a selected layer row, the labels of primary, default, ghost and danger buttons (enabled), field text and placeholders, and error text.
- [ ] AC-11 (always): UI text is set in Inter, which ships with the app: it shows without a network connection, in the browser and in the desktop app, and the app requests no font from another host.
- [ ] AC-12 (always): Buttons, fields, menu items, panels and dialogs take their corner radius from the tokens (`radius-sm` for controls, `radius-md` for panels and menus, `radius-lg` for dialogs), and keyboard focus shows as a ring in the `focus` colour on every focusable button, field, menu item, tab and row in both palettes (the canvas itself shows no ring, as today).
- [ ] AC-13 (state): While the active palette is light, the page's `theme-color` is the light `bg`; while it is dark, the dark `bg`.
- [ ] AC-14 (option): Where the UI language is German, the setting reads "Erscheinungsbild" with "Dunkel", "Hell" and "System"; each of the other shipped languages has the four strings in that language, none falling back to English.
- [ ] AC-15 (error): If the app cannot store a picked theme choice, then the pick still applies until the app is closed, and the next start uses what was stored before (dark if nothing was). If the stored choice cannot be read at startup, then the app starts dark and stays usable.
- [ ] AC-16 (state, manual: visual design): While the active palette is light, the start screen, both editors with every panel open, and the Settings, New, Export and Keyboard Shortcuts dialogs look complete: no dark block, no invisible icon, no unreadable text.

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

## Decisions

- 2026-11-15: Dark stays the default instead of following the system, because the owner chose "dark is the default, light can be chosen" and every existing user has a dark Pincel today.
- 2026-11-15: The palette switches without a transition, because a fade of every surface is costly on large editors and adds nothing.
- 2026-11-15: The `pc-*` names stay as aliases of the tokens, because renaming them in 86 files changes nothing visible and would make this spec too large (§3.2).
