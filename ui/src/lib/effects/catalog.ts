// Effect / adjustment catalog for the Effects and Adjust menus of both
// modes (Pixel and Image share `pincel_effects::EffectSpec` as the wire
// format; spec §15, 2026-10-11).
//
// Mirrors `pincel_effects::EffectSpec`: every entry names the wasm wire
// id and lists its parameters in the exact order `applyEffect` /
// `previewEffect` expect them. Choice parameters travel as their option
// index, booleans as 0 / 1, colours as one integer 0xRRGGBBAA (spec §15,
// 2026-10-29). Keep this table in step with the Rust enum;
// the `every_effect_name_is_accepted` test on the Rust side and
// `assertCatalogCoversAllEffects` below guard the id set from each end.

/** `effects` entries sit directly in the Effects menu, after its submenus. */
export type EffectGroup = 'blur' | 'sharpen' | 'distort' | 'noise' | 'effects' | 'adjust';

export type ParamDef =
  | {
      kind: 'number';
      key: string;
      label: string;
      min: number;
      max: number;
      step: number;
      default: number;
    }
  | { kind: 'bool'; key: string; label: string; default: boolean }
  | { kind: 'choice'; key: string; label: string; options: string[]; default: number }
  /** A straight-alpha colour as the integer 0xRRGGBBAA; it defaults to the
   *  current foreground colour (`EffectContext`), to `default` without one. */
  | { kind: 'color'; key: string; label: string; default: number };

/** What a dialog knows beyond the catalog when it opens. */
export type EffectContext = {
  /** The foreground colour as 0xRRGGBBAA. */
  foreground?: number | undefined;
};

export type EffectDef = {
  id: string;
  label: string;
  group: EffectGroup;
  params: ParamDef[];
  /** Build the wire parameter vector from the current values. Defaults to
   *  the params in declared order; Curves overrides to interleave the
   *  fixed x positions. */
  toParams?: (values: Record<string, number>) => number[];
};

const num = (
  key: string,
  label: string,
  min: number,
  max: number,
  def: number,
  step = 1,
): ParamDef => ({ kind: 'number', key, label, min, max, step, default: def });
const bool = (key: string, label: string, def = false): ParamDef => ({
  kind: 'bool',
  key,
  label,
  default: def,
});
const choice = (key: string, label: string, options: string[], def = 0): ParamDef => ({
  kind: 'choice',
  key,
  label,
  options,
  default: def,
});

const color = (key: string, label: string): ParamDef => ({
  kind: 'color',
  key,
  label,
  default: 0x000000ff,
});

// Curves is exposed as five output sliders at fixed input positions —
// enough for a pixel-art tone tweak without a spline editor.
const CURVE_XS = [0, 64, 128, 192, 255];

