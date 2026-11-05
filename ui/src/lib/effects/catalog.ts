// Effect / adjustment catalog for the Effects and Adjustments menus of both
// modes (Pixel and Image share `pincel_effects::EffectSpec` as the wire
// format; spec §15, 2026-10-11). Menu structure and per-mode filter:
// docs/specs/effect-catalog-v2.md (Decision Log 2026-11-04).
//
// Mirrors `pincel_effects::EffectSpec`: every entry names the wasm wire
// id and lists its parameters in the exact order `applyEffect` /
// `previewEffect` expect them. Choice parameters travel as their option
// index, booleans as 0 / 1, a point as two fractions, a linked pair as two
// numbers, a colour as one 0xRRGGBBAA integer (Decision Log 2026-10-29). Keep this table in step with the Rust enum; the
// `every_effect_name_is_accepted` test on the Rust side and
// `assertCatalogCoversAllEffects` below guard the id set from each end.

/** The Effects menu's submenus, in paint.net's order. */
export const EFFECT_SUBMENUS = [
  'artistic',
  'blurs',
  'color',
  'distort',
  'noise',
  'object',
  'photo',
  'render',
  'stylize',
] as const;

/** One submenu of the Effects menu. */
export type EffectSubmenu = (typeof EFFECT_SUBMENUS)[number];

/** Where an entry lives: an Effects submenu, or the Adjustments menu. */
export type EffectGroup = EffectSubmenu | 'adjust';

/** The editor mode a menu is built for. */
export type EffectMode = 'pixel' | 'image';

/** Where a colour parameter starts: the editor's foreground or background
 *  colour when the effect opens, or a fixed `0xRRGGBBAA`. */
export type ColorDefault = 'foreground' | 'background' | number;

/** The editor's colours as `0xRRGGBBAA`, for colour parameter defaults. */
export type EffectColors = { readonly foreground: number; readonly background: number };

/** Opaque black / opaque white: the colours when no editor supplies any. */
export const FALLBACK_COLORS: EffectColors = { foreground: 0x000000ff, background: 0xffffffff };

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
  /** A direction in degrees, 0° right, 90° down (one wire slot). */
  | { kind: 'angle'; key: string; label: string; default: number }
  /** A position in percent of the image area, edited as X / Y; the wire
   *  carries it as two fractions in [0, 1]. */
  | {
      kind: 'point';
      key: string;
      label: string;
      xKey: string;
      yKey: string;
      xLabel: string;
      yLabel: string;
      default: { x: number; y: number };
    }
  /** A random seed with a Reseed button (one wire slot). */
  | { kind: 'seed'; key: string; label: string; min: number; max: number; default: number }
  /** Two numbers with a link toggle (two wire slots; the toggle is UI
   *  state, kept in the value map under `linkKey`). */
  | {
      kind: 'pair';
      key: string;
      label: string;
      first: { key: string; label: string };
      second: { key: string; label: string };
      linkKey: string;
      min: number;
      max: number;
      step: number;
      default: number;
    }
  /** A colour with alpha (one wire slot holding `0xRRGGBBAA`). */
  | { kind: 'color'; key: string; label: string; default: ColorDefault };

export type EffectDef = {
  id: string;
  label: string;
  group: EffectGroup;
  /** The modes whose menus show this entry; the wire format accepts every
   *  effect in both cores regardless. */
  modes: readonly EffectMode[];
  params: ParamDef[];
  /** Parameter defaults that differ per mode, by parameter key (Pixelate's
   *  Colour: Centre in Pixel mode). `entriesFor` applies them. */
  modeDefaults?: Partial<Record<EffectMode, Record<string, number>>>;
  /** Build the wire parameter vector from the current values. Defaults to
   *  the params in declared order; Curves overrides to interleave the
   *  fixed x positions. */
  toParams?: (values: Record<string, number>) => number[];
};

const IMAGE: readonly EffectMode[] = ['image'];
const BOTH: readonly EffectMode[] = ['pixel', 'image'];

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

const angle = (key: string, label: string, def: number): ParamDef => ({
  kind: 'angle',
  key,
  label,
  default: def,
});
const seed = (key: string, label: string, min: number, max: number, def: number): ParamDef => ({
  kind: 'seed',
  key,
  label,
  min,
  max,
  default: def,
});

