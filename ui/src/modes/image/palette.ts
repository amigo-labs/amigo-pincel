// Image mode's Colors-panel palette (docs/specs/image-palette-gradient.md):
// the entry shape, its stored form and the rules for Add color. App state
// for Image mode as a whole, stored in the IndexedDB pref `palette:image`
// (never localStorage, AGENTS.md §9) and never on a document's undo stack.
// Rune-free, so Node-side specs can import it.

import { DEFAULT_PALETTE, sameColor, toHex, type Rgba8 } from '../../lib/color';

/** IndexedDB pref holding the palette once the user loaded or extended it. */
export const PALETTE_PREF = 'palette:image';

/** Most entries a palette holds (the palette file codecs' limit). */
export const PALETTE_MAX = 256;

/** One palette entry: its color and, from a `.gpl` file, its name. */
export interface PaletteEntry {
  readonly color: Rgba8;
  readonly name?: string;
}

/** An entry as stored: packed `0xRRGGBBAA` plus the optional name. */
export interface StoredEntry {
  rgba: number;
  name?: string;
}

/** The built-in default palette (color-picker AC-27) as entries. */
export const DEFAULT_ENTRIES: readonly PaletteEntry[] = DEFAULT_PALETTE.map((color) => ({
  color,
}));

/** `c` as packed `0xRRGGBBAA`. */
export function packRgba(c: Rgba8): number {
  return ((c.r << 24) | (c.g << 16) | (c.b << 8) | c.a) >>> 0;
}

/** Packed `0xRRGGBBAA` as a color. */
export function unpackRgba(v: number): Rgba8 {
  return { r: (v >>> 24) & 0xff, g: (v >>> 16) & 0xff, b: (v >>> 8) & 0xff, a: v & 0xff };
}

/** `entries` in their stored form. */
export function toStored(entries: readonly PaletteEntry[]): StoredEntry[] {
  return entries.map((e) =>
    e.name ? { rgba: packRgba(e.color), name: e.name } : { rgba: packRgba(e.color) },
  );
}

/** One stored entry, or `null` when it cannot be read. */
function readEntry(v: unknown): PaletteEntry | null {
  if (typeof v !== 'object' || v === null) return null;
  const { rgba, name } = v as { rgba?: unknown; name?: unknown };
  if (typeof rgba !== 'number' || !Number.isInteger(rgba) || rgba < 0 || rgba > 0xffffffff) {
    return null;
  }
  if (name !== undefined && typeof name !== 'string') return null;
  const color = unpackRgba(rgba);
  return name ? { color, name } : { color };
}

/** A stored palette: its readable entries, at most the first
 *  `PALETTE_MAX`; `null` (the default palette) when the value is not a
 *  list or holds no readable entry (AC-14). */
export function readStoredPalette(v: unknown): PaletteEntry[] | null {
  if (!Array.isArray(v)) return null;
  const entries = v
    .map(readEntry)
    .filter((e): e is PaletteEntry => e !== null)
    .slice(0, PALETTE_MAX);
  return entries.length > 0 ? entries : null;
}

/** What Add color does with `color`: the extended palette, or why it
 *  refuses — a duplicate wins over a full palette (AC-10, AC-11). */
export function addColor(
  entries: readonly PaletteEntry[],
  color: Rgba8,
): PaletteEntry[] | 'duplicate' | 'full' {
  if (entries.some((e) => sameColor(e.color, color))) return 'duplicate';
  if (entries.length >= PALETTE_MAX) return 'full';
  return [...entries, { color: { r: color.r, g: color.g, b: color.b, a: color.a } }];
}

/** The name a swatch shows next to its hex value, or `undefined` when it
 *  has none or the name is that hex value (`.gpl` labels unnamed entries
 *  with their hex code). */
export function shownName(e: PaletteEntry): string | undefined {
  if (!e.name) return undefined;
  return e.name.toLowerCase() === toHex(e.color).toLowerCase() ? undefined : e.name;
}
