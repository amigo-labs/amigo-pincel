// Pixel grid and custom grid (docs/specs/pixel-grid.md, Decision Log
// 2026-10-29). Both are view decoration drawn on their own stage-sized
// canvas over the sprite — not the sprite-sized overlay, whose CSS upscale
// would make a line one sprite pixel wide — so lines stay one device pixel
// at every zoom. The grid's cell size and offset are sprite state (wasm
// `gridX` … `setGrid`); the show flags, line colour and opacity are the
// per-user IndexedDB pref `pixelGrid`, never written into a file.

import { isIdbAvailable } from '../../lib/idb/db';
import { getPref, setPref } from '../../lib/idb/prefs';
import { ed, live, view } from './stores/editor.svelte';

/** IndexedDB pref key of the grid view settings. */
export const GRID_PREF = 'pixelGrid';

/** The pixel grid shows from this zoom (screen px per sprite px) up. */
export const PIXEL_GRID_MIN_ZOOM = 8;

/** Default custom-grid line colour and opacity (%). */
export const DEFAULT_GRID_COLOR = '#0000ff';
export const DEFAULT_GRID_OPACITY = 60;

/** What the `pixelGrid` pref stores. */
export interface GridPref {
  showPixelGrid: boolean;
  showGrid: boolean;
  /** Custom-grid line colour, `#rrggbb`. */
  color: string;
  /** Custom-grid line opacity, 0–100 %. */
  opacity: number;
}