const color = (key: string, label: string, def: ColorDefault): ParamDef => ({
  kind: 'color',
  key,
  label,
  default: def,
});

const center = (): ParamDef => ({
  kind: 'point',
  key: 'center',
  label: 'Center',
  xKey: 'center_x',
  yKey: 'center_y',
  xLabel: 'Center X',
  yLabel: 'Center Y',
  default: { x: 50, y: 50 },
});

/** The resampling distortions' edge behaviour (one wire slot, the option
 *  index; docs/specs/effects-distort.md). */
const edge = (def = 0): ParamDef =>
  choice('edge', 'Edge behavior', ['Clamp', 'Wrap', 'Reflect', 'Transparent'], def);

/** Radial Blur's amount and centre as wire slots: the amount, then the
 *  centre as two fractions (the point kind's encoding in `toParams`). */
function pointParams(v: Record<string, number>): number[] {
  const read = (key: string, fallback: number): number => {
    const x = v[key];
    return typeof x === 'number' && Number.isFinite(x) ? x : fallback;
  };
  return [read('amount', 10), read('center_x', 50) / 100, read('center_y', 50) / 100];
}

// Curves is exposed as five output sliders at fixed input positions —
// enough for a pixel-art tone tweak without a spline editor.
const CURVE_XS = [0, 64, 128, 192, 255];

