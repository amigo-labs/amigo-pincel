// Values of the Import sprite sheet dialog (docs/specs/sprite-sheet-import.md):
// the session defaults (AC-10), field ranges and parsing (AC-12) and the
// background colour's hex form (AC-07). Rune-free, no wasm.

import type { MessageKey } from '../i18n/en';

/** The six grid fields, in this order. */
export const GRID_FIELDS = [
  'cellWidth',
  'cellHeight',
  'offsetX',
  'offsetY',
  'spacingX',
  'spacingY',
] as const;

export type GridField = (typeof GRID_FIELDS)[number];

/** A grid as the core takes it: `[cellW, cellH, offsetX, offsetY, spacingX, spacingY]`. */
export type GridValues = readonly [number, number, number, number, number, number];

/** What the dialog remembers of a confirmed import. */
export interface SheetDefaults {
  grid: GridValues;
  duration: number;
  rowsAsTags: boolean;
}

/** 32 × 32 cells (a common pixel-art cell size), 100 ms (a new frame's duration). */
export const FIRST_DEFAULTS: SheetDefaults = {
  grid: [32, 32, 0, 0, 0, 0],
  duration: 100,
  rowsAsTags: true,
};

/** Largest cell side (File ▸ New's cap). */
export const MAX_CELL = 4096;
/** Frame duration range in ms. */
export const MIN_DURATION = 1;
export const MAX_DURATION = 65535;

// The last confirmed import of this app session; not stored (spec Assumptions).
let last: SheetDefaults | null = null;

/** The values the dialog starts with when no grid is suggested (AC-10). */
export function sheetDefaults(): SheetDefaults {
  return last ?? FIRST_DEFAULTS;
}

/** Remember a confirmed import's values for the next dialog. */
export function rememberSheetDefaults(values: SheetDefaults): void {
  last = values;
}

/** The label key and the allowed range of each grid field on a `w × h` sheet. */
export function fieldRange(
  field: GridField,
  w: number,
  h: number,
): { label: MessageKey; min: number; max: number } {
  switch (field) {
    case 'cellWidth':
      return { label: 'sheetImport.cellWidth', min: 1, max: MAX_CELL };
    case 'cellHeight':
      return { label: 'sheetImport.cellHeight', min: 1, max: MAX_CELL };
    case 'offsetX':
      return { label: 'sheetImport.offsetX', min: 0, max: w - 1 };
    case 'offsetY':
      return { label: 'sheetImport.offsetY', min: 0, max: h - 1 };
    case 'spacingX':
      return { label: 'sheetImport.spacingX', min: 0, max: w };
    case 'spacingY':
      return { label: 'sheetImport.spacingY', min: 0, max: h };
  }
}

/** `text` as a whole number within `min..=max`, else `null`. */
export function parseWhole(text: string, min: number, max: number): number | null {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const n = Number(trimmed);
  return n >= min && n <= max ? n : null;
}

/** `#RRGGBB` (opaque) or `#RRGGBBAA` as `0xRRGGBBAA`, else `null`. */
export function parseHex(text: string): number | null {
  const m = /^#?([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(text.trim());
  if (!m) return null;
  return Number.parseInt(`${m[1]}${m[2] ?? 'ff'}`, 16) >>> 0;
}

/** `0xRRGGBBAA` as `#rrggbb` when opaque, else `#rrggbbaa`. */
export function hexOf(color: number): string {
  const full = (color >>> 0).toString(16).padStart(8, '0');
  return `#${full.endsWith('ff') ? full.slice(0, 6) : full}`;
}

/** The `#rrggbb` part of a colour, for `<input type="color">`. */
export function rgbHexOf(color: number): string {
  return `#${(color >>> 0).toString(16).padStart(8, '0').slice(0, 6)}`;
}

/** The message key for a grid error code of the core (`SheetSummary.error`). */
export function gridErrorKey(code: string): MessageKey | null {
  switch (code) {
    case 'no-cell':
      return 'sheetImport.noCell';
    case 'all-empty':
      return 'sheetImport.allEmpty';
    case 'too-many-frames':
      return 'sheetImport.tooManyFrames';
    default:
      return null;
  }
}

/** The preview size that fits a `w × h` sheet into `boxW × boxH`, at least 1 px. */
export function fitPreview(w: number, h: number, boxW: number, boxH: number): [number, number] {
  const scale = Math.min(boxW / w, boxH / h);
  return [Math.max(1, Math.round(w * scale)), Math.max(1, Math.round(h * scale))];
}

/** The `.aseprite` name of a sheet file: `hero.png` → `hero.aseprite` (AC-15). */
export function documentName(fileName: string): string {
  const dot = fileName.lastIndexOf('.');
  return `${dot > 0 ? fileName.slice(0, dot) : fileName}.aseprite`;
}
