// Canvas sizes for a new document, shared by the start screen's New cards
// and the New-document dialog. Pixel sprites keep the historical 64×64
// default and the 4096 cap; images default to 800×600 and are capped by
// the edge / area limits of the image core's memory (see MAX_CANVAS_DIM /
// clampCanvasSize).
import type { EditorMode, NewDocParams } from './types';
import { clipboardImage, imageSize } from '../clipboard';
import { MAX_CANVAS_DIM, clampCanvasSize } from '../../modes/image/stores/editor.svelte';

/** Largest edge and default size per mode. */
export const NEW_DOC_LIMITS: Record<EditorMode, { max: number; width: number; height: number }> = {
  pixel: { max: 4096, width: 64, height: 64 },
  image: { max: MAX_CANVAS_DIM, width: 800, height: 600 },
};

function clamp(v: number, max: number): number {
  return Math.max(1, Math.min(max, Math.floor(Number.isFinite(v) ? v : 1)));
}

/**
 * The document to create from the size the user typed, clamped to the
 * mode's limits. A cleared field (`null` / `undefined` from the number
 * input) or any non-finite value falls back to the mode's default size.
 */
export function newDocParams(mode: EditorMode, width: number, height: number): NewDocParams {
  const d = NEW_DOC_LIMITS[mode];
  const fw = Number.isFinite(width) ? width : d.width;
  const fh = Number.isFinite(height) ? height : d.height;
  const [w, h] =
    mode === 'image'
      ? clampCanvasSize(fw, fh)
      : [clamp(fw, NEW_DOC_LIMITS.pixel.max), clamp(fh, NEW_DOC_LIMITS.pixel.max)];
  return { mode, width: w, height: h };
}

/**
 * The size of the image on the clipboard as a new image's default, like
 * paint.net (spec §9.2); `null` for Pixel mode, an empty clipboard, or a
 * browser that can't read it without prompting.
 */
export async function clipboardDocSize(mode: EditorMode): Promise<NewDocParams | null> {
  if (mode !== 'image') return null;
  const blob = await clipboardImage({ onlyIfAllowed: true });
  const size = blob ? await imageSize(blob) : null;
  return size ? newDocParams(mode, size.width, size.height) : null;
}
