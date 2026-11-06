// Color helpers shared by both modes. The wasm surface speaks packed
// `0xRRGGBBAA` (red in the high byte, alpha in the low byte); the UI keeps
// `#RRGGBB` plus a separate alpha. The picker (color-picker spec) adds hex
// parsing, HSV conversion, the recent-colors list and the Image-mode
// default palette. Rune-free, so Node-side specs can import it.

/** `#RRGGBB` → packed `0xRRGGBBAA`. Alpha defaults to fully opaque. */
export function packColor(hex: string, alpha = 0xff): number {
  const rgb = Number.parseInt(hex.slice(1), 16);
  return ((rgb << 8) | (alpha & 0xff)) >>> 0;
}

/**
 * Packed `0xRRGGBBAA` → `#RRGGBB`. Alpha is intentionally dropped; callers
 * that need it read the low byte themselves.
 */
export function unpackColor(rgba: number): string {
  const rgb = (rgba >>> 8) & 0xffffff;
  return '#' + rgb.toString(16).padStart(6, '0');
}

/** An 8-bit RGB color. */
export interface Rgb8 {
  readonly r: number;
  readonly g: number;
  readonly b: number;
}

/** An 8-bit RGBA color (straight alpha). */
export interface Rgba8 extends Rgb8 {
  readonly a: number;
}

/** Hue in degrees 0–360, saturation and value 0–1. */
export interface Hsv {
  readonly h: number;
  readonly s: number;
  readonly v: number;
}

/** A parsed hex color; `a` is `null` when the text carried no alpha. */
export interface ParsedHex extends Rgb8 {
  readonly a: number | null;
}

/** `v` rounded and clamped to 0–255; `NaN` gives 0 (color-picker AC-07). */
export function clampByte(v: number): number {
  if (Number.isNaN(v)) return 0;
  return Math.min(255, Math.max(0, Math.round(v)));
}

const byte = (s: string | undefined): number => Number.parseInt(s ?? '0', 16);

/** Parses `RRGGBB`, `#RRGGBB`, `#RRGGBBAA` or `#RGB` (any case, outer
 *  spaces ignored); `null` for anything else (color-picker AC-05, AC-06). */
export function parseHex(text: string): ParsedHex | null {
  const t = text.trim().toLowerCase();
  let m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/.exec(t);
  if (m) return { r: byte(m[1]), g: byte(m[2]), b: byte(m[3]), a: null };
  m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/.exec(t);
  if (m) return { r: byte(m[1]), g: byte(m[2]), b: byte(m[3]), a: byte(m[4]) };
  m = /^#([0-9a-f])([0-9a-f])([0-9a-f])$/.exec(t);
  if (m) {
    const d = (s: string | undefined) => byte(`${s ?? '0'}${s ?? '0'}`);
    return { r: d(m[1]), g: d(m[2]), b: d(m[3]), a: null };
  }
  return null;
}

const hex2 = (v: number): string => clampByte(v).toString(16).padStart(2, '0');

/** `#rrggbb`, or `#rrggbbaa` with `withAlpha`. */
export function toHex(c: Rgb8 & { readonly a?: number }, withAlpha = false): string {
  const rgb = `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}`;
  return withAlpha ? `${rgb}${hex2(c.a ?? 255)}` : rgb;
}

/** `#rrggbb` (as the stores keep colors) → RGB bytes; black on bad input. */
export function hexToRgb(hex: string): Rgb8 {
  const p = parseHex(hex);
  return p ? { r: p.r, g: p.g, b: p.b } : { r: 0, g: 0, b: 0 };
}

/** HSV → RGB bytes, each channel rounded to the nearest integer. */
export function hsvToRgb(h: number, s: number, v: number): Rgb8 {
  const hh = (((h % 360) + 360) % 360) / 60;
  const i = Math.floor(hh);
  const f = hh - i;
  const p = v * (1 - s);
  const q = v * (1 - s * f);
  const t = v * (1 - s * (1 - f));
  const sectors: readonly (readonly [number, number, number])[] = [
    [v, t, p],
    [q, v, p],
    [p, v, t],
    [p, q, v],
    [t, p, v],
    [v, p, q],
  ];
  const [r, g, b] = sectors[i] ?? [v, t, p];
  return { r: clampByte(r * 255), g: clampByte(g * 255), b: clampByte(b * 255) };
}

