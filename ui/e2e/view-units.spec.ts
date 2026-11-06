import { expect, test } from '@playwright/test';
import {
  DEFAULT_IMAGE_VIEW,
  formatPixels,
  parseImageViewPref,
  RULER_MIN_SPACING,
  rulerStep,
  rulerTicks,
  type ViewUnit,
} from '../src/modes/image/view-units';
import { de } from '../src/lib/i18n/de';
import { en } from '../src/lib/i18n/en';

// image-view-aids: units, ruler steps and the stored pref, tested without a
// page (the module is rune-free). The editor wiring is in
// image-view-aids.spec.ts.

const UNITS: readonly ViewUnit[] = ['px', 'in', 'cm'];
// The editor's zoom limits (MIN_ZOOM / MAX_ZOOM in the rune store).
const MIN_ZOOM = 0.02;
const MAX_ZOOM = 64;

test('AC-03: labels step by 1, 2 or 5 × 10ⁿ, at least 40 px apart, at every zoom', () => {
  const zooms: number[] = [];
  for (let z = MIN_ZOOM; z <= MAX_ZOOM; z *= 1.07) zooms.push(z);
  zooms.push(MAX_ZOOM);
  for (const unit of UNITS) {
    for (const zoom of zooms) {
      const ticks = rulerTicks(2000, 37.5, zoom, unit, 96);
      expect(ticks.length).toBeGreaterThan(0);
      const step = rulerStep(zoom, unit, 96);
      const mantissa = step / 10 ** Math.floor(Math.log10(step) + 1e-9);
      expect([1, 2, 5]).toContain(Number(mantissa.toFixed(6)));
      for (let i = 1; i < ticks.length; i++) {
        const a = ticks[i - 1];
        const b = ticks[i];
        if (!a || !b) continue;
        expect(b.value - a.value).toBeCloseTo(step, 9);
        expect(b.pos - a.pos).toBeGreaterThanOrEqual(Math.max(40, RULER_MIN_SPACING) - 1e-6);
        // Neighbouring labels stay distinct after rounding to two decimals.
        if (unit !== 'px') expect(a.value.toFixed(2)).not.toBe(b.value.toFixed(2));
      }
    }
  }
  // Minimum zoom on a 20000 px image thins out to a large step.
  expect(rulerStep(MIN_ZOOM, 'px', 96)).toBe(5000);
  // Maximum zoom in inches at 96 DPI keeps a step of at least 0.01 in.
  expect(rulerStep(MAX_ZOOM, 'in', 96)).toBeGreaterThanOrEqual(0.01);
});

test('AC-02: ticks sit where their image coordinate lies, 0 at the image origin', () => {
  const ticks = rulerTicks(800, 123, 2, 'px', 96);
  for (const t of ticks) expect(t.pos).toBeCloseTo(123 + t.value * 2, 9);
  expect(ticks.some((t) => t.value === 0)).toBe(true);
  expect(ticks.some((t) => t.value < 0)).toBe(true);
});

test('AC-12: Pixels are whole, inches and centimeters two decimals, per language', () => {
  expect(formatPixels(12, 'px', 96, 'en')).toBe('12');
  expect(formatPixels(-3, 'px', 96, 'en')).toBe('-3');
  expect(formatPixels(1, 'in', 96, 'en')).toBe('0.01');
  expect(formatPixels(1, 'in', 96, 'de')).toBe('0,01');
  expect(formatPixels(96, 'cm', 96, 'en')).toBe('2.54');
  expect(formatPixels(300, 'in', 300, 'de')).toBe('1,00');
  expect([en['units.px'], en['units.in'], en['units.cm']]).toEqual(['px', 'in', 'cm']);
  expect([de['units.px'], de['units.in'], de['units.cm']]).toEqual(['px', 'Zoll', 'cm']);
});

test('AC-21: a broken resolution counts as 96 DPI', () => {
  for (const dpi of [0, -72, Number.NaN, Number.POSITIVE_INFINITY]) {
    expect(formatPixels(96, 'in', dpi, 'en')).toBe('1.00');
    expect(formatPixels(96, 'cm', dpi, 'en')).toBe('2.54');
  }
});

test('AC-20: a missing, malformed or unknown pref falls back to the defaults', () => {
  expect(DEFAULT_IMAGE_VIEW).toEqual({ showRulers: false, showPixelGrid: false, units: 'px' });
  for (const raw of [
    undefined,
    null,
    'rulers',
    42,
    {},
    { showRulers: true, showPixelGrid: true, units: 'mm' },
    { showRulers: 'yes', showPixelGrid: true, units: 'px' },
  ]) {
    expect(parseImageViewPref(raw)).toEqual(DEFAULT_IMAGE_VIEW);
  }
  expect(parseImageViewPref({ showRulers: true, showPixelGrid: false, units: 'cm' })).toEqual({
    showRulers: true,
    showPixelGrid: false,
    units: 'cm',
  });
});
