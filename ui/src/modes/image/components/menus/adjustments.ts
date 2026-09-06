// Catalog of adjustments for the Adjustments menu (spec §12). Reuses the effect
// dialog's field/definition types; adjustments are layer-targeting effects too.
import type { EffectCommand } from '../../core/wasm';
import type { EffectGroup } from './effects';

const CONTRAST_PRESET: Array<[number, number]> = [
  [0, 0],
  [0.25, 0.13],
  [0.5, 0.5],
  [0.75, 0.87],
  [1, 1],
];

/** Curves presets (spec §12.3) — Phase 1 exposes presets, not a curve editor. */
function curves(points: Array<[number, number]>): () => EffectCommand {
  return () => ({ type: 'curves', channel: 'composite', points });
}

export const ADJUSTMENT_GROUPS: EffectGroup[] = [
  {
    group: 'Tone',
    items: [
      {
        label: 'Brightness / Contrast…',
        title: 'Brightness / Contrast',
        fields: [
          { key: 'brightness', label: 'Brightness', kind: 'range', min: -150, max: 150, step: 1 },
          { key: 'contrast', label: 'Contrast', kind: 'range', min: -150, max: 150, step: 1 },
          { key: 'enhanced', label: 'Enhanced', kind: 'toggle' },
        ],
        make: () => ({ type: 'brightness_contrast', brightness: 0, contrast: 0, enhanced: false }),
      },
      {
        label: 'Levels…',
        title: 'Levels',
        fields: [
          {
            key: 'channel',
            label: 'Channel',
            kind: 'select',
            options: [
              { value: 'composite', label: 'RGB' },
              { value: 'red', label: 'Red' },
              { value: 'green', label: 'Green' },
              { value: 'blue', label: 'Blue' },
            ],
          },
          { key: 'in_black', label: 'In black', kind: 'range', min: 0, max: 1, step: 0.01 },
          { key: 'gamma', label: 'Gamma', kind: 'range', min: 0.1, max: 3, step: 0.01 },
          { key: 'in_white', label: 'In white', kind: 'range', min: 0, max: 1, step: 0.01 },
          { key: 'out_black', label: 'Out black', kind: 'range', min: 0, max: 1, step: 0.01 },
          { key: 'out_white', label: 'Out white', kind: 'range', min: 0, max: 1, step: 0.01 },
        ],
        make: () => ({
          type: 'levels',
          channel: 'composite',
          in_black: 0,
          in_white: 1,
          gamma: 1,
          out_black: 0,
          out_white: 1,
        }),
      },
      {
        label: 'Curves: Increase Contrast',
        title: 'Curves — Increase Contrast',
        fields: [],
        make: curves(CONTRAST_PRESET),
      },
      {
        label: 'Curves: Lighten',
        title: 'Curves — Lighten',
        fields: [],
        make: curves([
          [0, 0],
          [0.5, 0.65],
          [1, 1],
        ]),
      },
      {
        label: 'Curves: Darken',
        title: 'Curves — Darken',
        fields: [],
        make: curves([
          [0, 0],
          [0.5, 0.35],
          [1, 1],
        ]),
      },
    ],
  },
  {
    group: 'Color',
    items: [
      {
        label: 'Hue / Saturation…',
        title: 'Hue / Saturation',
        fields: [
          { key: 'hue', label: 'Hue', kind: 'range', min: -180, max: 180, step: 1 },
          { key: 'saturation', label: 'Saturation', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'lightness', label: 'Lightness', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'colorize', label: 'Colorize', kind: 'toggle' },
        ],
        make: () => ({
          type: 'hue_saturation',
          hue: 0,
          saturation: 0,
          lightness: 0,
          colorize: false,
        }),
      },
      {
        label: 'Color Balance…',
        title: 'Color Balance',
        fields: [
          { key: 'shadows', index: 0, label: 'Shadows R', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'shadows', index: 1, label: 'Shadows G', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'shadows', index: 2, label: 'Shadows B', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'midtones', index: 0, label: 'Midtones R', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'midtones', index: 1, label: 'Midtones G', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'midtones', index: 2, label: 'Midtones B', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'highlights', index: 0, label: 'Highlights R', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'highlights', index: 1, label: 'Highlights G', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'highlights', index: 2, label: 'Highlights B', kind: 'range', min: -100, max: 100, step: 1 },
          { key: 'preserve_luminosity', label: 'Preserve luminosity', kind: 'toggle' },
        ],
        make: () => ({
          type: 'color_balance',
          shadows: [0, 0, 0],
          midtones: [0, 0, 0],
          highlights: [0, 0, 0],
          preserve_luminosity: false,
        }),
      },
      {
        label: 'Grayscale',
        title: 'Grayscale',
        fields: [
          {
            key: 'method',
            label: 'Method',
            kind: 'select',
            options: [
              { value: 'luminosity', label: 'Luminosity' },
              { value: 'average', label: 'Average' },
              { value: 'bt709', label: 'BT.709' },
            ],
          },
        ],
        make: () => ({ type: 'grayscale', method: 'luminosity', mixer: [0.299, 0.587, 0.114] }),
      },
    ],
  },
  {
    group: 'Stylize',
    items: [
      {
        label: 'Invert',
        title: 'Invert',
        fields: [],
        make: () => ({ type: 'invert' }),
      },
      {
        label: 'Posterize…',
        title: 'Posterize',
        fields: [{ key: 'levels', label: 'Levels', kind: 'range', min: 2, max: 64, step: 1 }],
        make: () => ({ type: 'posterize', levels: 8 }),
      },
      {
        label: 'Threshold…',
        title: 'Threshold',
        fields: [{ key: 'threshold', label: 'Threshold', kind: 'range', min: 0, max: 255, step: 1 }],
        make: () => ({ type: 'threshold', threshold: 128 }),
      },
    ],
  },
];
