---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.2, pincel.md §9.5, pincel.md §10.3, pincel.md §11.3]
---

# First-run tour and sample documents

## Context

A first-time user sees an empty start screen and then two editors with many tools. paint.net users don't know tilemaps and slices; Aseprite users don't know Image mode. The user asked for a short, skippable tour that points at the real UI, plus one sample document per mode to explore without starting from blank.

Already decided elsewhere: prefs live in IndexedDB, never `localStorage` (CLAUDE.md §9, spec §10.3); every string goes through `t()` (§9.5); the editor frame is responsive — desktop, tablet drawer, phone with bottom tool strip and sheets (§9.2, Decision Log 2026-10-11) — so tour anchors move between layouts; on macOS the native menu bar owns the Help menu and the in-app Help menu is not shown (§11.3, Decision Log 2026-10-27); the start screen has no menu bar and shows its own About button where no native Help menu exists; autostart (`autoNew`, Decision Log 2026-10-28) can skip the start screen entirely.

Migrated from the legacy plan `20-first-run-tutorial` (removed 2026-10-29, see git history) (2026-10-28).

## Scope

### In scope

- A tour of five to seven steps with a highlighted anchor and one sentence each, covering: the two modes; creating or opening a document; the tool rail and its sections; the options bar; the panels; undo and the shortcut overview; where to find help
- Step lists per context: start screen, Pixel editor, Image editor
- A first-launch banner on the start screen offering the tour; a one-time hint per editor offering that editor's part
- Replaying the tour from Help ▸ Tour, including the macOS native Help menu
- Keyboard control and screen-reader semantics of the tour
- Two sample documents (a small animated sprite, a layered image) and an "Open a sample" entry on the start screen, available offline
- English and German strings

### Out of scope

- Interactive lessons with validation — because that is the separate curriculum product in `docs/specs/lessons.md`
- Video, and tooltips on every control — because the plan limits the tour to a one-minute walkthrough of real UI
- Translations beyond English and German — because the other locales come with `more-languages`; whichever of the two lands second must translate the other's keys, which `pnpm check` enforces

## Acceptance criteria

- [ ] AC-01 (event): When the start screen shows and neither "tour seen" nor "tour declined" is stored, then a banner offers the tour with a **Tour** and a **No thanks** action, and no tour dialog is open.
- [ ] AC-02 (always): The tour never opens on its own; it starts only from the banner's Tour action, an editor's hint, or Help ▸ Tour.
- [ ] AC-03 (event): When the user chooses Tour or No thanks on the banner, then that choice is stored, and after a reload the banner does not show again.
- [ ] AC-04 (event): When the tour runs, then each step highlights its anchor element and shows a callout with one sentence, Back, Next, Skip and a "n / N" counter, and the callout lies fully inside the viewport.
- [ ] AC-05 (always): Each context's step list has at most seven steps, the start screen's and an editor's lists together cover the seven topics listed under Scope, and each step's English text is at most 140 characters.
- [ ] AC-06 (event): When a step's anchor is not present in the current layout (e.g. a collapsed panel, or the phone layout at 390 px), then that step shows its callout centered in the viewport instead of failing, and the tour can continue.
- [ ] AC-07 (event): When the tour is open, then Esc skips it, → and Enter go to the next step, ← goes to the previous step, and keyboard focus stays inside the callout.
- [ ] AC-08 (always): The open tour is exposed as a dialog to assistive technology, and its step text is announced when the step changes.
- [ ] AC-09 (option): Where the OS or browser requests reduced motion, the highlight and callout move between steps without any animation or transition.
- [ ] AC-10 (event): When an editor (Pixel or Image) opens for the first time, then a one-line hint offers that editor's part of the tour; on later opens of the same editor, also after a reload, the hint does not show.
- [ ] AC-11 (event): When the user chooses Help ▸ Tour in an editor, then the tour for that editor starts at step 1, regardless of stored tour choices.
- [ ] AC-12 (option, manual: the macOS native menu bar cannot be driven by Playwright): Where the desktop app runs on macOS, the native Help menu carries a Tour entry that starts the tour for the current context.
- [ ] AC-13 (event): When the user chooses "Open a sample" on the start screen and picks the sprite sample, then it opens in Pixel mode with 4 frames, at least one tag, 2 layers and a slice.
- [ ] AC-14 (event): When the user picks the image sample, then it opens in Image mode with 3 layers.
- [ ] AC-15 (state): While the app is offline after it has been loaded once, both samples still open.
- [ ] AC-16 (always): Each sample file is at most 100 KB and is licensed CC0.
- [ ] AC-17 (always): Every tour, banner, hint and sample string exists in English and German.

## Technical notes

Hints from the legacy plan and a 2026-10-29 code check; paths drift — grep for the names.

