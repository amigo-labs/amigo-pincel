// Pixel-mode Pencil / Eraser brush: one size / shape / pixel-perfect
// setting both tools share, persisted as the IndexedDB pref `pixelBrush`
// (docs/specs/pixel-pencil.md).

import { getPref, setPref } from '../../lib/idb/prefs';
import { live, toolState } from './stores/editor.svelte';
import type { Tool } from './tools';

/** Footprint of the Pixel-mode brush. */
export type BrushShape = 'square' | 'round';

/** Smallest / largest Pixel-mode brush size. */
export const MIN_BRUSH_SIZE = 1;
export const MAX_BRUSH_SIZE = 8;

/** IndexedDB pref key of the persisted brush. */
const BRUSH_PREF = 'pixelBrush';

/** What the `pixelBrush` pref stores. */
export type BrushPref = { size: number; shape: BrushShape; pixelPerfect: boolean };

/** The tools the brush options apply to. */
export function isBrushTool(tool: Tool): boolean {
  return tool === 'pencil' || tool === 'eraser';
}

/** A typed or stepped size, clamped to the nearest bound of 1–8. */
export function clampBrushSize(n: number): number {
  if (Number.isNaN(n)) return MIN_BRUSH_SIZE;
  return Math.min(MAX_BRUSH_SIZE, Math.max(MIN_BRUSH_SIZE, Math.round(n)));
}

/** Step the shared brush size by `delta`, staying within 1–8. */
export function stepBrushSize(delta: number): void {
  toolState.brushSize = clampBrushSize(toolState.brushSize + delta);
}

/** A stored pref value, or `null` when it is missing or malformed. */
export function parseBrushPref(v: unknown): BrushPref | null {
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (typeof o.size !== 'number' || typeof o.pixelPerfect !== 'boolean') return null;
  if (o.shape !== 'square' && o.shape !== 'round') return null;
  return { size: clampBrushSize(o.size), shape: o.shape, pixelPerfect: o.pixelPerfect };
}

/** Restore the persisted brush (defaults stand when it is missing). */
export async function loadBrushPref(): Promise<void> {
  try {
    const pref = parseBrushPref(await getPref(BRUSH_PREF));
    if (pref) {
      toolState.brushSize = pref.size;
      toolState.brushShape = pref.shape;
      toolState.pixelPerfect = pref.pixelPerfect;
    }
  } finally {
    live.brushPrefLoaded = true;
  }
}

/** Persist `pref` once the initial load settled. */
export function saveBrushPref(pref: BrushPref): void {
  if (!live.brushPrefLoaded) return;
  setPref(BRUSH_PREF, pref).catch((err: unknown) => {
    console.error('setPref pixelBrush failed', err);
  });
}
