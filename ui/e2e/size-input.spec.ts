import { expect, test } from '@playwright/test';
import {
  changeUnit,
  editField,
  effectiveDpi,
  initialSize,
  invalidFields,
  parseDecimal,
  rawSize,
  resultSize,
  type SizeDoc,
  type SizeState,
  type SizeUnit,
} from '../src/modes/image/size-input';

// image-resize-autocrop: the size arithmetic both size dialogs share, tested
// without a page (the module is rune-free). The dialogs' wiring is in
// image-resize-autocrop.spec.ts.

const DOC: SizeDoc = { width: 400, height: 300, dpi: 96 };

function absolute(doc: SizeDoc, unit: SizeUnit, w: string, h: string, res?: string): SizeState {
  const s = initialSize(doc, 'en');
  return {
    ...s,
    mode: 'absolute',
    unit,
    width: { text: w },
    height: { text: h },
    resolution: res === undefined ? s.resolution : { text: res },
  };
}

test('AC-01: every opening starts at 100 %, Pixels, document size, aspect on', () => {
  const s = initialSize(DOC, 'en');
  expect(s.mode).toBe('percent');
  expect(s.percent.text).toBe('100');
  expect(s.unit).toBe('px');
  expect([s.width.text, s.height.text]).toEqual(['400', '300']);
  expect(s.keepAspect).toBe(true);
  expect(s.resolution.text).toBe('96');
  expect(resultSize(s, DOC)).toEqual([400, 300]);
  expect(initialSize({ ...DOC, dpi: 300 }, 'en').resolution.text).toBe('300');
  for (const bad of [0, -5, Number.NaN, Number.POSITIVE_INFINITY]) {
    expect(effectiveDpi(bad)).toBe(96);
    expect(initialSize({ ...DOC, dpi: bad }, 'en').resolution.text).toBe('96');
  }
});

test('AC-02: a percentage scales both edges, at least 1 px each', () => {
  const s = initialSize(DOC, 'en');
  expect(rawSize({ ...s, percent: { text: '50' } }, DOC)).toEqual([200, 150]);
  expect(rawSize({ ...s, percent: { text: '33.3' } }, DOC)).toEqual([133, 100]);
  const small: SizeDoc = { width: 50, height: 50, dpi: 96 };
  expect(rawSize({ ...initialSize(small, 'en'), percent: { text: '1' } }, small)).toEqual([1, 1]);
});

test('AC-03: absolute sizes in each unit', () => {
  expect(rawSize(absolute(DOC, 'in', '2', '1', '300'), DOC)).toEqual([600, 300]);
  expect(rawSize(absolute(DOC, 'cm', '2.54', '1', '300'), DOC)).toEqual([300, 118]);
  expect(rawSize(absolute(DOC, 'percent', '50', '200'), DOC)).toEqual([200, 600]);
  expect(rawSize(absolute(DOC, 'px', '37', '12'), DOC)).toEqual([37, 12]);
  expect(rawSize(absolute(DOC, 'in', '0.001', '1'), DOC)).toEqual([1, 96]);
});

test('AC-04: a unit change shows the size in the new unit and keeps it', () => {
  const doc: SizeDoc = { width: 333, height: 217, dpi: 96 };
  let s = absolute(doc, 'px', '333', '217', '300');
  s = changeUnit(s, 'in', doc, 'en');
  expect([s.width.text, s.height.text]).toEqual(['1.11', '0.72']);
  expect(rawSize(s, doc)).toEqual([333, 217]);
  s = changeUnit(s, 'cm', doc, 'en');
  expect(rawSize(s, doc)).toEqual([333, 217]);
  s = changeUnit(s, 'percent', doc, 'en');
  expect([s.width.text, s.height.text]).toEqual(['100.00', '100.00']);
  s = changeUnit(s, 'px', doc, 'en');
  expect([s.width.text, s.height.text]).toEqual(['333', '217']);
  // Every unit round trip stays within 1 px per edge, at any resolution.
  for (const res of ['72', '96', '300', '9999']) {
    for (const unit of ['in', 'cm', 'percent'] as const) {
      const there = changeUnit(absolute(doc, 'px', '257', '31', res), unit, doc, 'en');
      const [w, h] = rawSize(there, doc) ?? [0, 0];
      expect(Math.abs(w - 257)).toBeLessThanOrEqual(1);
      expect(Math.abs(h - 31)).toBeLessThanOrEqual(1);
      const back = changeUnit(there, 'px', doc, 'en');
      expect(rawSize(back, doc)).toEqual([w, h]);
    }
  }
});

