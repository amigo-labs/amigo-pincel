---
id: todo-first-run-tutorial
title: Short tutorial on first start
status: todo
size: M (one session)
mode: shell + both editors
depends_on: []
created: 2026-10-28
---

# Short tutorial on first start

## Goal

On the very first launch, Pincel offers a **short, skippable tour** of five to seven steps. It points at the real UI with highlighted spots and one sentence each:
1. The two modes.
2. Create or open a document.
3. The tool rail and its sections.
4. The options bar.
5. The panels.
6. Undo and the shortcut overview.
7. Where to find help.

It can be replayed any time from Help ▸ Tour.

There is also an optional **sample document per mode**, a small animated sprite and a layered image, to explore without starting from blank.

## Why

The user asked for it. A first-time user sees an empty start screen and two editors with many tools. paint.net users don't know the tilemap and slice concepts, and Aseprite users don't know Image mode. A one-minute tour lowers the barrier, and the sample files show what the app can do.

## Current state (read these first)

**Start screen and shell.**
- `ui/src/lib/shell/StartScreen.svelte` and `ui/src/App.svelte`.
- `onMount` reads prefs before drawing: the `booting` flag waits for `autoNew`. The tour pref is read in that same step.
- **Autostart** (`ui/src/lib/shell/auto-new.svelte.ts`) can skip the start screen. The tour must still work inside the editor.

**Prefs.** `ui/src/lib/idb/prefs.ts` (`getPref` / `setPref`). Never use localStorage (CLAUDE.md §9).

**Editor frame.** `ui/src/lib/components/layout/EditorFrame.svelte`
- The tool rail has `role="toolbar"`, `aria-label` `frame.tools`.
- The menu bar, options bar and panels are all reachable by role or label, and are good tour anchors.

**Help menu.** `ui/src/lib/help/menu.ts` / `state.svelte.ts`. The `AboutDialog.svelte` pattern can be reused for a modal step.

**Responsive frame** (§9.2): desktop, tablet (drawer) and phone (bottom tool strip, sheets). **Anchors move between layouts.**

**Sample files.** None ship today. The PWA precaches assets (`vite-plugin-pwa`, `ui/vite.config.ts`).

## Design

**Tour engine.** No library: a new dependency needs approval, and a few hundred lines suffice.
- `ui/src/lib/tour/` contains:
  - `steps.ts`: a step list per context (start screen, pixel editor, image editor).
  - `Tour.svelte`: a dimmed backdrop with a cut-out around the anchor (`getBoundingClientRect`), plus a callout with text, Back, Next, Skip and "n / N".
- Anchors are **CSS selectors on roles and aria-labels**, never on classes. If an anchor is missing (a collapsed panel, a phone layout), that step centers its callout instead of failing.
- Use the keyboard: Esc skips, ←/→ go back and forth, Enter goes next. Focus is trapped in the callout. `role="dialog"`, `aria-live` for the step text.
- **Respect "reduce motion"** (`22-reduced-motion.md`): no animated transitions between spots.

**When it shows.**
- On the first launch (no `tourSeen` pref), the start screen shows a small banner: "New to Pincel? Take a 1-minute tour" with **Tour** and **No thanks**. **Never auto-start a modal tour.** People who came to open a file shouldn't be blocked.
- Choosing either stores `tourSeen` / `tourDeclined`.
- The first time each editor is opened, a one-line hint offers that editor's part ("Show me around Pixel mode"), once per mode.
- Help ▸ Tour replays the tour for the current context.

**Sample documents.**
- `ui/public/samples/slime.aseprite`: about 32×32, 4 frames, a tag, 2 layers, a slice.
- `ui/public/samples/postcard.ora`: about 800×600, 3 layers, text, an effect applied.
- Both made in Pincel, licensed CC0, under 100 KB each, precached for offline use.
- Start screen: an **"Open a sample"** link that loads them through the normal open path.
- Add `*.aseprite` / `*.ora` to `.gitattributes` as binary if they aren't listed yet (CLAUDE.md §6.4).

**Text.** Every string goes through `t()` (en + de, and the languages from `19-more-languages.md` when they exist). Keep each step under about 140 characters.

## Tasks (one M session)

1. **`docs(spec)`.** Add the tour and samples to §9.2 (start screen) and the Help menu, plus a Decision Log entry: own tour engine, banner instead of an auto-modal, and sample files with their license.
2. **`feat(ui)`.**
   - The tour engine, the steps for the start screen and both editors, the banner, the per-mode hint, Help ▸ Tour, and i18n.
   - The sample files and the "Open a sample" link.
   - e2e `ui/e2e/tour.spec.ts`:
     - first visit: the banner is shown;
     - Tour walks all steps via Next, and each callout is visible inside the viewport;
     - Skip stores the pref, and after a reload there is no banner;
     - Help ▸ Tour replays it;
     - phone viewport (390 px): steps with missing anchors center instead of erroring;
     - "Open a sample" opens the slime in Pixel mode with 4 frames.

## Out of scope

- Interactive lessons with validation. That is the curriculum vision in `docs/specs/lessons.md`, a separate product.
- Video, and tooltips on every control.
