import { expect, test } from '@playwright/test';
import { de } from '../src/lib/i18n/de';
import { en, type MessageKey } from '../src/lib/i18n/en';
import {
  formatDateTime,
  formatMessage,
  formatNumber,
  formatPercent,
  placeholders,
} from '../src/lib/i18n/format';

// Message formatting and the dictionaries' placeholders
// (docs/specs/more-languages.md AC-06 – AC-09). Node only: the
// dictionaries and `format.ts` are rune-free.

const DICTS: Readonly<Record<string, Readonly<Record<MessageKey, string>>>> = { de };

test('count messages follow the locale plural rules (AC-07, AC-08)', () => {
  const snapshots = '{count, plural, one {# snapshot} other {# snapshots}}';
  expect(formatMessage(snapshots, { count: 1 }, 'en')).toBe('1 snapshot');
  expect(formatMessage(snapshots, { count: 2 }, 'en')).toBe('2 snapshots');
  expect(formatMessage(snapshots, { count: 0 }, 'en')).toBe('0 snapshots');
  // French uses "one" for 0 and 1.
  const fr = '{count, plural, one {# instantané} other {# instantanés}}';
  expect(formatMessage(fr, { count: 0 }, 'fr')).toBe('0 instantané');
  expect(formatMessage(fr, { count: 1 }, 'fr')).toBe('1 instantané');
  expect(formatMessage(fr, { count: 2 }, 'fr')).toBe('2 instantanés');
  // Japanese has one form for every count.
  const ja = '{count, plural, other {#件のスナップショット}}';
  for (const n of [0, 1, 2, 10])
    expect(formatMessage(ja, { count: n }, 'ja')).toBe(`${n}件のスナップショット`);
  // `#` uses the locale's number format; placeholders inside a form fill too.
  expect(
    formatMessage(
      '{n, plural, one {# file in {dir}} other {# files in {dir}}}',
      { n: 1200, dir: 'x' },
      'de',
    ),
  ).toBe('1.200 files in x');
  // Unknown placeholders stay as written.
  expect(formatMessage('a {b} c', { x: 1 }, 'en')).toBe('a {b} c');
});

test('no shown message uses a "(s)"-style fallback (AC-07)', () => {
  for (const [name, dict] of Object.entries({ en, ...DICTS })) {
    for (const [key, text] of Object.entries(dict)) {
      expect(/\((s|e?n|es)\)/.test(text), `${name} ${key}: ${text}`).toBe(false);
    }
  }
});

test('every translation has exactly the placeholders of its English source (AC-06)', () => {
  for (const [name, dict] of Object.entries(DICTS)) {
    for (const key of Object.keys(en) as MessageKey[]) {
      expect(placeholders(dict[key]), `${name} ${key}`).toEqual(placeholders(en[key]));
    }
  }
});

test('numbers, percentages and dates follow the locale (AC-09)', () => {
  // A no-break space before %: narrow (U+202F) or plain (U+00A0), as the
  // runtime's CLDR version writes it.
  expect(formatPercent(100, 'fr')).toMatch(/^100[\u00a0\u202f]%$/);
  expect(formatPercent(50, 'de')).toMatch(/^50[\u00a0\u202f]%$/);
  expect(formatPercent(50, 'en')).toBe('50%');
  expect(formatNumber(1.5, 'de')).toBe('1,5');
  expect(formatNumber(1.5, 'pt-BR')).toBe('1,5');
  expect(formatNumber(1.5, 'en')).toBe('1.5');
  const when = Date.UTC(2026, 10, 12, 9, 30);
  expect(formatDateTime(when, 'de')).toContain('2026');
  expect(formatDateTime(when, 'fr')).not.toBe(formatDateTime(when, 'en'));
});
