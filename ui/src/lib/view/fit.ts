/**
 * Largest integer display-zoom multiplier that shows a whole
 * `spriteW × spriteH` sprite inside a `viewportW × viewportH` box while
 * leaving `margin` CSS px of breathing room on every edge.
 *
 * The result is clamped to `[minZoom, maxZoom]`. It falls back to
 * `minZoom` when the sprite is degenerate or the viewport is unmeasured
 * (0) or smaller than one sprite pixel plus margin, so a freshly-mounted
 * layout never yields a zero / negative zoom. The display path (CSS
 * `width = spriteW * zoom`, nearest-neighbor scaling) treats the return
 * value as a pure pixel multiplier — see `App.svelte` and spec §4.4.
 */
export function fitZoom(
  viewportW: number,
  viewportH: number,
  spriteW: number,
  spriteH: number,
  minZoom: number,
  maxZoom: number,
  margin = 0,
): number {
  if (spriteW <= 0 || spriteH <= 0) return minZoom;
  const availW = viewportW - margin * 2;
  const availH = viewportH - margin * 2;
  if (availW <= 0 || availH <= 0) return minZoom;
  const fit = Math.floor(Math.min(availW / spriteW, availH / spriteH));
  return Math.max(minZoom, Math.min(maxZoom, fit));
}
