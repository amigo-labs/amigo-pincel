import { expect, test } from '@playwright/test';
import {
  DEFAULT_PALETTE,
  clampByte,
  hsvToRgb,
  parseHex,
  pushRecent,
  readColor,
  readRecent,
  rgbToHsv,
  toHex,
} from '../src/lib/color';

// Color helpers behind the picker and the Image-mode Colors panel
// (color-picker AC-05 – AC-07, AC-11, AC-12, AC-23, AC-27, AC-30). Plain
// TypeScript, so this spec runs in Node without a page.

test('the hex field accepts RRGGBB, #RRGGBB, #RRGGBBAA and #RGB (AC-05, AC-23)', () => {
  expect(parseHex('11aa33')).toEqual({ r: 0x11, g: 0xaa, b: 0x33, a: null });
  expect(parseHex('#11AA33')).toEqual({ r: 0x11, g: 0xaa, b: 0x33, a: null });
  expect(parseHex('#11223380')).toEqual({ r: 17, g: 34, b: 51, a: 128 });
  expect(parseHex('#abc')).toEqual(parseHex('#aabbcc'));
  expect(parseHex(' #ABC ')).toEqual({ r: 0xaa, g: 0xbb, b: 0xcc, a: null });
});

test('other hex text is refused (AC-06)', () => {
  for (const text of ['#12345', 'zzz', '', '#', 'abc', '#1234567', '#gggggg', '11223380']) {
    expect(parseHex(text), text).toBeNull();
  }
});

test('channel values clamp to 0–255 (AC-07)', () => {
  expect(clampByte(-5)).toBe(0);
  expect(clampByte(300)).toBe(255);
  expect(clampByte(12.4)).toBe(12);
  expect(clampByte(Number.NaN)).toBe(0);
});

test('hex text round-trips with and without alpha', () => {
  expect(toHex({ r: 17, g: 34, b: 51, a: 128 })).toBe('#112233');
  expect(toHex({ r: 17, g: 34, b: 51, a: 128 }, true)).toBe('#11223380');
});

test('HSV conversion keeps hue and saturation when they are undefined (AC-11)', () => {
  expect(hsvToRgb(0, 1, 1)).toEqual({ r: 255, g: 0, b: 0 });
  expect(hsvToRgb(120, 1, 1)).toEqual({ r: 0, g: 255, b: 0 });
  expect(rgbToHsv({ r: 0, g: 0, b: 255 })).toEqual({ h: 240, s: 1, v: 1 });
  // Black and grey have no hue of their own: the previous one is kept.
  expect(rgbToHsv({ r: 0, g: 0, b: 0 }, { h: 200, s: 0.5 })).toEqual({ h: 200, s: 0.5, v: 0 });
  expect(rgbToHsv({ r: 128, g: 128, b: 128 }, { h: 200, s: 0.5 }).h).toBe(200);
});

test('the default palette is 12 greys and 12 hues at 100, 70 and 40 % value (AC-27)', () => {
  expect(DEFAULT_PALETTE).toHaveLength(48);
  expect(DEFAULT_PALETTE.every((c) => c.a === 255)).toBe(true);
  expect(DEFAULT_PALETTE.slice(0, 12).map((c) => c.r)).toEqual([
    0, 23, 46, 70, 93, 116, 139, 162, 185, 209, 232, 255,
  ]);
  expect(DEFAULT_PALETTE.slice(0, 12).every((c) => c.r === c.g && c.g === c.b)).toBe(true);
  const hex = DEFAULT_PALETTE.map((c) => toHex(c));
  expect(hex.slice(12, 24)).toEqual([
    '#ff0000',
    '#ff8000',
    '#ffff00',
    '#80ff00',
    '#00ff00',
    '#00ff80',
    '#00ffff',
    '#0080ff',
    '#0000ff',
    '#8000ff',
    '#ff00ff',
    '#ff0080',
  ]);
  // 70 %: 178.5 rounds up to 179, 89.25 down to 89.
  expect(hex[24]).toBe('#b30000');
  expect(hex[25]).toBe('#b35900');
  // 40 %: 102 and 51.
  expect(hex[36]).toBe('#660000');
  expect(hex[37]).toBe('#663300');
});

test('recent colors: most recent first, no duplicates, at most 12 (AC-12)', () => {
  const c = (n: number, a = 255) => ({ r: n, g: n, b: n, a });
  let list = pushRecent([], [c(1)]);
  list = pushRecent(list, [c(2)]);
  list = pushRecent(list, [c(1)]);
  expect(list).toEqual([c(1), c(2)]);
  // Same RGB, other alpha: a different color.
  list = pushRecent(list, [c(2, 128)]);
  expect(list).toEqual([c(2, 128), c(1), c(2)]);
  // Foreground first when both were used.
  list = pushRecent(list, [c(9), c(8)]);
  expect(list.slice(0, 2)).toEqual([c(9), c(8)]);
  for (let i = 20; i < 40; i += 1) list = pushRecent(list, [c(i)]);
  expect(list).toHaveLength(12);
  expect(list[0]).toEqual(c(39));
});

test('stored colors that cannot be read fall back, keeping readable entries (AC-30)', () => {
  expect(readRecent(undefined)).toEqual([]);
  expect(readRecent('nope')).toEqual([]);
  expect(readRecent(['#11223380', 42, '#zzz', '#445566'])).toEqual([
    { r: 17, g: 34, b: 51, a: 128 },
    { r: 0x44, g: 0x55, b: 0x66, a: 255 },
  ]);
  expect(readColor('#11223380')).toEqual({ r: 17, g: 34, b: 51, a: 128 });
  // A pref from before alpha: opaque.
  expect(readColor('#445566')).toEqual({ r: 0x44, g: 0x55, b: 0x66, a: 255 });
  expect(readColor(null)).toBeNull();
  expect(readColor('#12')).toBeNull();
});
