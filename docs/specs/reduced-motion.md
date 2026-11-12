---
status: todo
created: 2026-10-28
size: M
mode: both
design: [pincel.md §9.2, pincel.md §9.4, pincel.md §15]
---

# Respect "reduce motion"

## Context

The marching-ants selection outline runs continuously while a selection exists (two editors, two animation loops) — exactly the kind of perpetual motion vestibular-sensitive users switch off at the OS level. Respecting `prefers-reduced-motion` is an accessibility baseline (WCAG 2.3.3 "Animation from Interactions"), and the user asked for it. Nothing in the UI reads `prefers-reduced-motion` today.

When the OS or browser asks for reduced motion, Pincel draws the selection outline as **static** dashes and **turns off UI transitions and animations**. Animation *content* — frame playback and the animation preview — is what the user asked to see and stays unaffected. An in-app Settings ▸ Reduce motion option forces the same behavior regardless of the system setting. It has two states: checked = always reduce, unchecked = follow the system; motion can never be forced on while the system requests reduce.

## Scope

### In scope

- Static selection outline (and the other marching outlines of the Pixel overlay) under reduced motion, in both modes
- UI transitions and CSS animations finish instantly under reduced motion
- Live reaction to a change of the system setting
- Settings ▸ Reduce motion, persisted in IndexedDB prefs
- en + de strings; §9.4 accessibility sentence updated

### Out of scope

- Reducing motion in frame playback or the animation preview — because that motion is the content the user asked to see.
- Reduced transparency or contrast modes — because they are separate accessibility work.

## Acceptance criteria

- [ ] AC-01 (state): While the browser reports `prefers-reduced-motion: reduce` and a rectangular selection exists, two element screenshots of the canvas taken 300 ms apart are identical — in Pixel and in Image mode.
- [ ] AC-02 (state): While reduced motion is not requested and a rectangular selection exists, two element screenshots of the canvas taken 300 ms apart differ — in Pixel and in Image mode.
- [ ] AC-03 (state): While reduced motion is in effect in Pixel mode, the other marching outlines — a drag preview and the active slice's outline — are static too (screenshots 300 ms apart are identical).
- [ ] AC-04 (event): When the selection changes while reduced motion is in effect, then the outline shows the new selection (a screenshot after the change differs from one before it).
- [ ] AC-05 (event): When the system setting switches between reduce and no-preference while the app runs, then the outline stops or starts marching without a reload.
- [ ] AC-06 (state): While reduced motion is in effect, UI transitions and CSS animations finish instantly: the computed transition and animation durations of animated UI elements (e.g. the start screen's chevron) are at most 0.01 ms.
- [ ] AC-07 (state): While reduced motion is in effect in Pixel mode, frame playback still advances the timeline's current frame.
- [ ] AC-08 (state, manual: visual design): While reduced motion is in effect, the static selection outline remains clearly visible on light and dark content (black and white dashes).
- [ ] AC-09 (option): Where Settings ▸ Reduce motion is on, the editor behaves as in AC-01, AC-03, AC-04 and AC-06 even when the system does not request reduced motion.
- [ ] AC-10 (event): When the user changes Settings ▸ Reduce motion and reloads the app, then the setting is kept.
- [ ] AC-11 (always): A new profile follows the system setting (Settings ▸ Reduce motion is not forced on).
- [ ] AC-12 (option): Where the UI language is German, the Settings ▸ Reduce motion entry is German.
- [ ] AC-13 (state): While the system requests reduced motion and Settings ▸ Reduce motion is unchecked, motion is reduced and the setting notes that the system setting is on ("On in system settings" under the checkbox).

## Technical notes

Hints, not contracts — paths and line numbers drift; grep for the names (verified 2026-10-29).

**Image-mode ants.** `ui/src/modes/image/components/canvas/CanvasOverlay.svelte`: `dashOffset` (≈L22); `marchingAnts()` sets `ctx.lineDashOffset = -dashOffset * unit` (≈L142); `loop()` advances `dashOffset = (dashOffset + 0.5) % 8` (≈L189) and requests the next frame while something is on the overlay (≈L192 / L205). Plan design: keep `dashOffset` at 0 and don't run the rAF loop just for the ants; still redraw once when the selection changes (AC-04).

**Pixel-mode ants.** `view.marchPhase` (mod 4) in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L76) and `live.marchTicks` (≈L158), advanced in `ui/src/modes/pixel/render.ts` (≈L228–255, every `MARCH_FRAMES_PER_STEP` frames); used by `ui/src/modes/pixel/overlay-painter.ts` for the marquee, drag previews and the active slice. Don't advance `marchPhase` while reduced; if the ants were the only reason for the render tick to keep scheduling (`live.rafHandle`, `PixelEditor.svelte` ≈L439), let it go idle.

**Shared flag.** Plan design: `ui/src/lib/motion.svelte.ts` exporting `motion = $state({ reduced })`, initialised from `matchMedia('(prefers-reduced-motion: reduce)')` and updated on its `change` event, combined with the in-app option. `first-run-tutorial` currently plans to read `prefers-reduced-motion` itself — it should use this flag once it exists.

**CSS.** One block in `ui/src/app.css`:

