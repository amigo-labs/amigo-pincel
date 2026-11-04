import { expect, test } from '@playwright/test';
import {
  defaultValues,
  EFFECT_SUBMENUS,
  EFFECTS,
  effectById,
  entriesFor,
  toParams,
  wrapAngle,
} from '../src/lib/effects/catalog';
import { de } from '../src/lib/i18n/de';
import { en } from '../src/lib/i18n/en';

// effect-catalog-v2 AC-07: the restructured catalog sends exactly the wire
// vectors the previous catalog sent for the same control values, so every
// applied effect gives the same pixels as before. The vectors below are the
// previous catalog's output, written out by hand; each change sets one
// control (a value key of the effect's dialog) and leaves the rest at their
// defaults. A point is edited in percent, the previous catalog took the same
// position as a fraction. No browser page: the catalog is plain data.

type Change = readonly [key: string, value: number, wire: readonly number[]];
type Pin = { readonly defaults: readonly number[]; readonly changes: readonly Change[] };

const c = (x: number): number => x / 255;
const CURVES_DEFAULT = [0, 0, 0, c(64), c(64), c(128), c(128), c(192), c(192), 1, 1];

const PINS: Record<string, Pin> = {
  gaussian_blur: { defaults: [2], changes: [['radius', 5, [5]]] },
  box_blur: {
    defaults: [3, 3],
    changes: [
      ['width', 5, [5, 3]],
      ['height', 9, [3, 9]],
    ],
  },
  // effects-blurs appends the Centered slot (1 = on, the previous
  // behaviour) to the previous vectors.
  motion_blur: {
    defaults: [6, 0, 1],
    changes: [
      ['distance', 10, [10, 0, 1]],
      ['angle', 90, [6, 90, 1]],
      ['centered', 0, [6, 0, 0]],
    ],
  },
  // effects-blurs hides the Kind choice: Radial Blur always spins (kind 0).
  radial_blur: {
    defaults: [10, 0.5, 0.5, 0],
    changes: [
      ['amount', 20, [20, 0.5, 0.5, 0]],
      ['center_x', 30, [10, 0.3, 0.5, 0]],
      ['center_y', 70, [10, 0.5, 0.7, 0]],
    ],
  },
  sharpen: { defaults: [], changes: [] },
  unsharp_mask: {
    defaults: [50, 1, 0],
    changes: [
      ['amount', 120, [120, 1, 0]],
      ['radius', 2.5, [50, 2.5, 0]],
      ['threshold', 10, [50, 1, 10]],
    ],
  },
  emboss: {
    defaults: [45, 45, 1],
    changes: [
      ['angle', 200, [200, 45, 1]],
      ['elevation', 30, [45, 30, 1]],
      ['relief', 2, [45, 45, 2]],
    ],
  },
  edge_detect: {
    defaults: [0, 100],
    changes: [
      ['algorithm', 2, [2, 100]],
      ['amount', 250, [0, 250]],
    ],
  },
  relief: {
    defaults: [45, 1],
    changes: [
      ['angle', 300, [300, 1]],
      ['amount', 3, [45, 3]],
    ],
  },
  add_noise: {
    defaults: [20, 0, 0, 1],
    changes: [
      ['amount', 50, [50, 0, 0, 1]],
      ['noise_type', 1, [20, 1, 0, 1]],
      ['channels', 1, [20, 0, 1, 1]],
      ['seed', 4242, [20, 0, 0, 4242]],
    ],
  },
  reduce_noise: { defaults: [1], changes: [['radius', 4, [4]]] },
  brightness_contrast: {
    defaults: [0, 0, 0],
    changes: [
      ['brightness', 40, [40, 0, 0]],
      ['contrast', -20, [0, -20, 0]],
      ['enhanced', 1, [0, 0, 1]],
    ],
  },
  hue_saturation: {
    defaults: [0, 0, 0, 0],
    changes: [
      ['hue', 90, [90, 0, 0, 0]],
      ['saturation', -50, [0, -50, 0, 0]],
      ['lightness', 25, [0, 0, 25, 0]],
      ['colorize', 1, [0, 0, 0, 1]],
    ],
  },
  curves: {
    defaults: CURVES_DEFAULT,
    changes: [
      ['channel', 2, [2, ...CURVES_DEFAULT.slice(1)]],
      ['y0', 10, [0, 0, c(10), ...CURVES_DEFAULT.slice(3)]],
      ['y128', 200, [...CURVES_DEFAULT.slice(0, 6), c(200), ...CURVES_DEFAULT.slice(7)]],
      ['y255', 100, [...CURVES_DEFAULT.slice(0, 10), c(100)]],
    ],
  },
  levels: {
    defaults: [0, 0, 1, 1, 0, 1],
    changes: [
      ['channel', 3, [3, 0, 1, 1, 0, 1]],
      ['in_black', 20, [0, c(20), 1, 1, 0, 1]],
      ['in_white', 200, [0, 0, c(200), 1, 0, 1]],
      ['gamma', 1.5, [0, 0, 1, 1.5, 0, 1]],
      ['out_black', 30, [0, 0, 1, 1, c(30), 1]],
      ['out_white', 220, [0, 0, 1, 1, 0, c(220)]],
    ],
  },
  color_balance: {
    defaults: [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    changes: [
      ['s_cr', 10, [10, 0, 0, 0, 0, 0, 0, 0, 0, 1]],
      ['m_mg', -30, [0, 0, 0, 0, -30, 0, 0, 0, 0, 1]],
      ['h_yb', 50, [0, 0, 0, 0, 0, 0, 0, 0, 50, 1]],
      ['preserve_luminosity', 0, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]],
    ],
  },
  invert: { defaults: [], changes: [] },
  grayscale: {
    defaults: [0, 0.3, 0.59, 0.11],
    changes: [
      ['method', 3, [3, 0.3, 0.59, 0.11]],
      ['r', 0.5, [0, 0.5, 0.59, 0.11]],
      ['g', 0.2, [0, 0.3, 0.2, 0.11]],
      ['b', 0.9, [0, 0.3, 0.59, 0.9]],
    ],
  },
  posterize: { defaults: [4], changes: [['levels', 8, [8]]] },
  threshold: { defaults: [128], changes: [['threshold', 60, [60]]] },
  // Added by adjustments-paintnet (no previous catalog): the table's order
  // and defaults.
  auto_level: { defaults: [], changes: [] },
  exposure: { defaults: [0], changes: [['exposure', 1.25, [1.25]]] },
  highlights_shadows: {
    defaults: [0, 0],
    changes: [
      ['shadows', 40, [40, 0]],
      ['highlights', -25, [0, -25]],
    ],
  },
  invert_alpha: { defaults: [], changes: [] },
  sepia: { defaults: [100], changes: [['intensity', 30, [30]]] },
  temperature_tint: {
    defaults: [0, 0],
    changes: [
      ['temperature', 70, [70, 0]],
      ['tint', -60, [0, -60]],
    ],
  },
  // Added by effects-blurs (no previous catalog): the table's order and
  // defaults; a point in fractions, an angle in degrees.
  bokeh: {
    defaults: [10, 25],
    changes: [
      ['radius', 40, [40, 25]],
      ['highlights', 90, [10, 90]],
    ],
  },
  fragment: {
    defaults: [4, 8, 0],
    changes: [
      ['fragments', 12, [12, 8, 0]],
      ['distance', 0, [4, 0, 0]],
      ['rotation', 370, [4, 8, 10]],
    ],
  },
  median: {
    defaults: [10, 50],
    changes: [
      ['radius', 150, [150, 50]],
      ['percentile', 0, [10, 0]],
    ],
  },
  sketch_blur: {
    defaults: [10, 1],
    changes: [
      ['radius', 30, [30, 1]],
      ['seed', 777, [10, 777]],
    ],
  },
  surface_blur: {
    defaults: [6, 25],
    changes: [
      ['radius', 12, [12, 25]],
      ['threshold', 60, [6, 60]],
    ],
  },
  zoom_blur: {
    defaults: [10, 0.5, 0.5],
    changes: [
      ['amount', 55, [55, 0.5, 0.5]],
      ['center_x', 0, [10, 0, 0.5]],
      ['center_y', 100, [10, 0.5, 1]],
    ],
  },
  // Added by effects-distort (no previous catalog): the table's order and
  // defaults; choices as option index, the edge behaviour last
  // (0 = Clamp, 1 = Wrap, 2 = Reflect, 3 = Transparent).
  bulge: {
    defaults: [45, 0.5, 0.5, 0],
    changes: [
      ['amount', -200, [-200, 0.5, 0.5, 0]],
      ['center_x', 25, [45, 0.25, 0.5, 0]],
      ['edge', 3, [45, 0.5, 0.5, 3]],
    ],
  },
  crystalize: {
    defaults: [8, 1],
    changes: [
      ['cell_size', 250, [250, 1]],
      ['seed', 42, [8, 42]],
    ],
  },
  dents: {
    defaults: [25, 50, 10, 1, 0],
    changes: [
      ['scale', 200, [200, 50, 10, 1, 0]],
      ['refraction', 0, [25, 0, 10, 1, 0]],
      ['roughness', 100, [25, 50, 100, 1, 0]],
      ['seed', 9, [25, 50, 10, 9, 0]],
      ['edge', 2, [25, 50, 10, 1, 2]],
    ],
  },
  frosted_glass: {
    defaults: [10, 1],
    changes: [
      ['amount', 200, [200, 1]],
      ['seed', 0, [10, 0]],
    ],
  },
  morphology: {
    defaults: [0, 0, 1],
    changes: [
      ['operation', 3, [3, 0, 1]],
      ['shape', 1, [0, 1, 1]],
      ['radius', 100, [0, 0, 100]],
    ],
  },
  // Image mode's defaults (Average); Pixel mode's Centre is pinned below.
  pixelate: {
    defaults: [8, 0],
    changes: [
      ['cell_size', 2, [2, 0]],
      ['color', 1, [8, 1]],
    ],
  },
  polar_inversion: {
    defaults: [1, 0.5, 0.5, 1],
    changes: [
      ['amount', -3.5, [-3.5, 0.5, 0.5, 1]],
      ['center_y', 0, [1, 0.5, 0, 1]],
      ['edge', 0, [1, 0.5, 0.5, 0]],
    ],
  },
  tile_reflection: {
    defaults: [40, 30, 8, 0],
    changes: [
      ['tile_size', 2, [2, 30, 8, 0]],
      ['angle', 370, [40, 10, 8, 0]],
      ['curvature', -100, [40, 30, -100, 0]],
      ['edge', 1, [40, 30, 8, 1]],
    ],
  },
  twist: {
    defaults: [30, 100, 0.5, 0.5, 0],
    changes: [
      ['amount', -100, [-100, 100, 0.5, 0.5, 0]],
      ['size', 200, [30, 200, 0.5, 0.5, 0]],
      ['center_x', 10, [30, 100, 0.1, 0.5, 0]],
      ['edge', 3, [30, 100, 0.5, 0.5, 3]],
    ],
  },
};