export const EFFECTS: EffectDef[] = [
  {
    id: 'gaussian_blur',
    label: 'Gaussian Blur',
    group: 'blurs',
    modes: IMAGE,
    params: [num('radius', 'Radius', 0.1, 250, 2, 0.1)],
  },
  {
    id: 'box_blur',
    label: 'Square Blur',
    group: 'blurs',
    modes: IMAGE,
    params: [
      {
        kind: 'pair',
        key: 'size',
        label: 'Size',
        first: { key: 'width', label: 'Width' },
        second: { key: 'height', label: 'Height' },
        linkKey: 'size_linked',
        min: 1,
        max: 251,
        step: 2,
        default: 3,
      },
    ],
  },
  {
    id: 'motion_blur',
    label: 'Motion Blur',
    group: 'blurs',
    modes: IMAGE,
    params: [
      num('distance', 'Distance', 1, 100, 6),
      angle('angle', 'Angle', 0),
      // docs/specs/effects-blurs.md: off smears only backwards, like a trail.
      bool('centered', 'Centered', true),
    ],
  },
  {
    id: 'radial_blur',
    label: 'Radial Blur',
    group: 'blurs',
    modes: IMAGE,
    params: [num('amount', 'Amount', 1, 100, 10), center()],
    // docs/specs/effects-blurs.md: Radial Blur always spins (zooming is
    // Zoom Blur); the wire keeps its kind slot, sent as 0 = spin.
    toParams: (v) => [...pointParams(v), 0],
  },
  { id: 'sharpen', label: 'Sharpen', group: 'photo', modes: IMAGE, params: [] },
  {
    id: 'unsharp_mask',
    label: 'Unsharp Mask',
    group: 'photo',
    modes: IMAGE,
    params: [
      num('amount', 'Amount %', 1, 500, 50),
      num('radius', 'Radius', 0.1, 50, 1, 0.1),
      num('threshold', 'Threshold', 0, 255, 0),
    ],
  },
  {
    id: 'emboss',
    label: 'Emboss',
    group: 'stylize',
    modes: IMAGE,
    params: [
      angle('angle', 'Angle', 45),
      num('elevation', 'Elevation', 0, 90, 45),
      num('relief', 'Relief', 0.1, 10, 1, 0.1),
    ],
  },
  {
    id: 'edge_detect',
    label: 'Edge Detect',
    group: 'stylize',
    modes: IMAGE,
    params: [
      choice('algorithm', 'Algorithm', ['Sobel', 'Prewitt', 'Laplacian']),
      num('amount', 'Amount %', 1, 400, 100),
    ],
  },
  {
    id: 'relief',
    label: 'Relief',
    group: 'stylize',
    modes: IMAGE,
    params: [angle('angle', 'Angle', 45), num('amount', 'Amount', 0.1, 10, 1, 0.1)],
  },
  {
    id: 'add_noise',
    label: 'Add Noise',
    group: 'noise',
    modes: BOTH,
    params: [
      num('amount', 'Amount', 1, 100, 20),
      choice('noise_type', 'Distribution', ['Uniform', 'Gaussian']),
      choice('channels', 'Channels', ['RGB', 'Monochromatic']),
      seed('seed', 'Seed', 0, 9999, 1),
    ],
  },
  {
    id: 'reduce_noise',
    label: 'Reduce Noise',
    group: 'noise',
    modes: IMAGE,
    params: [num('radius', 'Radius', 1, 10, 1)],
  },
  {
    id: 'brightness_contrast',
    label: 'Brightness / Contrast',
    group: 'adjust',
    modes: BOTH,
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
    modes: BOTH,
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
    modes: BOTH,
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
    modes: BOTH,
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
    modes: BOTH,
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
  { id: 'invert', label: 'Invert Colors', group: 'adjust', modes: BOTH, params: [] },
  {
    id: 'grayscale',
    label: 'Black and White',
    group: 'adjust',
    modes: BOTH,
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
    modes: BOTH,
    params: [num('levels', 'Levels', 2, 64, 4)],
  },
  {
    id: 'threshold',
    label: 'Threshold',
    group: 'adjust',
    modes: BOTH,
    params: [num('threshold', 'Threshold', 0, 255, 128)],
  },
  // docs/specs/adjustments-paintnet.md: paint.net's remaining adjustments.
  { id: 'auto_level', label: 'Auto-Level', group: 'adjust', modes: BOTH, params: [] },
  {
    id: 'exposure',
    label: 'Exposure',
    group: 'adjust',
    modes: BOTH,
    params: [num('exposure', 'Exposure', -5, 5, 0, 0.01)],
  },
  {
    id: 'highlights_shadows',
    label: 'Highlights / Shadows',
    group: 'adjust',
    modes: BOTH,
    params: [
      num('shadows', 'Shadows', -100, 100, 0),
      num('highlights', 'Highlights', -100, 100, 0),
    ],
  },
  { id: 'invert_alpha', label: 'Invert Alpha', group: 'adjust', modes: BOTH, params: [] },
  {
    id: 'sepia',
    label: 'Sepia',
    group: 'adjust',
    modes: BOTH,
    params: [num('intensity', 'Intensity', 0, 100, 100)],
  },
  {
    id: 'temperature_tint',
    label: 'Temperature and Tint',
    group: 'adjust',
    modes: BOTH,
    params: [num('temperature', 'Temperature', -100, 100, 0), num('tint', 'Tint', -100, 100, 0)],
  },
  // docs/specs/effects-blurs.md: paint.net's remaining blurs.
  {
    id: 'bokeh',
    label: 'Bokeh',
    group: 'blurs',
    modes: IMAGE,
    params: [num('radius', 'Radius', 1, 100, 10), num('highlights', 'Highlights', 0, 100, 25)],
  },
  {
    id: 'fragment',
    label: 'Fragment',
    group: 'blurs',
    modes: IMAGE,
    params: [
      num('fragments', 'Fragments', 2, 50, 4),
      num('distance', 'Distance', 0, 100, 8),
      angle('rotation', 'Rotation', 0),
    ],
  },
  {
    id: 'median',
    label: 'Median',
    group: 'blurs',
    modes: IMAGE,
    params: [num('radius', 'Radius', 1, 200, 10), num('percentile', 'Percentile', 0, 100, 50)],
  },
  {
    id: 'sketch_blur',
    label: 'Sketch Blur',
    group: 'blurs',
    modes: IMAGE,
    params: [num('radius', 'Radius', 1, 100, 10), seed('seed', 'Seed', 0, 9999, 1)],
  },
  {
    id: 'surface_blur',
    label: 'Surface Blur',
    group: 'blurs',
    modes: IMAGE,
    params: [num('radius', 'Radius', 1, 100, 6), num('threshold', 'Threshold', 1, 100, 25)],
  },
  {
    id: 'zoom_blur',
    label: 'Zoom Blur',
    group: 'blurs',
    modes: IMAGE,
    params: [num('amount', 'Amount', 1, 100, 10), center()],
  },
  // docs/specs/outline-effect.md: a hard pixel-art outline.
  {
    id: 'outline',
    label: 'Outline Object',
    group: 'object',
    modes: BOTH,
    params: [
      choice('place', 'Place', ['Outside', 'Inside', 'Both']),
      choice('shape', 'Shape', ['Circle', 'Square'], 1),
      num('thickness', 'Thickness', 1, 8, 1),
      color('color', 'Color', 'foreground'),
    ],
  },
  // docs/specs/effects-distort.md: paint.net's Distort menu. Morphology and
  // Pixelate keep pixels hard-edged, so Pixel mode shows them too.
  {
    id: 'bulge',
    label: 'Bulge',
    group: 'distort',
    modes: IMAGE,
    params: [num('amount', 'Amount', -200, 100, 45), center(), edge()],
  },
  {
    id: 'crystalize',
    label: 'Crystalize',
    group: 'distort',
    modes: IMAGE,
    params: [num('cell_size', 'Cell size', 2, 250, 8), seed('seed', 'Seed', 0, 9999, 1)],
  },
  {
    id: 'dents',
    label: 'Dents',
    group: 'distort',
    modes: IMAGE,
    params: [
      num('scale', 'Scale', 1, 200, 25),
      num('refraction', 'Refraction', 0, 200, 50),
      num('roughness', 'Roughness', 0, 100, 10),
      seed('seed', 'Seed', 0, 9999, 1),
      edge(),
    ],
  },
  {
    id: 'frosted_glass',
    label: 'Frosted Glass',
    group: 'distort',
    modes: IMAGE,
    params: [num('amount', 'Amount', 1, 200, 10), seed('seed', 'Seed', 0, 9999, 1)],
  },
  {
    id: 'morphology',
    label: 'Morphology',
    group: 'distort',
    modes: BOTH,
    params: [
      choice('operation', 'Operation', ['Dilate', 'Erode', 'Open', 'Close']),
      choice('shape', 'Shape', ['Square', 'Circle']),
      num('radius', 'Radius', 1, 100, 1),
    ],
  },
  {
    id: 'pixelate',
    label: 'Pixelate',
    group: 'distort',
    modes: BOTH,
    params: [
      num('cell_size', 'Cell size', 2, 200, 8),
      choice('color', 'Color', ['Average', 'Center']),
    ],
    // Centre picks an existing pixel: no new colours in a Pixel-mode sprite.
    modeDefaults: { pixel: { color: 1 } },
  },
  {
    id: 'polar_inversion',
    label: 'Polar Inversion',
    group: 'distort',
    modes: IMAGE,
    params: [num('amount', 'Amount', -4, 4, 1, 0.01), center(), edge(1)],
  },
  {
    id: 'tile_reflection',
    label: 'Tile Reflection',
    group: 'distort',
    modes: IMAGE,
    params: [
      num('tile_size', 'Tile size', 2, 200, 40),
      angle('angle', 'Angle', 30),
      num('curvature', 'Curvature', -100, 100, 8),
      edge(),
    ],
  },
  {
    id: 'twist',
    label: 'Twist',
    group: 'distort',
    modes: IMAGE,
    params: [
      num('amount', 'Amount', -100, 100, 30),
      num('size', 'Size %', 1, 200, 100),
      center(),
      edge(),
    ],
  },
];

/** English fallback names of the Effects submenus (`effect.group.<id>`). */
export const SUBMENU_LABELS: Record<EffectSubmenu, string> = {
  artistic: 'Artistic',
  blurs: 'Blurs',
  color: 'Color',
  distort: 'Distort',
  noise: 'Noise',
  object: 'Object',
  photo: 'Photo',
  render: 'Render',
  stylize: 'Stylize',
};

export function effectById(id: string): EffectDef | undefined {
  return EFFECTS.find((e) => e.id === id);
}

/** The definition as `mode` shows it: its `modeDefaults` for that mode
 *  become the parameters' defaults. */
export function forMode(def: EffectDef, mode: EffectMode): EffectDef {
  const overrides = def.modeDefaults?.[mode];
  if (!overrides) return def;
  const params = def.params.map((p): ParamDef => {
    const v = overrides[p.key];
    if (v === undefined || p.kind === 'bool' || p.kind === 'point') return p;
    return { ...p, default: v };
  });
  return { ...def, params };
}

/** Entries of `group` that `mode`'s menus show, in catalog order, with
 *  that mode's defaults. */
export function entriesFor(group: EffectGroup, mode: EffectMode): EffectDef[] {
  return EFFECTS.filter((e) => e.group === group && e.modes.includes(mode)).map((e) =>
    forMode(e, mode),
  );
}

/** Wraps a typed angle into the dial's range: 0–360 stays, anything
 *  outside wraps around (370 → 10, −10 → 350). */
export function wrapAngle(deg: number): number {
  if (!Number.isFinite(deg)) return 0;
  if (deg >= 0 && deg <= 360) return deg;
  return ((deg % 360) + 360) % 360;
}

/** `true` for a colour value the wire accepts: an integer `0xRRGGBBAA`. */
export function isColorValue(v: unknown): v is number {
  return typeof v === 'number' && Number.isInteger(v) && v >= 0 && v <= 0xffffffff;
}

/** `#RRGGBB` (opaque) or `#RRGGBBAA` as `0xRRGGBBAA`; `null` for any other
 *  text (outline-effect AC-11). */
export function parseHexColor(text: string): number | null {
  const m = /^#([0-9a-f]{6}|[0-9a-f]{8})$/i.exec(text.trim());
  if (!m?.[1]) return null;
  const hex = m[1].length === 6 ? `${m[1]}ff` : m[1];
  return parseInt(hex, 16);
}

/** `0xRRGGBBAA` as lower-case `#rrggbbaa`. */
export function formatHexColor(rgba: number): string {
  return `#${(rgba >>> 0).toString(16).padStart(8, '0')}`;
}

/** A `#rrggbb` colour plus an alpha 0–255 as `0xRRGGBBAA`. */
export function rgbaOf(hex: string, alpha = 255): number {
  const rgb = parseHexColor(hex);
  const a = Math.min(255, Math.max(0, Math.round(alpha)));
  // Arithmetic, not bitwise: `&` would turn a red above 0x7f negative.
  return rgb === null ? a : rgb - (rgb % 256) + a;
}

/** Default value map for a definition (numbers; bools and the link
 *  toggle as 0/1; a point in percent; a colour as `0xRRGGBBAA`, taken from
 *  `colors` where it follows the foreground or background). */
export function defaultValues(
  def: EffectDef,
  colors: EffectColors = FALLBACK_COLORS,
): Record<string, number> {
  const out: Record<string, number> = {};
  for (const p of def.params) {
    switch (p.kind) {
      case 'color':
        out[p.key] = typeof p.default === 'number' ? p.default : colors[p.default];
        break;
      case 'bool':
        out[p.key] = p.default ? 1 : 0;
        break;
      case 'point':
        out[p.xKey] = p.default.x;
        out[p.yKey] = p.default.y;
        break;
      case 'pair':
        out[p.first.key] = p.default;
        out[p.second.key] = p.default;
        out[p.linkKey] = 1;
        break;
      default:
        out[p.key] = p.default;
    }
  }
  return out;
}

/** Wire parameter vector in the order `pincel_effects::EffectSpec` expects. */
export function toParams(def: EffectDef, values: Record<string, number>): number[] {
  if (def.toParams) return def.toParams(values);
  const read = (key: string, fallback: number): number => {
    const v = values[key];
    return typeof v === 'number' && Number.isFinite(v) ? v : fallback;
  };
  return def.params.flatMap((p) => {
    switch (p.kind) {
      case 'bool':
        return [read(p.key, p.default ? 1 : 0)];
      case 'point':
        return [read(p.xKey, p.default.x) / 100, read(p.yKey, p.default.y) / 100];
      case 'pair':
        return [read(p.first.key, p.default), read(p.second.key, p.default)];
      case 'angle':
        return [wrapAngle(read(p.key, p.default))];
      case 'color': {
        const v = values[p.key];
        const fallback = typeof p.default === 'number' ? p.default : FALLBACK_COLORS[p.default];
        return [isColorValue(v) ? v : fallback];
      }
      default:
        return [read(p.key, p.default)];
    }
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
