// What the New size form tells about its fields, and its presets
// (docs/specs/new-document-form.md). Plain functions over the limits, so the
// start screen's New cards and the New dialog say the same.

import type { MessageKey } from '../i18n/en';
import type { EditorMode } from './types';

/** A note under a size field: a message and its parameters. */
export interface SizeNote {
  key: MessageKey;
  params: Record<string, string | number>;
}

/** The size limits of one mode (`NEW_DOC_LIMITS`). */
export interface SizeLimits {
  max: number;
  width: number;
  height: number;
}

/** Common sizes per mode, in this order (AC-02). */
export const SIZE_PRESETS: Readonly<Record<EditorMode, readonly (readonly [number, number])[]>> = {
  pixel: [
    [16, 16],
    [32, 32],
    [64, 64],
    [128, 128],
    [256, 256],
  ],
  image: [
    [800, 600],
    [1280, 720],
    [1920, 1080],
    [1080, 1080],
  ],
};

/** The edge Create uses for `v` before the image area cap. */
export function edgeOf(mode: EditorMode, v: number | null, limits: SizeLimits, fallback: number): number {
  if (v === null || !Number.isFinite(v)) return fallback;
  const r = mode === 'image' ? Math.round(v) : Math.floor(v);
  return Math.max(1, Math.min(limits.max, r));
}

/** Why the clamp changes a field's value; `n` is the edge Create uses. */
export function fieldNote(v: number | null, bad: boolean, limits: SizeLimits, n: number): SizeNote | null {
  if (bad) return { key: 'newDoc.notNumber', params: { n } };
  if (v === null || !Number.isFinite(v)) return { key: 'newDoc.empty', params: { n } };
  if (v < 1) return { key: 'newDoc.tooSmall', params: { n } };
  if (v > limits.max) return { key: 'newDoc.tooLarge', params: { max: limits.max, n } };
  if (!Number.isInteger(v)) return { key: 'newDoc.notWhole', params: { n } };
  return null;
}

/** The note for a size the image area limit cuts down, or `null`. */
export function areaNote(
  edges: { width: number; height: number },
  result: { width: number; height: number },
  maxArea: number,
): SizeNote | null {
  if (result.width === edges.width && result.height === edges.height) return null;
  const side = Math.round(Math.sqrt(maxArea));
  return {
    key: 'newDoc.tooManyPixels',
    params: { limit: `${side} × ${side}`, w: result.width, h: result.height },
  };
}
