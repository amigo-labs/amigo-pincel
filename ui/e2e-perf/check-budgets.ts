// Budget check for the performance benchmarks
// (docs/specs/performance-benchmarks.md AC-04 / AC-08, budgets in
// docs/specs/pincel.md §4.5).
//
// Two modes:
// - budget (reference machine): every measured metric with a budget is
//   compared with it. A budget never met is `pending` and passes; once a
//   run meets it (recorded in the met-state file), a value more than the
//   tolerance (10 %) over it fails.
// - baseline (CI runner): every metric is compared with the last recorded
//   value of the same runner; more than 10 % worse fails. Budgets are not
//   checked. A run without failures becomes the runner's new baseline.
//
// Inputs are the criterion output (`target/criterion/**/new/*.json`,
// median per bench) and the browser script's results JSON. All metrics
// are lower-is-better.
//
// Runs directly under Node's type stripping (`node e2e-perf/check-budgets.ts`),
// so this file imports nothing local and uses only erasable TypeScript.

import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** One budget row of `budgets.json`. */
export interface Budget {
  readonly metric: string;
  readonly scenario: string;
  readonly max: number;
  readonly unit: string;
  readonly owner: string;
}

/** The parsed `budgets.json`. */
export interface BudgetFile {
  readonly tolerance: number;
  readonly budgets: readonly Budget[];
}

/** Measured values by metric id (`bench/<id>` or `browser/<scenario>/<metric>`). */
export type Metrics = Readonly<Record<string, number>>;

/** Metric id → date the budget was first met (budget mode state). */
export type MetState = Readonly<Record<string, string>>;

/** The last recorded values of one CI runner (baseline mode state). */
export interface Baseline {
  readonly runner: string;
  readonly date: string;
  readonly values: Metrics;
}

/** The browser script's results file (`e2e-perf/results/browser.json`). */
export interface BrowserResults {
  readonly date: string;
  readonly machine: string;
  readonly scenarios: Readonly<
    Record<string, { readonly label: string; readonly metrics: Metrics }>
  >;
}

export type Status =
  'met' | 'within-tolerance' | 'pending' | 'over-budget' | 'no-budget' | 'ok' | 'regressed' | 'new';

/** The verdict for one metric. */
export interface Finding {
  readonly metric: string;
  readonly label: string;
  readonly status: Status;
  readonly measured: number;
  /** The budget (budget mode) or the baseline value (baseline mode). */
  readonly reference: number | null;
  readonly unit: string;
  readonly failed: boolean;
}

export interface BudgetOutcome {
  readonly findings: readonly Finding[];
  /** The met-state including budgets first met by this run. */
  readonly met: MetState;
}

export interface BaselineOutcome {
  readonly findings: readonly Finding[];
  /** The runner's baseline after this run (unchanged when a value regressed). */
  readonly baseline: Baseline;
}

/** `value` is at most `tolerance` over `reference` (exactly at the limit passes). */
export function withinTolerance(value: number, reference: number, tolerance: number): boolean {
  const limit = reference * (1 + tolerance);
  return value - limit <= Math.abs(limit) * 1e-9;
}

function unitOf(metric: string): string {
  return metric.endsWith('-mb') ? 'MB' : 'ms';
}

/** Budget mode (AC-04): compares `metrics` with the budgets. */
export function checkBudgets(
  metrics: Metrics,
  file: BudgetFile,
  met: MetState,
  date: string,
): BudgetOutcome {
  const byMetric = new Map(file.budgets.map((b) => [b.metric, b]));
  const nextMet: Record<string, string> = { ...met };
  const findings: Finding[] = [];
  for (const [metric, measured] of Object.entries(metrics)) {
    const budget = byMetric.get(metric);
    if (!budget) {
      findings.push({
        metric,
        label: metric,
        status: 'no-budget',
        measured,
        reference: null,
        unit: unitOf(metric),
        failed: false,
      });
      continue;
    }
    const base = {
      metric,
      label: budget.scenario,
      measured,
      reference: budget.max,
      unit: budget.unit,
    };
    if (measured <= budget.max) {
      nextMet[metric] ??= date;
      findings.push({ ...base, status: 'met', failed: false });
    } else if (met[metric] === undefined) {
      findings.push({ ...base, status: 'pending', failed: false });
    } else if (withinTolerance(measured, budget.max, file.tolerance)) {
      findings.push({ ...base, status: 'within-tolerance', failed: false });
    } else {
      findings.push({ ...base, status: 'over-budget', failed: true });
    }
  }
  return { findings, met: nextMet };
}

