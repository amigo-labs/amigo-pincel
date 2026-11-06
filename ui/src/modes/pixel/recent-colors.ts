// Pixel mode's side of the recent colors (color-picker AC-24): a paint
// change that added an undo step records the packed colors it painted with.

import type { Rgba8 } from '../../lib/color';
import { noteUsed } from '../../lib/colors/recent.svelte';
import { ed } from './stores/editor.svelte';

/** Packed `0xRRGGBBAA` → RGBA bytes. */
export function unpackRgba(p: number): Rgba8 {
  return { r: (p >>> 24) & 0xff, g: (p >>> 16) & 0xff, b: (p >>> 8) & 0xff, a: p & 0xff };
}

/** Records `packed` (foreground first) as painted with when the document
 *  gained an undo step since `depthBefore`, i.e. the change altered pixels. */
export function notePainted(depthBefore: number, packed: readonly number[]): void {
  if (ed.doc && ed.doc.undoDepth > depthBefore) noteUsed('pixel', packed.map(unpackRgba));
}
