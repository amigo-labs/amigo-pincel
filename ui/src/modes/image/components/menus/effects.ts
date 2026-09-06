// Catalog of effects for the Effects menu and the shared effect dialog
// (spec §11). UI configuration only — the actual math lives in fineliner-effects
// and is reached through the controller.
import type { EffectCommand } from '../../core/wasm';
import { editor } from '../../stores/editor.svelte';

/** A tunable parameter rendered by the effect dialog.
 *
 * `key` names the command field; an optional `index` targets one element of an
 * array-valued field (e.g. a color-balance tone range). */
export type Field =
  | { key: string; index?: number; label: string; kind: 'range'; min: number; max: number; step: number }
  | {
      key: string;
      index?: number;
      label: string;
      kind: 'select';
      options: Array<{ value: string; label: string }>;
    }
  | { key: string; index?: number; label: string; kind: 'toggle' };

/** A single effect: its menu label, dialog title, fields, and default command. */
export interface EffectDef {
  label: string;
  title: string;
  fields: Field[];
  make: () => EffectCommand;
}

/** Effects grouped as they appear in the menu (spec §11.1–§11.4). */
export interface EffectGroup {
  group: string;
  items: EffectDef[];
}

const ANGLE: Field = { key: 'angle', label: 'Angle', kind: 'range', min: 0, max: 360, step: 1 };

export const EFFECT_GROUPS: EffectGroup[] = [
  {
    group: 'Blur',
    items: [
      {
        label: 'Gaussian Blur…',
        title: 'Gaussian Blur',
        fields: [{ key: 'radius', label: 'Radius', kind: 'range', min: 0.1, max: 250, step: 0.1 }],
        make: () => ({ type: 'gaussian_blur', radius: 5 }),
      },
      {
        label: 'Box Blur…',
        title: 'Box Blur',
        fields: [
          { key: 'width', label: 'Width', kind: 'range', min: 1, max: 251, step: 2 },
          { key: 'height', label: 'Height', kind: 'range', min: 1, max: 251, step: 2 },
        ],
        make: () => ({ type: 'box_blur', width: 5, height: 5 }),
      },
      {
        label: 'Motion Blur…',
        title: 'Motion Blur',
        fields: [{ key: 'distance', label: 'Distance', kind: 'range', min: 1, max: 500, step: 1 }, ANGLE],
        make: () => ({ type: 'motion_blur', distance: 20, angle: 0 }),
      },
      {
        label: 'Radial Blur…',
        title: 'Radial Blur',
        fields: [
          { key: 'amount', label: 'Amount', kind: 'range', min: 1, max: 100, step: 1 },
          {
            key: 'kind',
            label: 'Type',
            kind: 'select',
            options: [
              { value: 'spin', label: 'Spin' },
              { value: 'zoom', label: 'Zoom' },
            ],
          },
        ],
        // Center defaults to the canvas centre (spec §11.1); not tunable in the UI yet.
        make: () => ({
          type: 'radial_blur',
          amount: 30,
          center_x: editor.width / 2,
          center_y: editor.height / 2,
          kind: 'spin',
        }),
      },
    ],
  },
  {
    group: 'Sharpen',
    items: [
      {
        label: 'Sharpen',
        title: 'Sharpen',
        fields: [],
        make: () => ({ type: 'sharpen' }),
      },
      {
        label: 'Unsharp Mask…',
        title: 'Unsharp Mask',
        fields: [
          { key: 'amount', label: 'Amount %', kind: 'range', min: 1, max: 500, step: 1 },
          { key: 'radius', label: 'Radius', kind: 'range', min: 0.1, max: 250, step: 0.1 },
          { key: 'threshold', label: 'Threshold', kind: 'range', min: 0, max: 255, step: 1 },
        ],
        make: () => ({ type: 'unsharp_mask', amount: 100, radius: 3, threshold: 0 }),
      },
    ],
  },
  {
    group: 'Distort',
    items: [
      {
        label: 'Emboss…',
        title: 'Emboss',
        fields: [
          ANGLE,
          { key: 'elevation', label: 'Elevation', kind: 'range', min: 0, max: 90, step: 1 },
          { key: 'relief', label: 'Relief', kind: 'range', min: 1, max: 10, step: 1 },
        ],
        make: () => ({ type: 'emboss', angle: 135, elevation: 30, relief: 5 }),
      },
      {
        label: 'Edge Detect…',
        title: 'Edge Detect',
        fields: [
          {
            key: 'algorithm',
            label: 'Algorithm',
            kind: 'select',
            options: [
              { value: 'sobel', label: 'Sobel' },
              { value: 'prewitt', label: 'Prewitt' },
              { value: 'laplacian', label: 'Laplacian' },
            ],
          },
          { key: 'amount', label: 'Amount %', kind: 'range', min: 0, max: 100, step: 1 },
        ],
        make: () => ({ type: 'edge_detect', algorithm: 'sobel', amount: 100 }),
      },
      {
        label: 'Relief…',
        title: 'Relief',
        fields: [ANGLE, { key: 'amount', label: 'Amount', kind: 'range', min: 1, max: 10, step: 1 }],
        make: () => ({ type: 'relief', angle: 135, amount: 5 }),
      },
    ],
  },
  {
    group: 'Noise',
    items: [
      {
        label: 'Add Noise…',
        title: 'Add Noise',
        fields: [
          { key: 'amount', label: 'Amount %', kind: 'range', min: 0, max: 100, step: 1 },
          {
            key: 'noise_type',
            label: 'Type',
            kind: 'select',
            options: [
              { value: 'uniform', label: 'Uniform' },
              { value: 'gaussian', label: 'Gaussian' },
            ],
          },
          {
            key: 'channels',
            label: 'Channels',
            kind: 'select',
            options: [
              { value: 'rgb', label: 'Color' },
              { value: 'monochromatic', label: 'Monochromatic' },
            ],
          },
          { key: 'seed', label: 'Seed', kind: 'range', min: 0, max: 9999, step: 1 },
        ],
        make: () => ({
          type: 'add_noise',
          amount: 25,
          noise_type: 'gaussian',
          channels: 'rgb',
          seed: 0,
        }),
      },
      {
        label: 'Reduce Noise…',
        title: 'Reduce Noise',
        fields: [{ key: 'radius', label: 'Radius', kind: 'range', min: 1, max: 10, step: 1 }],
        make: () => ({ type: 'reduce_noise', radius: 2 }),
      },
    ],
  },
];