```css
@media (prefers-reduced-motion: reduce) {
  *, ::before, ::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

The in-app option sets a `data-reduce-motion` attribute on `<html>`; duplicate the selector for it.

**Drift.** The plan names panel, drawer and sheet animations in `ui/src/lib/components/layout/`; as of 2026-10-29 the only CSS `transition` in `ui/src` is the start screen's chevron (`ui/src/lib/shell/StartScreen.svelte`) — the global CSS block still covers anything added later.

**Tests.** e2e `ui/e2e/reduced-motion.spec.ts`, both modes: `page.emulateMedia({ reducedMotion: 'reduce' })`, a rectangular selection, two element screenshots of the canvas 300 ms apart (identical / differ without emulation). Screenshots in tests are allowed (AGENTS.md §7.4); app code never reads canvas pixels.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| No selection exists | Nothing to animate; behavior unchanged |
| System setting switches to reduce mid-march | The outline freezes without reload (AC-05) |
| Selection moved or resized under reduced motion | Outline redrawn at the new place, still static (AC-04) |
| Pixel playback with reduced motion | Playback runs normally (AC-07) |
| In-app option on, system setting no-preference | Reduced motion in effect (AC-09) |
| Effect dialog live preview under reduced motion | Unaffected — it is content, not UI motion |
| System requests reduce, in-app option unchecked | Reduced motion in effect; the checkbox stays unchecked and notes the system setting under it (AC-13) |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `pixel: under reduced motion the selection outline stands still (AC-01)`, `image: …` (same title) (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-02 | `pixel: without reduced motion the selection outline marches (AC-02, AC-11)`, `image: …` (same title) (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-03 | `pixel: under reduced motion a drag preview and the active slice outline stand still (AC-03, AC-09)` (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-04 | `pixel: under reduced motion a changed selection shows at its new place (AC-04)`, `image: …` (same title); `pixel: View ▸ Reduce Motion stills the outline without the system setting (AC-09)`, `image: …` (same title) (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-05 | `pixel: a switch of the system setting stops and starts the march without a reload (AC-05)`, `image: …` (same title) (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-06 | `UI transitions finish instantly under reduced motion, from the system or the option (AC-06, AC-09, AC-10)` (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-07 | `pixel: frame playback still advances under reduced motion (AC-07)` (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-08 | manual sign-off pending: check the still outline on light and dark content in both modes (`manual: {who} checked {what}, {date}`); both overlays keep their black and white dashes, only the march stops | open |        |
| AC-09 | `pixel: View ▸ Reduce Motion stills the outline without the system setting (AC-09)`, `image: …` (same title); `pixel: under reduced motion a drag preview and the active slice outline stand still (AC-03, AC-09)`; `UI transitions finish instantly under reduced motion, from the system or the option (AC-06, AC-09, AC-10)` (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-10 | `UI transitions finish instantly under reduced motion, from the system or the option (AC-06, AC-09, AC-10)` (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-11 | `pixel: without reduced motion the selection outline marches (AC-02, AC-11)`, `image: …` (same title) (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-12 | `View ▸ Reduce Motion is German in the German UI (AC-12)` (`ui/e2e/reduced-motion.spec.ts`) | built |        |
| AC-13 | `pixel: under a system reduce request the entry stays unchecked and names the system setting (AC-13)`, `image: …` (same title) (`ui/e2e/reduced-motion.spec.ts`) | built |        |

## Decisions

- 2026-10-28 (legacy plan): Animation content (frame playback, animation preview) is not reduced — it is what the user asked to see.
- 2026-10-28 (legacy plan): Include the in-app View ▸ Reduce Motion option (plan recommendation: "yes, it's cheap") — some users can't or won't change the OS setting.
- 2026-10-29 (migration): Size raised from the plan's S to M — the change spans a shared flag, both overlays, global CSS, the View menu of both modes, prefs, i18n and e2e.
- 2026-10-29: A Decision Log entry records honouring `prefers-reduced-motion` and the in-app View ▸ Reduce Motion (IndexedDB pref): static selection / marching outlines, UI transitions and animations finish instantly, frame playback and the animation preview keep running, all through one shared `motion` flag (`ui/src/lib/motion.svelte.ts`) for both editors and the tour; `pincel.md` §9.4 gains one sentence — AGENTS.md §2.1 (owner accepted the recommendation).
- 2026-10-29: View ▸ Reduce Motion has two states: checked = always reduce, unchecked = follow the system, with no way to force motion on; while the system requests reduce, the label gets the suffix "(on in system settings)" and the checkbox shows only the user's own option (AC-13) — a system reduce request should always win (accessibility first), and the suffix explains the effective state without a three-way control (owner accepted the recommendation).
- 2026-11-03 (build): The e2e stillness check compares four canvas screenshots taken 300, 370 and 430 ms after the first — one pair 300 ms apart can land on the same dash phase by chance (the ants' period is a few hundred milliseconds), which made a single-pair check pass before the change.
- 2026-11-03 (build): The label carries the suffix whenever the system requests reduced motion, checked or not — the 2026-10-29 decision ties the suffix to the system request, and it stays true information when the option is on as well.
- 2026-11-03 (build): Pixel mode's render tick keeps running (it drains the core's events every frame); under reduced motion only `marchPhase` stops advancing and rests at 0, and Image mode's overlay stops its rAF loop and redraws once per change at dash offset 0 — the tick has work besides the ants, the overlay loop has none.
- 2026-11-03 (build): View ▸ Reduce Motion lives in the in-window menu bar of both editors (next to Language), not in the native Tauri menu — the native View menu carries only the zoom entries and has no Language submenu either.
- 2026-11-03 (build): AC-07's test was green from the start — frame playback never read the flag; the test guards that it stays that way.
- 2026-11-12: The entry moved from the View menu into the Settings dialog (Edit ▸ Settings…, Ctrl+, / ⌘,; `settings-dialog`) — the owner wanted View to hold only what the canvas shows. Its behavior is unchanged.
