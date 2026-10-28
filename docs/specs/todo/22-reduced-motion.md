---
id: todo-reduced-motion
title: Respect "reduce motion" — static marching ants, no UI animation
status: todo
size: S (one session, small)
mode: both
depends_on: []
created: 2026-10-28
---

# Respect "reduce motion"

## Goal

When the OS or browser asks for reduced motion (`prefers-reduced-motion: reduce`), Pincel:
- draws the **selection outline as static** black and white dashes, with no marching;
- **turns off UI transitions and animations**: chevrons, drawers, sheets, hover fades.

Animation *content* is unaffected. Frame playback and the animation preview are what the user asked to see.

## Why

The user asked for it. The marching ants run continuously while a selection exists (two editors, rAF loops), which is exactly the kind of perpetual motion vestibular-sensitive users switch off at the OS level. It's an accessibility baseline, WCAG 2.3.3 "Animation from Interactions".

## Current state (read these first)

**Image mode ants.** `ui/src/modes/image/components/canvas/CanvasOverlay.svelte`
- `dashOffset` (≈L22);
- `marchingAnts()` sets `lineDashOffset = -dashOffset * unit` (≈L135–142);
- `loop()` advances `dashOffset = (dashOffset + 0.5) % 8` and requests the next frame **while something is on the overlay** (≈L180–195).

**Pixel mode ants.**
- `view.marchPhase` (mod 4) in `ui/src/modes/pixel/stores/editor.svelte.ts` (≈L75);
- `live.marchTicks` (≈L158);
- advanced in `ui/src/modes/pixel/render.ts` (≈L228–255, every `MARCH_FRAMES_PER_STEP` frames);
- used by `ui/src/modes/pixel/overlay-painter.ts` for the marquee, drag previews and the active slice (≈L60–150).

**CSS transitions.**
- Tailwind `transition-*` classes, for example the start-screen chevron `transition-transform` in `ui/src/lib/shell/StartScreen.svelte`.
- Panel, drawer and sheet animations in `ui/src/lib/components/layout/`.

**Nothing reads `prefers-reduced-motion` today** (`grep -rn reduced-motion ui/src` is empty).

## Design

**Shared reactive flag.** `ui/src/lib/motion.svelte.ts`:
- `export const motion = $state({ reduced: … })`, initialized from `matchMedia('(prefers-reduced-motion: reduce)')`;
- updated on its `change` event, so toggling the OS setting applies live.

**Image mode.** While `motion.reduced`:
- keep `dashOffset` fixed at 0;
- don't run the rAF loop just for the ants. Still redraw once when the selection changes.

**Pixel mode.** Don't advance `marchPhase` while `motion.reduced`. If the ants were the only reason for the render tick to keep scheduling, let it go idle.

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

**In-app override.** Optional; recommendation: yes, it's cheap.
- View ▸ **Reduce Motion** (checkbox), defaulting to "follow system".
- Stored in the IndexedDB prefs.
- When on, it sets a `data-reduce-motion` attribute on `<html>`. Duplicate the CSS selector to cover the attribute.

## Tasks (one S session)

1. **`docs(spec)`.** A sentence in §9.4 (or a new accessibility line in §9.2) plus a Decision Log entry: content animation stays, UI animation stops, and the static ants.
2. **`feat(ui)`.**
   - The flag, both overlays, the CSS block, the optional View toggle, and i18n.
   - e2e `ui/e2e/reduced-motion.spec.ts`, in both modes:
     - `page.emulateMedia({ reducedMotion: 'reduce' })`;
     - make a rectangular selection;
     - take two element screenshots of the canvas 300 ms apart: they must be **identical**;
     - the same without emulation: they must **differ**.

     This uses screenshots, which tests may do (§7.4). App code still never reads canvas pixels.

## Out of scope

- Reducing motion in playback or previews.
- Reduced transparency or contrast modes (separate accessibility work).