export const EFFECTS: EffectDef[] = [
  {
    id: 'gaussian_blur',
    label: 'Gaussian Blur',
    group: 'blur',
    params: [num('radius', 'Radius', 0.1, 250, 2, 0.1)],
  },
  {
    id: 'box_blur',
    label: 'Box Blur',
    group: 'blur',
    params: [num('width', 'Width', 1, 251, 3, 2), num('height', 'Height', 1, 251, 3, 2)],
  },
  {
    id: 'motion_blur',
    label: 'Motion Blur',
    group: 'blur',
    params: [num('distance', 'Distance', 1, 100, 6), num('angle', 'Angle', 0, 360, 0)],
  },
  {
    id: 'radial_blur',
    label: 'Radial Blur',
    group: 'blur',
    params: [
      num('amount', 'Amount', 1, 100, 10),
      num('center_x', 'Center X', 0, 1, 0.5, 0.01),
      num('center_y', 'Center Y', 0, 1, 0.5, 0.01),
      choice('kind', 'Kind', ['Spin', 'Zoom']),
    ],
  },
  { id: 'sharpen', label: 'Sharpen', group: 'sharpen', params: [] },
  {
    id: 'unsharp_mask',
    label: 'Unsharp Mask',
    group: 'sharpen',
    params: [
      num('amount', 'Amount %', 1, 500, 50),
      num('radius', 'Radius', 0.1, 50, 1, 0.1),
      num('threshold', 'Threshold', 0, 255, 0),
    ],
  },
  {
    id: 'emboss',
    label: 'Emboss',
    group: 'distort',
    params: [
      num('angle', 'Angle', 0, 360, 45),
      num('elevation', 'Elevation', 0, 90, 45),
      num('relief', 'Relief', 0.1, 10, 1, 0.1),
    ],
  },
  {
    id: 'edge_detect',
    label: 'Edge Detect',
    group: 'distort',
    params: [
      choice('algorithm', 'Algorithm', ['Sobel', 'Prewitt', 'Laplacian']),
      num('amount', 'Amount %', 1, 400, 100),
    ],
  },
  {
    id: 'relief',
    label: 'Relief',
    group: 'distort',
    params: [num('angle', 'Angle', 0, 360, 45), num('amount', 'Amount', 0.1, 10, 1, 0.1)],
  },
  {
    id: 'add_noise',
    label: 'Add Noise',
    group: 'noise',
    params: [
      num('amount', 'Amount', 1, 100, 20),
      choice('noise_type', 'Distribution', ['Uniform', 'Gaussian']),
      choice('channels', 'Channels', ['RGB', 'Monochromatic']),
      num('seed', 'Seed', 0, 9999, 1),
    ],
  },
  {
    id: 'reduce_noise',
    label: 'Reduce Noise',
    group: 'noise',
    params: [num('radius', 'Radius', 1, 10, 1)],
  },
  {
    id: 'outline',
    label: 'Outline',
    group: 'effects',
    params: [
      choice('place', 'Place', ['Outside', 'Inside', 'Both']),
      choice('shape', 'Shape', ['Circle', 'Square'], 1),
      num('thickness', 'Thickness', 1, 8, 1),
      color('color', 'Color'),
    ],
  },
  {
    id: 'brightness_contrast',
    label: 'Brightness / Contrast',
    group: 'adjust',
    params: [
      num('brightness', 'Brightness', -150, 150, 0),
      num('contrast', 'Contrast', -150, 150, 0),
      bool('enhanced', 'Enhanced (S-curve)'),
    ],
  },
  {
    id: 'hue_saturation',
    label: 'Hue / Saturation',
    group: 'adjust',
    params: [
      num('hue', 'Hue', -180, 180, 0),
      num('saturation', 'Saturation', -100, 100, 0),
      num('lightness', 'Lightness', -100, 100, 0),
      bool('colorize', 'Colorize'),
    ],
  },
  {
    id: 'curves',
    label: 'Curves',
    group: 'adjust',
    params: [
      choice('channel', 'Channel', ['Composite', 'Red', 'Green', 'Blue', 'Alpha']),
      ...CURVE_XS.map((x) => num(`y${x}`, `Out @ ${x}`, 0, 255, x)),
    ],
    // The UI edits 0–255; the effect takes control points in [0, 1].
    toParams: (v) => [
      v.channel ?? 0,
      ...CURVE_XS.flatMap((x) => [x / 255, (v[`y${x}`] ?? x) / 255]),
    ],
  },
  {
    id: 'levels',
    label: 'Levels',
    group: 'adjust',
    params: [
      choice('channel', 'Channel', ['Composite', 'Red', 'Green', 'Blue']),
      num('in_black', 'Input black', 0, 255, 0),
      num('in_white', 'Input white', 0, 255, 255),
      num('gamma', 'Gamma', 0.1, 10, 1, 0.01),
      num('out_black', 'Output black', 0, 255, 0),
      num('out_white', 'Output white', 0, 255, 255),
    ],
    // The UI edits 0–255 points; the effect takes them in [0, 1].
    toParams: (v) => [
      v.channel ?? 0,
      (v.in_black ?? 0) / 255,
      (v.in_white ?? 255) / 255,
      v.gamma ?? 1,
      (v.out_black ?? 0) / 255,
      (v.out_white ?? 255) / 255,
    ],
  },
  {
    id: 'color_balance',
    label: 'Color Balance',
    group: 'adjust',
    params: [
      num('s_cr', 'Shadows cyan–red', -100, 100, 0),
      num('s_mg', 'Shadows magenta–green', -100, 100, 0),
      num('s_yb', 'Shadows yellow–blue', -100, 100, 0),
      num('m_cr', 'Midtones cyan–red', -100, 100, 0),
      num('m_mg', 'Midtones magenta–green', -100, 100, 0),
      num('m_yb', 'Midtones yellow–blue', -100, 100, 0),
      num('h_cr', 'Highlights cyan–red', -100, 100, 0),
      num('h_mg', 'Highlights magenta–green', -100, 100, 0),
      num('h_yb', 'Highlights yellow–blue', -100, 100, 0),
      bool('preserve_luminosity', 'Preserve luminosity', true),
    ],
  },
  { id: 'invert', label: 'Invert Colors', group: 'adjust', params: [] },
  {
    id: 'grayscale',
    label: 'Grayscale',
    group: 'adjust',
    params: [
      choice('method', 'Method', ['Luminosity', 'Average', 'BT.709', 'Channel mixer']),
      num('r', 'Red weight', 0, 1, 0.3, 0.01),
      num('g', 'Green weight', 0, 1, 0.59, 0.01),
      num('b', 'Blue weight', 0, 1, 0.11, 0.01),
    ],
  },
  {
    id: 'posterize',
    label: 'Posterize',
    group: 'adjust',
    params: [num('levels', 'Levels', 2, 64, 4)],
  },
  {
    id: 'threshold',
    label: 'Threshold',
    group: 'adjust',
    params: [num('threshold', 'Threshold', 0, 255, 128)],
  },
];

