import { expect, test } from '@playwright/test';
import { de } from '../src/lib/i18n/de';
import { en } from '../src/lib/i18n/en';
import { historyStepName, type Translate } from '../src/lib/history/names';

// docs/specs/clone-stamp.md AC-18: the Clone Stamp's history step is named
// in English and German (the core names the step `clone_stamp`).

const translate =
  (table: Record<string, string>): Translate =>
  (key, params) =>
    (table[key] ?? key).replace(/\{(\w+)\}/g, (m, k: string) =>
      params && k in params ? String(params[k]) : m,
    );

test('the Clone Stamp step is named in English and German (AC-18)', () => {
  expect(historyStepName('clone_stamp', translate(en))).toBe('Clone Stamp');
  expect(historyStepName('clone_stamp', translate(de))).toBe('Kopierstempel');
});