- **Boot**: `ui/src/App.svelte` reads prefs in `onMount` before drawing (`booting` flag, waits for `autoNew`); read the tour prefs in the same step. `ui/src/lib/shell/StartScreen.svelte` already has an About button for platforms without a native Help menu (comment "No menu bar is mounted here").
- **Autostart**: `ui/src/lib/shell/auto-new.svelte.ts` (`autoNew` pref) — with autostart on, the start screen and its banner never show; the per-editor hint (AC-10) is then the first offer.
- **Prefs**: `ui/src/lib/idb/prefs.ts` (`getPref` / `setPref` / `removePref`). Plan names `tourSeen` / `tourDeclined`.
- **Anchors**: `ui/src/lib/components/layout/EditorFrame.svelte` and `ToolRail.svelte` — tool rail `role="toolbar"` with `aria-label` `t('frame.tools')`; options bar `role="toolbar"` `frame.toolOptions`; panels `frame.panels` (dialog on phone, tablist). Anchor by role and aria-label, never by CSS class.
- **Help menu**: `ui/src/lib/help/menu.ts` (`helpMenus()` returns nothing when `hasNativeMenu()`), `ui/src/lib/help/state.svelte.ts` (`openAbout`). `AboutDialog.svelte` is a modal pattern to reuse. Native menu: `src-tauri/src/main.rs` (`Labels` table, menu builder) — a Tour item needs a label in each locale and an event to the renderer.
- **Engine sketch from the plan**: `ui/src/lib/tour/` with `steps.ts` (steps per context) and `Tour.svelte` (dimmed backdrop with a cut-out around the anchor via `getBoundingClientRect`, callout). `role="dialog"`, `aria-live` for the step text.
- **Samples**: plan proposes `ui/public/samples/slime.aseprite` (≈32×32, 4 frames, a tag, 2 layers, a slice) and `ui/public/samples/postcard.ora` (≈800×600, 3 layers, text, an effect baked in), both made in Pincel, loaded through the normal open path, precached by `vite-plugin-pwa` (`ui/vite.config.ts`). `.gitattributes` already lists `*.aseprite` and `*.ora` as binary.
- **Reduced motion**: no app-wide reduced-motion handling exists yet (`reduced-motion.md`); the tour reads `prefers-reduced-motion` itself.
- **e2e**: plan proposes `ui/e2e/tour.spec.ts`; helpers in `ui/e2e/helpers.ts`. Playwright's `emulateMedia({ reducedMotion: 'reduce' })` covers AC-09, `context.setOffline(true)` covers AC-15.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Autostart is on, first launch | No start screen, no banner; the editor's one-time hint offers the tour (AC-10) |
| IndexedDB unavailable (private mode, blocked storage) | Banner shows on every launch; choosing Tour / No thanks still works for the session; no error is shown |
| Phone layout (390 px): tool rail is a bottom strip, panels are a sheet | Steps anchor to the strip / sheet button where present, otherwise center (AC-06) |
| Window resized or rotated while a step is shown | Highlight and callout follow the anchor's new position and stay inside the viewport |
| User opens a file from the banner screen without answering the banner | Nothing stored; banner shows again on the next start-screen visit |
| Sample fails to load (corrupt cache) | The normal open-error path reports it; the start screen stays usable |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| "Once per mode" means the hint shows on the first open of that editor only, whether or not the user acts on it | The plan says "once per mode" without a dismissal rule | Low: a user who ignored it can still use Help ▸ Tour |
| The sample files can be produced reproducibly in Pincel (or by a script using the cores) and committed as binaries | The plan says "made in Pincel" | Low |

## Open questions

- The Decision Log entry is not written yet: own tour engine (no library), banner instead of an auto-started modal, and the two CC0 sample files shipped with the app (the plan's first task; CLAUDE.md §2.1).
- How is the start-screen part of the tour replayed where the start screen has no menu bar (web, Windows, Linux) — a Tour button next to the existing About button, or not at all?
- Does choosing "No thanks" on the start-screen banner also suppress the per-editor hints (AC-10), or do the hints still show once per mode?

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

## Decisions

- No tour library: a new dependency needs approval and a few hundred lines suffice (plan recommendation, CLAUDE.md §9).
- A banner, never an auto-started modal tour: people who came to open a file must not be blocked (plan).
- Anchors are found by ARIA role and label, not by CSS class: roles survive restyling and the responsive layouts (plan).
- A missing anchor centers the step instead of failing: the responsive frame hides or moves anchors per layout (plan).
- Samples are CC0 and at most 100 KB each so they can be precached for offline use without weighing on the PWA (plan).
- No `depends_on: more-languages`: the plan only says the tour is better after it if it should ship translated; English and German are enough to ship, and the typed dictionaries force the missing translations whichever spec lands second.
- Help ▸ Tour also appears in the macOS native Help menu (AC-12): on macOS the in-app Help menu is not shown (Decision Log 2026-10-27), so otherwise the tour could not be replayed there.
