import { expect, test } from '@playwright/test';
import { en } from '../src/lib/i18n/en';
import {
  historyStartName,
  historyStepName,
  type HistoryOrigin,
  type Translate,
} from '../src/lib/history/names';

// docs/specs/history-panel.md: row names, without a page.

const tr: Translate = (key, params) =>
  en[key].replace(/\{(\w+)\}/g, (m, k: string) => (params && k in params ? String(params[k]) : m));

test('a kind the UI has no name for reads "Unknown step" and its id (AC-17)', () => {
  expect(historyStepName('teleport_layer', tr)).toBe('Unknown step teleport_layer');
  expect(historyStepName('effect.no_such_effect', tr)).toBe('Unknown step effect.no_such_effect');
  expect(historyStepName('pencil', tr)).toBe('Pencil');
  expect(historyStepName('brush', tr)).toBe('Brush');
  expect(historyStepName('merge_down', tr)).toBe('Merge Down');
  expect(historyStepName('resize_canvas', tr)).toBe('Resize Canvas');
});

test('an effect step is named as the effect menu names it (AC-04)', () => {
  expect(historyStepName('effect.gaussian_blur', tr)).toBe(en['effect.gaussian_blur']);
  expect(historyStepName('effect.gaussian_blur', tr)).toBe('Gaussian Blur');
});

test('the start entry is named after how the document began, or the oldest state once a step was dropped (AC-03)', () => {
  const names: Record<HistoryOrigin, string> = {
    new: 'New',
    open: 'Open',
    paste: 'Paste',
    recovered: 'Recovered',
    start: 'Start',
  };
  for (const [origin, name] of Object.entries(names) as [HistoryOrigin, string][]) {
    expect(historyStartName(origin, false, tr)).toBe(name);
    expect(historyStartName(origin, true, tr)).toBe('Oldest available state');
  }
});