/** Baseline mode (AC-08): compares `metrics` with the runner's last values. */
export function checkBaseline(
  metrics: Metrics,
  baseline: Baseline | null,
  tolerance: number,
  runner: string,
  date: string,
): BaselineOutcome {
  const previous = baseline?.values ?? {};
  const findings: Finding[] = Object.entries(metrics).map(([metric, measured]) => {
    const reference = previous[metric];
    const base = { metric, label: metric, measured, unit: unitOf(metric) };
    if (reference === undefined) {
      return { ...base, reference: null, status: 'new', failed: false };
    }
    const failed = !withinTolerance(measured, reference, tolerance);
    return { ...base, reference, status: failed ? 'regressed' : 'ok', failed };
  });
  const regressed = findings.some((f) => f.failed);
  const next: Baseline =
    regressed && baseline ? baseline : { runner, date, values: { ...previous, ...metrics } };
  return { findings, baseline: next };
}

/** Flattens the browser results into `browser/<scenario>/<metric>` ids. */
export function browserMetrics(results: BrowserResults): Metrics {
  const out: Record<string, number> = {};
  for (const [id, scenario] of Object.entries(results.scenarios)) {
    for (const [name, value] of Object.entries(scenario.metrics)) {
      out[`browser/${id}/${name}`] = value;
    }
  }
  return out;
}

/** Reads every criterion bench under `dir` as `bench/<full_id>` → median in ms. */
export function criterionMetrics(dir: string): Metrics {
  const out: Record<string, number> = {};
  const walk = (path: string): void => {
    for (const entry of readdirSync(path, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const child = join(path, entry.name);
      if (entry.name === 'new') {
        const bench = readJson<{ full_id: string }>(join(child, 'benchmark.json'));
        const estimates = readJson<{ median: { point_estimate: number } }>(
          join(child, 'estimates.json'),
        );
        out[`bench/${bench.full_id}`] = estimates.median.point_estimate / 1e6;
      } else if (entry.name !== 'base' && entry.name !== 'report') {
        walk(child);
      }
    }
  };
  walk(dir);
  return out;
}

function fmt(value: number, unit: string): string {
  const digits = value >= 100 ? 0 : value >= 1 ? 2 : 4;
  return `${value.toFixed(digits)} ${unit}`;
}

/** One report line; failures name the scenario, the reference and the value. */
export function formatFinding(f: Finding, mode: 'budget' | 'baseline'): string {
  const tag = f.failed ? 'FAIL' : f.status === 'pending' ? 'PENDING' : 'ok';
  const what = mode === 'budget' ? 'budget' : 'baseline';
  const ref =
    f.reference === null
      ? mode === 'budget'
        ? 'no budget'
        : 'no baseline yet'
      : `${what} ${fmt(f.reference, f.unit)}`;
  const delta =
    f.reference === null || f.reference === 0
      ? ''
      : ` (${f.measured >= f.reference ? '+' : ''}${(((f.measured - f.reference) / f.reference) * 100).toFixed(1)} %)`;
  const name = f.label === f.metric ? f.metric : `${f.label} [${f.metric}]`;
  return `${tag.padEnd(7)} ${name}: ${ref}, measured ${fmt(f.measured, f.unit)}${delta} — ${f.status}`;
}

/** Markdown rows for the spec's `## Measurements` table (AC-07). */
export function measurementsTable(metrics: Metrics): string {
  const rows = Object.entries(metrics)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([metric, value]) => `| \`${metric}\` | ${fmt(value, unitOf(metric))} |`);
  return ['| Metric | Value |', '| ------ | ----- |', ...rows].join('\n');
}

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, 'utf8')) as T;
}