test('every catalog entry has pinned vectors of the previous catalog', () => {
  expect(EFFECTS.map((e) => e.id).sort()).toEqual(Object.keys(PINS).sort());
});

for (const [id, pin] of Object.entries(PINS)) {
  test(`${id}: wire vectors equal the previous catalog's`, () => {
    const def = effectById(id);
    if (!def) throw new Error(`${id} missing from the catalog`);
    const defaults = defaultValues(def);
    expect(toParams(def, defaults)).toEqual(pin.defaults);
    for (const [key, value, wire] of pin.changes) {
      expect(key in defaults, `${id} has a control ${key}`).toBe(true);
      const params = toParams(def, { ...defaults, [key]: value });
      expect(params.length, `${id}.${key}`).toBe(wire.length);
      params.forEach((p, i) => expect(p, `${id}.${key} slot ${i}`).toBeCloseTo(wire[i] ?? NaN, 9));
    }
  });
}

// effects-distort AC-02: Pixelate's Colour starts at Centre in Pixel mode
// (no new colours), at Average in Image mode; the menus hand out each mode's
// definition.
test('pixelate: each mode starts at its own colour default', () => {
  const inMode = (mode: 'pixel' | 'image') => {
    const def = entriesFor('distort', mode).find((e) => e.id === 'pixelate');
    if (!def) throw new Error(`pixelate missing from ${mode} mode`);
    return toParams(def, defaultValues(def));
  };
  expect(inMode('pixel')).toEqual([8, 1]);
  expect(inMode('image')).toEqual([8, 0]);
  expect(entriesFor('distort', 'pixel').map((e) => e.id)).toEqual(['morphology', 'pixelate']);
});