/** A stored pref value, or `null` when it is missing or malformed. */
export function parseGridPref(v: unknown): GridPref | null {
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (typeof o.showPixelGrid !== 'boolean' || typeof o.showGrid !== 'boolean') return null;
  if (typeof o.color !== 'string' || !/^#[0-9a-f]{6}$/i.test(o.color)) return null;
  if (typeof o.opacity !== 'number' || !Number.isFinite(o.opacity)) return null;
  return {
    showPixelGrid: o.showPixelGrid,
    showGrid: o.showGrid,
    color: o.color.toLowerCase(),
    opacity: Math.min(100, Math.max(0, Math.round(o.opacity))),
  };
}

/** Restore the stored grid view settings (defaults stand without them). */
export async function loadGridPref(): Promise<void> {
  try {
    if (!isIdbAvailable()) return;
    const pref = parseGridPref(await getPref(GRID_PREF));
    if (pref) {
      view.showPixelGrid = pref.showPixelGrid;
      view.showGrid = pref.showGrid;
      view.gridColor = pref.color;
      view.gridOpacity = pref.opacity;
    }
  } catch (err) {
    console.error('getPref pixelGrid failed', err);
  } finally {
    live.gridPrefLoaded = true;
  }
}

/** Persist the current grid view settings once the initial load settled. */
export function saveGridPref(): void {
  if (!live.gridPrefLoaded || !isIdbAvailable()) return;
  const pref: GridPref = {
    showPixelGrid: view.showPixelGrid,
    showGrid: view.showGrid,
    color: view.gridColor,
    opacity: view.gridOpacity,
  };
  setPref(GRID_PREF, pref).catch((err: unknown) => {
    console.error('setPref pixelGrid failed', err);
  });
}

/** View ▸ Show Pixel Grid. */
export function togglePixelGrid(): void {
  view.showPixelGrid = !view.showPixelGrid;
  saveGridPref();
}

/** View ▸ Show Grid. */
export function toggleGrid(): void {
  view.showGrid = !view.showGrid;
  saveGridPref();
}

/** The sprite's grid geometry in sprite pixels. */
export interface GridCells {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** The open document's grid, or `null` without a document. */
export function docGrid(): GridCells | null {
  const d = ed.doc;
  if (!d) return null;
  return { x: d.gridX, y: d.gridY, w: d.gridWidth, h: d.gridHeight };
}

/**
 * The positions `offset + k × cell` (any integer k) that fall in
 * `[from, to]`, ascending. A cell size of 0 has no lines.
 */
export function gridLines(offset: number, cell: number, from: number, to: number): number[] {
  if (cell <= 0 || to < from) return [];
  const first = offset + Math.ceil((from - offset) / cell) * cell;
  const out: number[] = [];
  for (let p = first; p <= to; p += cell) out.push(p);
  return out;
}

/** `#rrggbb` plus opacity (%) as a canvas colour. */
function rgba(hex: string, opacity: number): string {
  const n = Number.parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${opacity / 100})`;
}

let pixelGridColor: string | null = null;

/** The low-contrast theme colour of the pixel grid (read once). */
function pixelGridStroke(): string {
  if (pixelGridColor === null) {
    const faint = getComputedStyle(document.documentElement)
      .getPropertyValue('--color-pc-faint')
      .trim();
    pixelGridColor = /^#[0-9a-f]{6}$/i.test(faint) ? rgba(faint, 45) : 'rgba(110, 110, 120, 0.45)';
  }
  return pixelGridColor;
}

/** Everything that changes the grid canvas, as one comparable key. */
function gridKey(cells: GridCells | null): string {
  const dpr = window.devicePixelRatio || 1;
  return [
    ed.canvasW,
    ed.canvasH,
    view.zoom,
    view.panX,
    view.panY,
    view.stageW,
    view.stageH,
    dpr,
    view.showPixelGrid,
    view.showGrid,
    view.gridColor,
    view.gridOpacity,
    cells ? `${cells.x},${cells.y},${cells.w},${cells.h}` : '-',
  ].join('|');
}

/** Repaint the grid canvas when anything it shows changed (called per frame). */
export function paintGridIfChanged(): void {
  const canvas = view.gridCanvas;
  if (!canvas) return;
  const cells = docGrid();
  const key = gridKey(cells);
  if (key === live.gridKey) return;
  live.gridKey = key;
  paintGrid(canvas, cells);
}

/**
 * Clear the stage-sized `canvas` and draw the pixel grid (zoom ≥ 8) and
 * the custom grid over it, only across the visible part of the sprite and
 * snapped to device pixels.
 */
function paintGrid(canvas: HTMLCanvasElement, cells: GridCells | null): void {
  const sprite = view.canvas;
  const dpr = window.devicePixelRatio || 1;
  const box = canvas.getBoundingClientRect();
  const w = Math.max(1, Math.round(box.width * dpr));
  const h = Math.max(1, Math.round(box.height * dpr));
  if (canvas.width !== w) canvas.width = w;
  if (canvas.height !== h) canvas.height = h;
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.clearRect(0, 0, w, h);
  if (!sprite || !cells || ed.canvasW <= 0 || ed.canvasH <= 0) return;
  const showPixel = view.showPixelGrid && view.zoom >= PIXEL_GRID_MIN_ZOOM;
  const showCustom = view.showGrid && cells.w > 0 && cells.h > 0;
  if (!showPixel && !showCustom) return;

  // Sprite → device pixels, from the sprite canvas's on-screen box.
  const s = sprite.getBoundingClientRect();
  const sx = s.width / ed.canvasW;
  const sy = s.height / ed.canvasH;
  const toDevX = (x: number) => Math.round((s.left - box.left + x * sx) * dpr);
  const toDevY = (y: number) => Math.round((s.top - box.top + y * sy) * dpr);
  // The visible part of the sprite, in sprite pixels.
  const x0 = Math.max(0, Math.floor((box.left - s.left) / sx));
  const x1 = Math.min(ed.canvasW, Math.ceil((box.right - s.left) / sx));
  const y0 = Math.max(0, Math.floor((box.top - s.top) / sy));
  const y1 = Math.min(ed.canvasH, Math.ceil((box.bottom - s.top) / sy));
  if (x1 <= x0 || y1 <= y0) return;
  const top = Math.max(0, toDevY(0));
  const bottom = Math.min(h, toDevY(ed.canvasH));
  const left = Math.max(0, toDevX(0));
  const right = Math.min(w, toDevX(ed.canvasW));

  // One device pixel per line: a fill of 1 px wide / high rects is crisp
  // without half-pixel stroke offsets.
  const drawX = (xs: number[]) => {
    for (const x of xs) {
      const dx = toDevX(x);
      if (dx >= 0 && dx < w) ctx.fillRect(dx, top, 1, bottom - top);
    }
  };
  const drawY = (ys: number[]) => {
    for (const y of ys) {
      const dy = toDevY(y);
      if (dy >= 0 && dy < h) ctx.fillRect(left, dy, right - left, 1);
    }
  };

  if (showPixel) {
    ctx.fillStyle = pixelGridStroke();
    drawX(gridLines(0, 1, Math.max(1, x0), Math.min(ed.canvasW - 1, x1)));
    drawY(gridLines(0, 1, Math.max(1, y0), Math.min(ed.canvasH - 1, y1)));
  }
  // The custom grid over the pixel grid (AC-14). Lines on the far sprite
  // edge would fall outside the sprite, so they end at `size − 1`.
  if (showCustom) {
    ctx.fillStyle = rgba(view.gridColor, view.gridOpacity);
    drawX(gridLines(cells.x, cells.w, x0, Math.min(ed.canvasW - 1, x1)));
    drawY(gridLines(cells.y, cells.h, y0, Math.min(ed.canvasH - 1, y1)));
  }
}
