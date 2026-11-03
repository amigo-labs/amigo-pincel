import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

// Where the performance benchmarks run (docs/specs/performance-benchmarks.md
// AC-05, AC-06): never in the PR gate, always in the performance workflow.
// Static checks of the workflow files — no browser, no app.

const root = fileURLToPath(new URL('../../', import.meta.url));
const read = (path: string): string => readFileSync(`${root}${path}`, 'utf8');
const PERF = '.github/workflows/perf.yml';

test('the PR gate runs neither the benchmarks nor the browser performance script (AC-05)', () => {
  const ci = read('.github/workflows/ci.yml');
  expect(ci).not.toMatch(/cargo\s+bench/);
  expect(ci).not.toMatch(/--benches|--all-targets[^\n]*cargo test|cargo test[^\n]*--all-targets/);
  expect(ci).not.toMatch(/pnpm (run )?perf\b/);
  expect(ci).not.toContain('playwright.perf.config');
  expect(ci).not.toContain('e2e-perf');
  // `pnpm test:e2e` collects `ui/e2e` only; the browser script lives in `ui/e2e-perf`.
  expect(read('ui/playwright.config.ts')).toMatch(/testDir:\s*'e2e'/);
  expect(read('ui/package.json')).toMatch(/"test:e2e":\s*"playwright test"/);
});

test('the performance workflow runs benches, browser script and budget check on dispatch and nightly (AC-06)', () => {
  expect(existsSync(`${root}${PERF}`)).toBe(true);
  const wf = read(PERF);
  // Triggers: manual dispatch and a nightly schedule, nothing else.
  expect(wf).toMatch(/^on:\s*\n\s+workflow_dispatch:/m);
  expect(wf).toMatch(/^\s+schedule:\s*\n\s+- cron: '[^']+'/m);
  expect(wf).not.toMatch(/^\s+(pull_request|push):/m);
  // The nightly run skips when main has no new commit since the last run.
  expect(wf).toContain('gh run list');
  expect(wf).toMatch(/headSha/);
  expect(wf).toMatch(/if: needs\.\w+\.outputs\.run == 'true'/);
  // All three pieces run.
  expect(wf).toContain('cargo bench -p pincel-image-core --bench compose');
  expect(wf).toContain('cargo bench -p pincel-core --bench compose');
  expect(wf).toMatch(/pnpm perf\s*$/m);
  expect(wf).toMatch(/pnpm perf:check[^\n]*--mode baseline/);
  // The budget check's failure fails the workflow.
  expect(wf).not.toContain('continue-on-error');
});

// A GitHub-hosted runner label covers several CPU models. Comparing a run on
// a slower CPU with a baseline from a faster one flags every metric as a
// regression (nightly run 41422525935: +60 – 120 % on unchanged code), so
// "the same runner" (AC-08) is OS, architecture and CPU model.
test('the runner baseline is kept per CPU model (AC-08)', () => {
  const wf = read(PERF);
  expect(wf).toMatch(/\/proc\/cpuinfo/);
  const key = /key: perf-baseline-\$\{\{ steps\.runner\.outputs\.id \}\}-\$\{\{ github\.run_id \}\}/;
  expect(wf.match(new RegExp(key.source, 'g'))).toHaveLength(2);
  expect(wf).toMatch(/restore-keys: perf-baseline-\$\{\{ steps\.runner\.outputs\.id \}\}-\s*$/m);
  expect(wf).toMatch(/pnpm perf:check[^\n]*--runner "\$\{\{ steps\.runner\.outputs\.id \}\}"/);
});