// AC-01 / AC-06: the nine submenus in paint.net's order and their names;
// menus only show the non-empty ones, so the full list is checked here.
test('submenus in paint.net order with English and German names', () => {
  const name = (dict: Record<string, string>) =>
    EFFECT_SUBMENUS.map((g) => dict[`effect.group.${g}`]);
  expect(name(en)).toEqual([
    'Artistic',
    'Blurs',
    'Color',
    'Distort',
    'Noise',
    'Object',
    'Photo',
    'Render',
    'Stylize',
  ]);
  expect(name(de)).toEqual([
    'Künstlerisch',
    'Weichzeichnen',
    'Farbe',
    'Verzerren',
    'Rauschen',
    'Objekt',
    'Foto',
    'Rendern',
    'Stilisieren',
  ]);
  expect(en['effect.box_blur']).toBe('Square Blur');
  expect(de['effect.box_blur']).toBe('Quadratisches Weichzeichnen');
});

// AC-09: a typed angle outside 0–360 wraps around.
test('wrapAngle wraps typed angles outside 0–360', () => {
  expect(wrapAngle(370)).toBe(10);
  expect(wrapAngle(-10)).toBe(350);
  expect(wrapAngle(0)).toBe(0);
  expect(wrapAngle(360)).toBe(360);
  expect(wrapAngle(725)).toBe(5);
});
