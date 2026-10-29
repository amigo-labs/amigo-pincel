import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import {
  type Baseline,
  type BudgetFile,
  type BrowserResults,
  checkBaseline,
  checkBudgets,
  criterionMetrics,
  main,
} from '../e2e-perf/check-budgets';

// The budget check of docs/specs/performance-benchmarks.md (AC-04 budget
// mode, AC-08 baseline mode). Pure Node — no browser, no app.

const BUDGETS: BudgetFile = {
  tolerance: 0.1,
  budgets: [
    {
      metric: 'browser/image-brush-4k/dab-latency-p95',
      scenario: 'Brush dab, Image 4K',
      max: 16,
      unit: 'ms',
      owner: 'performance-image-cache',
    },
  ],
};
const DAB = 'browser/image-brush-4k/dab-latency-p95';
const DATE = '2026-10-29';

test.describe('budget mode (AC-04)', () => {
  test('a met budget exceeded by more than 10 % fails and names scenario, budget, value', () => {
    const met = { [DAB]: '2026-10-01' };
    const { findings } = checkBudgets({ [DAB]: 17.7 }, BUDGETS, met, DATE);
    expect(findings).toHaveLength(1);
    expect(findings[0]).toMatchObject({ status: 'over-budget', failed: true, reference: 16 });

    const dir = test.info().outputPath();
    mkdirSync(dir, { recursive: true });
    const browser = writeResults(dir, { [DAB]: 17.7 });
    const metFile = join(dir, 'met.json');
    writeFileSync(metFile, JSON.stringify(met));
    const budgets = join(dir, 'budgets.json');
    writeFileSync(budgets, JSON.stringify(BUDGETS));
    const lines: string[] = [];
    const code = main(
      ['--mode', 'budget', '--browser', browser, '--budgets', budgets, '--met', metFile],
      (l) => lines.push(l),
    );
    expect(code).not.toBe(0);
    const fail = lines.find((l) => l.startsWith('FAIL'));
    expect(fail).toContain('Brush dab, Image 4K');
    expect(fail).toContain('budget 16.00 ms');
    expect(fail).toContain('measured 17.70 ms');
  });

  test('a met budget exceeded by at most 10 % passes, exactly 10 % included', () => {
    const met = { [DAB]: '2026-10-01' };
    for (const value of [16, 17, 17.6]) {
      const { findings } = checkBudgets({ [DAB]: value }, BUDGETS, met, DATE);
      expect(findings[0]?.failed, `measured ${value}`).toBe(false);
    }
  });

  test('a never-met budget is pending with its value and passes; meeting it makes it block', () => {
    const first = checkBudgets({ [DAB]: 40 }, BUDGETS, {}, DATE);
    expect(first.findings[0]).toMatchObject({ status: 'pending', failed: false, measured: 40 });
    expect(first.met).toEqual({});

    const meets = checkBudgets({ [DAB]: 15 }, BUDGETS, first.met, DATE);
    expect(meets.findings[0]).toMatchObject({ status: 'met', failed: false });
    expect(meets.met).toEqual({ [DAB]: DATE });

    const after = checkBudgets({ [DAB]: 40 }, BUDGETS, meets.met, '2026-10-30');
    expect(after.findings[0]).toMatchObject({ status: 'over-budget', failed: true });
  });

  test('the CLI reports pending, exits zero and records a first met budget with --write', () => {
    const dir = test.info().outputPath();
    mkdirSync(dir, { recursive: true });
    const budgets = join(dir, 'budgets.json');
    writeFileSync(budgets, JSON.stringify(BUDGETS));
    const metFile = join(dir, 'met.json');
    const lines: string[] = [];
    const args = ['--mode', 'budget', '--budgets', budgets, '--met', metFile, '--write'];
    expect(
      main([...args, '--browser', writeResults(dir, { [DAB]: 40 })], (l) => lines.push(l)),
    ).toBe(0);
    expect(lines.some((l) => l.startsWith('PENDING') && l.includes('measured 40.00 ms'))).toBe(
      true,
    );
    expect(JSON.parse(readFileSync(metFile, 'utf8'))).toEqual({});

    expect(main([...args, '--browser', writeResults(dir, { [DAB]: 12 })], () => {})).toBe(0);
    expect(Object.keys(JSON.parse(readFileSync(metFile, 'utf8')))).toEqual([DAB]);
    expect(main([...args, '--browser', writeResults(dir, { [DAB]: 40 })], () => {})).toBe(1);
  });

  test('a scenario without a budget is reported and skipped', () => {
    const { findings } = checkBudgets({ 'browser/x/frame-p50': 99 }, BUDGETS, {}, DATE);
    expect(findings[0]).toMatchObject({ status: 'no-budget', failed: false, measured: 99 });
  });
});

