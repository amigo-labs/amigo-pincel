// Data the shared layers panel shows (spec §9.2). Each mode maps its own
// layer model onto these rows; the panel never touches a core directly.

import { isMessageKey, t } from '../../i18n/index.svelte';

/** One row of the layers panel, listed top-most first. */
export interface LayerRow {
  /** Stable key across reorders (`{#each}` key, drag target). */
  key: string;
  /** What the mode uses to address the layer: Pixel id, Image index. */
  ref: number;
  name: string;
  visible: boolean;
  /** The layer's own lock flag. */
  locked: boolean;
  /** Locked through a parent group (shown dimmed, toggle disabled). */
  lockedByParent?: boolean;
  /** `0..=1`. */
  opacity: number;
  /** Blend mode id, one of the panel's `blendModes`. */
  blend: string;
  /** Group nesting depth (Pixel mode), `0` at the root. */
  depth?: number;
  /** A group: other layers can be dropped into it. */
  isGroup?: boolean;
  /** Small tag after the name, e.g. the tilemap / group kind. */
  badge?: string;
}

/** Where a dragged layer lands relative to the row it is dropped on. */
export type DropPlace = 'above' | 'below' | 'into';

/** RGBA8 thumbnail pixels, row-major. */
export interface Thumb {
  rgba: Uint8Array | Uint8ClampedArray;
  width: number;
  height: number;
}

/** A blend mode option. */
export interface BlendOption {
  value: string;
  label: string;
}

/** Panel options for blend mode ids, labelled in the UI language. */
export function blendOptions(ids: readonly string[]): BlendOption[] {
  return ids.map((value) => ({ value, label: blendLabel(value) }));
}

/** UI label of a blend mode id (`color_dodge` → “Color Dodge”). */
export function blendLabel(id: string): string {
  const key = `blend.${id}`;
  return isMessageKey(key) ? t(key) : id;
}