export const GROUP_LABELS: Record<EffectGroup, string> = {
  blur: 'Blur',
  sharpen: 'Sharpen',
  distort: 'Distort',
  noise: 'Noise',
  effects: 'Effects',
  adjust: 'Adjustments',
};

/** Groups that are submenus of the Effects menu; `effects` entries follow
 *  them directly, `adjust` has its own menu. */
export const EFFECT_MENU_GROUPS: EffectGroup[] = ['blur', 'sharpen', 'distort', 'noise'];

export function effectById(id: string): EffectDef | undefined {
  return EFFECTS.find((e) => e.id === id);
}

/** Default value map for a definition (numbers; bools as 0/1; colours as
 *  0xRRGGBBAA, the foreground from `ctx` when given). */
export function defaultValues(def: EffectDef, ctx: EffectContext = {}): Record<string, number> {
  const out: Record<string, number> = {};
  for (const p of def.params) {
    if (p.kind === 'bool') out[p.key] = p.default ? 1 : 0;
    else if (p.kind === 'color') out[p.key] = ctx.foreground ?? p.default;
    else out[p.key] = p.default;
  }
  return out;
}

/** `#RRGGBB` or `#RRGGBBAA` as 0xRRGGBBAA (alpha 255 without `AA`);
 *  `null` for any other text. */
export function parseHexColor(text: string): number | null {
  const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(text.trim());
  if (!m) return null;
  return parseInt(`${m[1]}${m[2] ?? 'ff'}`, 16);
}

/** 0xRRGGBBAA as `#rrggbb`, with `aa` appended unless it is opaque. */
export function formatHexColor(rgba: number): string {
  const v = rgba >>> 0;
  const rgb = (v >>> 8).toString(16).padStart(6, '0');
  const a = v & 0xff;
  return a === 0xff ? `#${rgb}` : `#${rgb}${a.toString(16).padStart(2, '0')}`;
}

/** `#rrggbb` plus an alpha 0–255 as 0xRRGGBBAA (black for a bad hex). */
export function rgbaOf(hex: string, alpha = 255): number {
  const rgb = parseHexColor(hex);
  const base = rgb === null ? 0 : rgb >>> 8;
  return (base * 256 + (Math.round(alpha) & 0xff)) >>> 0;
}

/** Wire parameter vector in the order `pincel_effects::EffectSpec` expects. */
export function toParams(def: EffectDef, values: Record<string, number>): number[] {
  if (def.toParams) return def.toParams(values);
  return def.params.map((p) => {
    const v = values[p.key];
    const fallback = p.kind === 'bool' ? (p.default ? 1 : 0) : p.default;
    return typeof v === 'number' && Number.isFinite(v) ? v : fallback;
  });
}

/**
 * Verify the catalog names exactly the ids the wasm module accepts.
 * Returns the mismatched ids (empty when in sync) so the caller can log
 * once at boot instead of failing hard.
 */
export function assertCatalogCoversAllEffects(wasmNames: string[]): string[] {
  const mine = new Set(EFFECTS.map((e) => e.id));
  const theirs = new Set(wasmNames);
  const missing: string[] = [];
  for (const n of theirs) if (!mine.has(n)) missing.push(`+${n}`);
  for (const n of mine) if (!theirs.has(n)) missing.push(`-${n}`);
  return missing;
}
