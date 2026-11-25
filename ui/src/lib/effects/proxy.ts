// Which effects can show an instant reduced-size preview, and how their
// parameters scale to it (docs/specs/performance-background-jobs.md AC-05).
// The proxy runs the effect on the document point-sampled to the canvas's
// on-screen size, so a length in pixels (a radius, a distance, an offset)
// shrinks with it. Per-pixel adjustments need no scaling. Effects that are
// not listed (distortions, renders, seeded or global-structure effects)
// show no proxy: their full-resolution preview is the first one.

/** A wire slot holding a length in pixels: its index, the smallest value
 *  the effect accepts, and whether it is a whole number. */
type Length = readonly [slot: number, min: number, int: boolean];

const NONE: readonly Length[] = [];

const LENGTHS: Readonly<Record<string, readonly Length[]>> = {
  // Per-pixel (or whole-layer statistics): unchanged parameters.
  brightness_contrast: NONE,
  hue_saturation: NONE,
  curves: NONE,
  levels: NONE,
  color_balance: NONE,
  invert: NONE,
  grayscale: NONE,
  posterize: NONE,
  threshold: NONE,
  auto_level: NONE,
  exposure: NONE,
  invert_alpha: NONE,
  sepia: NONE,
  temperature_tint: NONE,
  // Neighbourhood effects: their lengths scale.
  gaussian_blur: [[0, 0.1, false]],
  motion_blur: [[0, 1, false]],
  unsharp_mask: [[1, 0.1, false]],
  bokeh: [[0, 1, true]],
  median: [[0, 1, true]],
  surface_blur: [[0, 1, true]],
  reduce_noise: [[0, 1, true]],
  morphology: [[2, 1, true]],
  glow: [[0, 1, true]],
  frosted_glass: [[0, 1, true]],
  fragment: [[1, 0, true]],
  pixelate: [[0, 2, true]],
  stylize_outline: [[0, 1, true]],
  drop_shadow: [
    [0, -Infinity, true],
    [1, -Infinity, true],
    [2, 0, true],
  ],
};

/**
 * `params` of effect `name` for a proxy at `scale` (< 1) of the document's
 * size; `null` when the effect shows no proxy.
 */
export function proxyParams(name: string, params: readonly number[], scale: number): number[] | null {
  const lengths = LENGTHS[name];
  if (lengths === undefined) return null;
  const out = [...params];
  for (const [slot, min, int] of lengths) {
    const v = out[slot];
    if (v === undefined) continue;
    const scaled = v * scale;
    out[slot] = Math.max(min, int ? Math.round(scaled) : scaled);
  }
  return out;
}

/** The effect ids that show a proxy (for tests). */
export const PROXY_EFFECTS: readonly string[] = Object.keys(LENGTHS);
