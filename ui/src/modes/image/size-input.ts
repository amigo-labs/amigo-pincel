// The size input both Image-mode size dialogs share (Scale Image, Resize
// Canvas): percentage or absolute size in pixels / percent / inches /
// centimeters at a resolution, with an aspect-ratio lock. Rune-free, so
// node-side Playwright specs can test the arithmetic
// (docs/specs/image-resize-autocrop.md AC-01 – AC-08, AC-22).

import { clampCanvasSize } from './canvas-limits';

/** Units of the absolute size. */
export type SizeUnit = 'px' | 'percent' | 'in' | 'cm';

/** Which half of the input applies. */
export type SizeMode = 'percent' | 'absolute';

/** A field the user can type into. */
export type SizeField = 'percent' | 'width' | 'height' | 'resolution';

/** The UI language, for the decimal separator. */
export type SizeLocale = 'en' | 'de';

/** A text field; `exact` holds the unrounded value a unit change set while
 *  the text still shows it, so switching units does not accumulate
 *  rounding (AC-04). */
export interface FieldValue {
  readonly text: string;
  readonly exact?: number;
}

/** Everything the size input holds. */
export interface SizeState {
  readonly mode: SizeMode;
  readonly percent: FieldValue;
  readonly width: FieldValue;
  readonly height: FieldValue;
  readonly unit: SizeUnit;
  readonly resolution: FieldValue;
  readonly keepAspect: boolean;
}

/** The document the dialog resizes. */
export interface SizeDoc {
  readonly width: number;
  readonly height: number;
  readonly dpi: number;
}

/** The fallback resolution, as `image-view-aids` AC-21. */
export const DEFAULT_DPI = 96;
/** Highest resolution the field accepts (AC-07). */
export const MAX_RESOLUTION = 10000;
const CM_PER_INCH = 2.54;

/** `dpi` when it is a positive finite number, otherwise 96. */
export function effectiveDpi(dpi: number): number {
  return Number.isFinite(dpi) && dpi > 0 ? dpi : DEFAULT_DPI;
}

/** A non-negative decimal typed with a point or a comma; `null` when the
 *  text is not one (AC-22). */
export function parseDecimal(text: string): number | null {
  const s = text.trim().replace(',', '.');
  if (!/^(\d+\.?\d*|\.\d+)$/.test(s)) return null;
  const v = Number(s);
  return Number.isFinite(v) ? v : null;
}

/** `v` with `decimals` places and the language's decimal separator. */
export function formatDecimal(v: number, decimals: number, locale: SizeLocale): string {
  const s = v.toFixed(decimals);
  return locale === 'de' ? s.replace('.', ',') : s;
}

/** `v` with at most two decimals, trailing zeros dropped. */
function formatTrimmed(v: number, locale: SizeLocale): string {
  const s = String(Math.round(v * 100) / 100);
  return locale === 'de' ? s.replace('.', ',') : s;
}

/** An edge length in `unit` as shown: whole pixels, otherwise two decimals. */
export function formatEdge(v: number, unit: SizeUnit, locale: SizeLocale): string {
  return unit === 'px' ? String(Math.round(v)) : formatDecimal(v, 2, locale);
}

function value(f: FieldValue): number | null {
  return f.exact ?? parseDecimal(f.text);
}

/** `v` in `unit` as unrounded pixels of an edge `edge` px long at `dpi`. */
export function toPixels(v: number, unit: SizeUnit, edge: number, dpi: number): number {
  switch (unit) {
    case 'px':
      return v;
    case 'percent':
      return (edge * v) / 100;
    case 'in':
      return v * dpi;
    case 'cm':
      return (v * dpi) / CM_PER_INCH;
  }
}

/** The inverse of {@link toPixels}. */
export function fromPixels(px: number, unit: SizeUnit, edge: number, dpi: number): number {
  switch (unit) {
    case 'px':
      return px;
    case 'percent':
      return (px * 100) / edge;
    case 'in':
      return px / dpi;
    case 'cm':
      return (px * CM_PER_INCH) / dpi;
  }
}

/** Whether the unit converts through the resolution. */
export function usesResolution(unit: SizeUnit): boolean {
  return unit === 'in' || unit === 'cm';
}