test('AC-05: the resolution moves only print sizes', () => {
  const inches = absolute(DOC, 'in', '2', '1', '96');
  expect(rawSize(inches, DOC)).toEqual([192, 96]);
  expect(rawSize({ ...inches, resolution: { text: '300' } }, DOC)).toEqual([600, 300]);
  const px = absolute(DOC, 'px', '50', '40', '96');
  expect(rawSize({ ...px, resolution: { text: '300' } }, DOC)).toEqual([50, 40]);
  const pct = absolute(DOC, 'percent', '50', '50', '96');
  expect(rawSize({ ...pct, resolution: { text: '300' } }, DOC)).toEqual([200, 150]);
  const byPct = { ...initialSize(DOC, 'en'), percent: { text: '50' } };
  expect(rawSize({ ...byPct, resolution: { text: '300' } }, DOC)).toEqual([200, 150]);
});

test('AC-06: the aspect lock follows the document ratio, off it does not', () => {
  let s = absolute(DOC, 'px', '400', '300');
  s = editField(s, 'width', '200', DOC, 'en');
  expect(s.height.text).toBe('150');
  s = editField(s, 'height', '30', DOC, 'en');
  expect(s.width.text).toBe('40');
  s = editField(absolute(DOC, 'in', '1', '1'), 'width', '2', DOC, 'en');
  expect(s.height.text).toBe('1.50');
  s = editField(absolute(DOC, 'percent', '100', '100'), 'height', '25', DOC, 'en');
  expect(s.width.text).toBe('25.00');
  const off = { ...absolute(DOC, 'px', '400', '300'), keepAspect: false };
  expect(editField(off, 'width', '10', DOC, 'en').height.text).toBe('300');
});

test('AC-07: invalid used fields block, unused ones do not', () => {
  for (const bad of ['', 'abc', '0', '-3']) {
    expect(invalidFields(absolute(DOC, 'in', bad, '1'))).toEqual(['width']);
  }
  expect(invalidFields(absolute(DOC, 'px', '10.5', '3'))).toEqual(['width']);
  expect(invalidFields(absolute(DOC, 'in', '10.5', '3'))).toEqual([]);
  expect(invalidFields(absolute(DOC, 'in', '1', '1', '10001'))).toEqual(['resolution']);
  expect(invalidFields(absolute(DOC, 'in', '1', '1', '10000'))).toEqual([]);
  expect(rawSize(absolute(DOC, 'cm', '1', '1', '0'), DOC)).toBeNull();
  // The resolution counts only for inches and centimeters.
  expect(invalidFields(absolute(DOC, 'px', '10', '10', '0'))).toEqual([]);
  // The absolute fields count only while the absolute size is selected.
  const pct = { ...absolute(DOC, 'px', '', 'x'), mode: 'percent' as const };
  expect(invalidFields(pct)).toEqual([]);
  expect(invalidFields({ ...pct, percent: { text: '0' } })).toEqual(['percent']);
});

test('AC-08: a size beyond the limits is reduced as image-files AC-16', () => {
  const big: SizeDoc = { width: 4000, height: 3000, dpi: 96 };
  const s = { ...initialSize(big, 'en'), percent: { text: '1000' } };
  expect(rawSize(s, big)).toEqual([40000, 30000]);
  expect(resultSize(s, big)).toEqual([8192, 8192]);
  expect(resultSize(absolute(DOC, 'px', '20000', '50'), DOC)).toEqual([16384, 50]);
});

test('AC-22: decimals follow the language, both separators are accepted', () => {
  expect(parseDecimal('1,5')).toBe(1.5);
  expect(parseDecimal('1.5')).toBe(1.5);
  expect(parseDecimal(',5')).toBe(0.5);
  expect(parseDecimal('1,5,0')).toBeNull();
  const de = changeUnit(absolute(DOC, 'px', '96', '48'), 'in', DOC, 'de');
  expect([de.width.text, de.height.text]).toEqual(['1,00', '0,50']);
  expect(rawSize(editField(de, 'width', '2,5', DOC, 'de'), DOC)).toEqual([240, 180]);
  expect(initialSize({ ...DOC, dpi: 72.5 }, 'de').resolution.text).toBe('72,5');
  expect(initialSize({ ...DOC, dpi: 72.5 }, 'en').resolution.text).toBe('72.5');
});