/** RGB → HSV. Black has no saturation and grey no hue of their own:
 *  `keep` supplies them, so moving through black or grey does not reset
 *  the hue slider (color-picker AC-11). */
export function rgbToHsv(c: Rgb8, keep: { h: number; s: number } = { h: 0, s: 0 }): Hsv {
  const r = c.r / 255;
  const g = c.g / 255;
  const b = c.b / 255;
  const max = Math.max(r, g, b);
  const d = max - Math.min(r, g, b);
  if (max === 0) return { h: keep.h, s: keep.s, v: 0 };
  const s = d / max;
  if (d === 0) return { h: keep.h, s, v: max };
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  h *= 60;
  if (h < 0) h += 360;
  return { h, s, v: max };
}

/** How many colors the recent row holds (color-picker AC-12). */
export const RECENT_MAX = 12;

/** Two colors are the same entry when R, G, B and A are equal. */
export function sameColor(a: Rgba8, b: Rgba8): boolean {
  return a.r === b.r && a.g === b.g && a.b === b.b && a.a === b.a;
}

/** `list` with `used` (first = most recent) moved to the front, without
 *  duplicates, cut to `RECENT_MAX` (color-picker AC-12, AC-24). */
export function pushRecent(list: readonly Rgba8[], used: readonly Rgba8[]): Rgba8[] {
  const front: Rgba8[] = [];
  for (const c of used) if (!front.some((f) => sameColor(f, c))) front.push(c);
  const rest = list.filter((c) => !front.some((f) => sameColor(f, c)));
  return [...front, ...rest].slice(0, RECENT_MAX);
}

/** A stored color (`#rrggbb[aa]`), or `null` when it cannot be read; a
 *  value from before alpha loads opaque (color-picker AC-30). */
export function readColor(v: unknown): Rgba8 | null {
  if (typeof v !== 'string') return null;
  const p = parseHex(v);
  return p ? { r: p.r, g: p.g, b: p.b, a: p.a ?? 255 } : null;
}

/** A stored recent-colors list, keeping every readable entry (AC-30). */
export function readRecent(v: unknown): Rgba8[] {
  if (!Array.isArray(v)) return [];
  return v
    .map(readColor)
    .filter((c): c is Rgba8 => c !== null)
    .slice(0, RECENT_MAX);
}

/** The Image-mode default palette (color-picker AC-27): 12 greys, then 12
 *  fully saturated hues (0°, 30°, …) at value 100 %, 70 % and 40 %. Hues
 *  at multiples of 30° only need the factors 1, ½ and 0 of the value, so
 *  each channel is computed exactly and rounds halves up. */
export const DEFAULT_PALETTE: readonly Rgba8[] = (() => {
  const out: Rgba8[] = [];
  for (let i = 0; i < 12; i += 1) {
    const g = Math.round((i * 255) / 11);
    out.push({ r: g, g, b: g, a: 255 });
  }
  // Per hue, the share of the value in R, G and B, in halves.
  const halves: readonly (readonly [number, number, number])[] = [
    [2, 0, 0],
    [2, 1, 0],
    [2, 2, 0],
    [1, 2, 0],
    [0, 2, 0],
    [0, 2, 1],
    [0, 2, 2],
    [0, 1, 2],
    [0, 0, 2],
    [1, 0, 2],
    [2, 0, 2],
    [2, 0, 1],
  ];
  for (const percent of [100, 70, 40]) {
    for (const [r, g, b] of halves) {
      const ch = (k: number) => Math.round((percent * 255 * k) / 200);
      out.push({ r: ch(r), g: ch(g), b: ch(b), a: 255 });
    }
  }
  return out;
})();