/** The fields the selected mode reads (AC-07). */
export function usedFields(s: SizeState): readonly SizeField[] {
  if (s.mode === 'percent') return ['percent'];
  return usesResolution(s.unit) ? ['width', 'height', 'resolution'] : ['width', 'height'];
}

/** Whether `field` holds a value it can be applied with (AC-07). */
export function fieldValid(s: SizeState, field: SizeField): boolean {
  const v = value(s[field]);
  if (v === null || v <= 0) return false;
  if (field === 'resolution') return v <= MAX_RESOLUTION;
  if ((field === 'width' || field === 'height') && s.unit === 'px') return Number.isInteger(v);
  return true;
}

/** The fields the selected mode reads that are invalid. */
export function invalidFields(s: SizeState): readonly SizeField[] {
  return usedFields(s).filter((f) => !fieldValid(s, f));
}

/** The resulting size in whole pixels before the canvas limits, or `null`
 *  while a used field is invalid (AC-02, AC-03). */
export function rawSize(s: SizeState, doc: SizeDoc): [number, number] | null {
  if (invalidFields(s).length > 0) return null;
  const px = (v: number): number => Math.max(1, Math.round(v));
  if (s.mode === 'percent') {
    const p = value(s.percent) ?? 0;
    return [px((doc.width * p) / 100), px((doc.height * p) / 100)];
  }
  const dpi = value(s.resolution) ?? DEFAULT_DPI;
  return [
    px(toPixels(value(s.width) ?? 0, s.unit, doc.width, dpi)),
    px(toPixels(value(s.height) ?? 0, s.unit, doc.height, dpi)),
  ];
}

/** The size Apply produces: {@link rawSize} reduced to the canvas limits
 *  (AC-08), or `null` while a used field is invalid. */
export function resultSize(s: SizeState, doc: SizeDoc): [number, number] | null {
  const raw = rawSize(s, doc);
  return raw ? clampCanvasSize(raw[0], raw[1]) : null;
}

/** The state every opening starts from (AC-01). */
export function initialSize(doc: SizeDoc, locale: SizeLocale): SizeState {
  return {
    mode: 'percent',
    percent: { text: '100' },
    width: { text: String(doc.width) },
    height: { text: String(doc.height) },
    unit: 'px',
    resolution: { text: formatTrimmed(effectiveDpi(doc.dpi), locale) },
    keepAspect: true,
  };
}

/** The text of `field` changed; with the aspect lock on, an edge edit sets
 *  the other edge to keep the document's ratio (AC-06). */
export function editField(
  s: SizeState,
  field: SizeField,
  text: string,
  doc: SizeDoc,
  locale: SizeLocale,
): SizeState {
  const next: SizeState = { ...s, [field]: { text } };
  if (!s.keepAspect || (field !== 'width' && field !== 'height')) return next;
  const v = parseDecimal(text);
  if (v === null) return next;
  const other = field === 'width' ? 'height' : 'width';
  const [from, to] = field === 'width' ? [doc.width, doc.height] : [doc.height, doc.width];
  const dpi = value(s.resolution) ?? DEFAULT_DPI;
  const otherPx = (toPixels(v, s.unit, from, dpi) * to) / from;
  const otherV = fromPixels(otherPx, s.unit, to, dpi);
  const shown = s.unit === 'px' ? Math.max(1, Math.round(otherV)) : otherV;
  return { ...next, [other]: { text: formatEdge(shown, s.unit, locale) } };
}

/** Switch the absolute size to `unit`, showing the current resulting
 *  pixel size in it (AC-04). Invalid fields switch without conversion. */
export function changeUnit(
  s: SizeState,
  unit: SizeUnit,
  doc: SizeDoc,
  locale: SizeLocale,
): SizeState {
  const current = rawSize({ ...s, mode: 'absolute' }, doc);
  if (current === null) return { ...s, unit };
  const dpi = fieldValid(s, 'resolution') ? (value(s.resolution) ?? DEFAULT_DPI) : DEFAULT_DPI;
  const field = (px: number, edge: number): FieldValue => {
    const exact = fromPixels(px, unit, edge, dpi);
    return unit === 'px' ? { text: String(px) } : { text: formatEdge(exact, unit, locale), exact };
  };
  return {
    ...s,
    unit,
    width: field(current[0], doc.width),
    height: field(current[1], doc.height),
  };
}