test.describe('baseline mode (AC-08)', () => {
  const baseline: Baseline = { runner: 'ubuntu', date: '2026-10-28', values: { [DAB]: 100 } };

  test('more than 10 % worse than the baseline fails and names scenario, baseline, value', () => {
    const { findings, baseline: next } = checkBaseline(
      { [DAB]: 111 },
      baseline,
      0.1,
      'ubuntu',
      DATE,
    );
    expect(findings[0]).toMatchObject({ status: 'regressed', failed: true, reference: 100 });
    // A regression never becomes the new baseline.
    expect(next).toEqual(baseline);

    const dir = test.info().outputPath();
    mkdirSync(dir, { recursive: true });
    const file = join(dir, 'baseline.json');
    writeFileSync(file, JSON.stringify(baseline));
    const lines: string[] = [];
    const code = main(
      ['--mode', 'baseline', '--baseline', file, '--browser', writeResults(dir, { [DAB]: 111 })],
      (l) => lines.push(l),
    );
    expect(code).toBe(1);
    const fail = lines.find((l) => l.startsWith('FAIL'));
    expect(fail).toContain(DAB);
    expect(fail).toContain('baseline 100 ms');
    expect(fail).toContain('measured 111 ms');
  });

  test('within 10 %, exactly 10 % included, passes and becomes the new baseline', () => {
    const { findings, baseline: next } = checkBaseline(
      { [DAB]: 110 },
      baseline,
      0.1,
      'ubuntu',
      DATE,
    );
    expect(findings[0]).toMatchObject({ status: 'ok', failed: false });
    expect(next).toEqual({ runner: 'ubuntu', date: DATE, values: { [DAB]: 110 } });
  });

  test('budgets are not checked: far over budget but equal to the baseline passes', () => {
    const dir = test.info().outputPath();
    mkdirSync(dir, { recursive: true });
    const file = join(dir, 'baseline.json');
    writeFileSync(file, JSON.stringify(baseline));
    const budgets = join(dir, 'budgets.json');
    writeFileSync(budgets, JSON.stringify(BUDGETS));
    const args = ['--mode', 'baseline', '--baseline', file, '--budgets', budgets];
    expect(main([...args, '--browser', writeResults(dir, { [DAB]: 100 })], () => {})).toBe(0);
  });

  test('the first run on a new runner records the baseline and passes', () => {
    const dir = test.info().outputPath();
    mkdirSync(dir, { recursive: true });
    const file = join(dir, 'baseline.json');
    const args = ['--mode', 'baseline', '--baseline', file, '--runner', 'new-runner', '--write'];
    expect(main([...args, '--browser', writeResults(dir, { [DAB]: 42 })], () => {})).toBe(0);
    const saved = JSON.parse(readFileSync(file, 'utf8')) as Baseline;
    expect(saved.runner).toBe('new-runner');
    expect(saved.values).toEqual({ [DAB]: 42 });
  });
});

test('criterion medians are read as bench/<full_id> in milliseconds', () => {
  const dir = test.info().outputPath('criterion');
  const bench = join(dir, 'image_compose', 'full_1024_2layers', 'new');
  mkdirSync(bench, { recursive: true });
  writeFileSync(
    join(bench, 'benchmark.json'),
    JSON.stringify({ full_id: 'image_compose/full_1024_2layers' }),
  );
  writeFileSync(
    join(bench, 'estimates.json'),
    JSON.stringify({ median: { point_estimate: 101_000_000 } }),
  );
  expect(criterionMetrics(dir)).toEqual({ 'bench/image_compose/full_1024_2layers': 101 });
});

/** Writes a browser results file holding `metrics` (flat ids) and returns its path. */
function writeResults(dir: string, metrics: Record<string, number>): string {
  const scenarios: Record<string, { label: string; metrics: Record<string, number> }> = {};
  for (const [id, value] of Object.entries(metrics)) {
    const [, scenario = '', name = ''] = id.split('/');
    scenarios[scenario] ??= { label: scenario, metrics: {} };
    scenarios[scenario].metrics[name] = value;
  }
  const results: BrowserResults = { date: DATE, machine: 'test', scenarios };
  const path = join(dir, `browser-${Object.values(metrics).join('-')}.json`);
  writeFileSync(path, JSON.stringify(results));
  return path;
}