function writeJson(path: string, value: unknown): void {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

interface Options {
  mode: 'budget' | 'baseline';
  browser: string | null;
  criterion: string | null;
  budgets: string;
  met: string;
  baseline: string | null;
  runner: string;
  write: boolean;
  markdown: string | null;
}

function parseArgs(argv: readonly string[], here: string): Options {
  const opts: Options = {
    mode: 'budget',
    browser: null,
    criterion: null,
    budgets: join(here, 'budgets.json'),
    met: join(here, 'budgets-met.json'),
    baseline: null,
    runner: 'local',
    write: false,
    markdown: null,
  };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = (): string => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${flag} needs a value`);
      return v;
    };
    switch (flag) {
      case '--mode': {
        const m = value();
        if (m !== 'budget' && m !== 'baseline') throw new Error(`unknown mode "${m}"`);
        opts.mode = m;
        break;
      }
      case '--browser':
        opts.browser = value();
        break;
      case '--criterion':
        opts.criterion = value();
        break;
      case '--budgets':
        opts.budgets = value();
        break;
      case '--met':
        opts.met = value();
        break;
      case '--baseline':
        opts.baseline = value();
        break;
      case '--runner':
        opts.runner = value();
        break;
      case '--markdown':
        opts.markdown = value();
        break;
      case '--write':
        opts.write = true;
        break;
      default:
        throw new Error(`unknown argument "${flag}"`);
    }
  }
  return opts;
}

/**
 * Runs the check; returns the process exit code (0 pass, 1 fail, 2 usage).
 * `log` receives the report lines.
 */
export function main(
  argv: readonly string[],
  log: (line: string) => void = console.log,
  here: string = fileURLToPath(new URL('.', import.meta.url)),
): number {
  let opts: Options;
  try {
    opts = parseArgs(argv, here);
  } catch (e) {
    log(`usage error: ${(e as Error).message}`);
    return 2;
  }
  const metrics: Record<string, number> = {};
  if (opts.criterion && existsSync(opts.criterion)) {
    Object.assign(metrics, criterionMetrics(resolve(opts.criterion)));
  }
  if (opts.browser) {
    Object.assign(metrics, browserMetrics(readJson<BrowserResults>(opts.browser)));
  }
  if (Object.keys(metrics).length === 0) {
    log('usage error: no measurements (pass --criterion and/or --browser)');
    return 2;
  }
  const file = readJson<BudgetFile>(opts.budgets);
  const date = new Date().toISOString().slice(0, 10);
  let findings: readonly Finding[];
  if (opts.mode === 'budget') {
    const met = existsSync(opts.met) ? readJson<MetState>(opts.met) : {};
    const outcome = checkBudgets(metrics, file, met, date);
    findings = outcome.findings;
    if (opts.write) writeJson(opts.met, outcome.met);
  } else {
    if (!opts.baseline) {
      log('usage error: baseline mode needs --baseline <file>');
      return 2;
    }
    const baseline = existsSync(opts.baseline) ? readJson<Baseline>(opts.baseline) : null;
    const outcome = checkBaseline(metrics, baseline, file.tolerance, opts.runner, date);
    findings = outcome.findings;
    if (opts.write) writeJson(opts.baseline, outcome.baseline);
  }
  for (const f of findings) log(formatFinding(f, opts.mode));
  if (opts.markdown) writeFileSync(opts.markdown, `${measurementsTable(metrics)}\n`);
  const failed = findings.filter((f) => f.failed).length;
  log(
    failed > 0
      ? `${failed} metric(s) regressed more than ${file.tolerance * 100} % (${opts.mode} mode)`
      : `all ${findings.length} metric(s) pass (${opts.mode} mode)`,
  );
  return failed > 0 ? 1 : 0;
}

const invoked = process.argv[1];
if (invoked !== undefined && resolve(invoked) === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
