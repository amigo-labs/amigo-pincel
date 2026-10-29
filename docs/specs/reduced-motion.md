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

When the OS or browser asks for reduced motion, Pincel draws the selection outline as **static** dashes and **turns off UI transitions and animations**. Animation *content* — frame playback and the animation preview — is what the user asked to see and stays unaffected. An in-app View ▸ Reduce Motion option forces the same behavior regardless of the system setting. It has two states: checked = always reduce, unchecked = follow the system; motion can never be forced on while the system requests reduce.

## Scope

### In scope

- Static selection outline (and the other marching outlines of the Pixel overlay) under reduced motion, in both modes
- UI transitions and CSS animations finish instantly under reduced motion
- Live reaction to a change of the system setting
- View ▸ Reduce Motion, persisted in IndexedDB prefs
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
- [ ] AC-09 (option): Where View ▸ Reduce Motion is on, the editor behaves as in AC-01, AC-03, AC-04 and AC-06 even when the system does not request reduced motion.
- [ ] AC-10 (event): When the user changes View ▸ Reduce Motion and reloads the app, then the setting is kept.
- [ ] AC-11 (always): A new profile follows the system setting (View ▸ Reduce Motion is not forced on).
- [ ] AC-12 (option): Where the UI language is German, the View ▸ Reduce Motion entry is German.
- [ ] AC-13 (state): While the system requests reduced motion and View ▸ Reduce Motion is unchecked, motion is reduced and the menu entry's label notes that the system setting is on (suffix "(on in system settings)").

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
| System requests reduce, in-app option unchecked | Reduced motion in effect; the checkbox stays unchecked and the label carries the system suffix (AC-13) |

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

## Decisions

- 2026-10-28 (legacy plan): Animation content (frame playback, animation preview) is not reduced — it is what the user asked to see.
- 2026-10-28 (legacy plan): Include the in-app View ▸ Reduce Motion option (plan recommendation: "yes, it's cheap") — some users can't or won't change the OS setting.
- 2026-10-29 (migration): Size raised from the plan's S to M — the change spans a shared flag, both overlays, global CSS, the View menu of both modes, prefs, i18n and e2e.
- 2026-10-29: A Decision Log entry records honouring `prefers-reduced-motion` and the in-app View ▸ Reduce Motion (IndexedDB pref): static selection / marching outlines, UI transitions and animations finish instantly, frame playback and the animation preview keep running, all through one shared `motion` flag (`ui/src/lib/motion.svelte.ts`) for both editors and the tour; `pincel.md` §9.4 gains one sentence — AGENTS.md §2.1 (owner accepted the recommendation).
- 2026-10-29: View ▸ Reduce Motion has two states: checked = always reduce, unchecked = follow the system, with no way to force motion on; while the system requests reduce, the label gets the suffix "(on in system settings)" and the checkbox shows only the user's own option (AC-13) — a system reduce request should always win (accessibility first), and the suffix explains the effective state without a three-way control (owner accepted the recommendation).
