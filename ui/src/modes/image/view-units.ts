// Image-mode view aids: units, ruler steps and the stored view preference
// (docs/specs/image-view-aids.md). Rune-free, so node-side Playwright specs
// can test the arithmetic (AC-03, AC-12, AC-20, AC-21).

import { usesDecimalComma, type Locale } from '../../lib/i18n/locales';

/** The unit the rulers and the status bar show. */
export type ViewUnit = 'px' | 'in' | 'cm';

/** The UI language, for the decimal separator and the unit suffix. */
export type ViewLocale = Locale;

/** The persisted Image view state (AC-13, AC-14). */
export interface ImageViewPref {
  readonly showRulers: boolean;
  readonly showPixelGrid: boolean;
  readonly units: ViewUnit;
}

/** The IndexedDB pref key. */
export const IMAGE_VIEW_PREF = 'imageView';

/** A fresh profile's view: no rulers, no pixel grid, Pixels (AC-13). */
export const DEFAULT_IMAGE_VIEW: ImageViewPref = {
  showRulers: false,
  showPixelGrid: false,
  units: 'px',
};

/** Zoom from which the pixel grid is drawn: 400 % (AC-07, AC-08). */
export const PIXEL_GRID_MIN_ZOOM = 4;

/** Minimum distance between two ruler labels in CSS px: wider than the
 *  longest label, so neighbours never overlap (AC-03). */
export const RULER_MIN_SPACING = 60;

const UNITS: readonly ViewUnit[] = ['px', 'in', 'cm'];

/** The stored pref, or the defaults when it is missing, malformed or names
 *  an unknown unit (AC-20). */
export function parseImageViewPref(raw: unknown): ImageViewPref {
  if (typeof raw !== 'object' || raw === null) return DEFAULT_IMAGE_VIEW;
  const r = raw as Record<string, unknown>;
  if (
    typeof r.showRulers !== 'boolean' ||
    typeof r.showPixelGrid !== 'boolean' ||
    !UNITS.includes(r.units as ViewUnit)
  ) {
    return DEFAULT_IMAGE_VIEW;
  }
  return { showRulers: r.showRulers, showPixelGrid: r.showPixelGrid, units: r.units as ViewUnit };
}

/** `dpi` when it is a positive finite number, otherwise 96 (AC-21). */
export function viewDpi(dpi: number): number {
  return Number.isFinite(dpi) && dpi > 0 ? dpi : 96;
}

/** Pixels in one `unit` at `dpi`. */
export function pixelsPerUnit(unit: ViewUnit, dpi: number): number {
  const d = viewDpi(dpi);
  return unit === 'px' ? 1 : unit === 'in' ? d : d / 2.54;
}

/** A pixel value `p` in `unit` (AC-12). */
export function toUnit(p: number, unit: ViewUnit, dpi: number): number {
  return p / pixelsPerUnit(unit, dpi);
}

/** A value in `unit` as text: whole numbers for Pixels, two decimals
 *  otherwise, with the language's decimal separator (AC-12). */
export function formatUnitValue(v: number, unit: ViewUnit, locale: ViewLocale): string {
  const s = unit === 'px' ? String(Math.round(v)) : v.toFixed(2);
  // `-0.00` reads as a sign error.
  const clean = /^-0(\.0+)?$/.test(s) ? s.slice(1) : s;
  return usesDecimalComma(locale) ? clean.replace('.', ',') : clean;
}

/** A pixel value `p` shown in `unit`. */
export function formatPixels(p: number, unit: ViewUnit, dpi: number, locale: ViewLocale): string {
  return formatUnitValue(toUnit(p, unit, dpi), unit, locale);
}

/**
 * The ruler step in `unit`: the smallest 1, 2 or 5 × 10ⁿ whose labels lie at
 * least {@link RULER_MIN_SPACING} CSS px apart at `zoom` (CSS px per image
 * px). Pixels step by at least 1, inches and centimeters by at least 0.01,
 * so neighbouring labels stay distinct after rounding (AC-03).
 */
export function rulerStep(zoom: number, unit: ViewUnit, dpi: number): number {
  const screenPerUnit = zoom * pixelsPerUnit(unit, dpi);
  const min = unit === 'px' ? 0 : -2;
  for (let n = min; ; n++) {
    for (const m of [1, 2, 5]) {
      const step = m * 10 ** n;
      if (step * screenPerUnit >= RULER_MIN_SPACING) return Number(step.toPrecision(12));
    }
  }
}

/** One labelled ruler tick. */
export interface RulerTick {
  /** The value in the ruler's unit. */
  readonly value: number;
  /** Screen offset from the ruler's start, CSS px. */
  readonly pos: number;
}

/**
 * The labelled ticks of a ruler `length` CSS px long whose image coordinate
 * 0 lies at screen offset `pan` (AC-02): every multiple of the step whose
 * screen position falls inside the ruler.
 */
export function rulerTicks(
  length: number,
  pan: number,
  zoom: number,
  unit: ViewUnit,
  dpi: number,
): RulerTick[] {
  const step = rulerStep(zoom, unit, dpi);
  const screenPerUnit = zoom * pixelsPerUnit(unit, dpi);
  const first = Math.ceil(-pan / screenPerUnit / step);
  const last = Math.floor((length - pan) / screenPerUnit / step);
  const ticks: RulerTick[] = [];
  for (let k = first; k <= last; k++) {
    const value = Number((k * step).toPrecision(12));
    ticks.push({ value, pos: pan + value * screenPerUnit });
  }
  return ticks;
}
