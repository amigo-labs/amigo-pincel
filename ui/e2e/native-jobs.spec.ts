import { expect, test } from '@playwright/test';
import { jobBackend, NATIVE_JOB_MIN_PIXELS, nativeFirst } from '../src/lib/jobs/native';
import type { JobRunner, JobSetup } from '../src/lib/jobs/runner';

// The desktop app's job routing (docs/specs/performance-desktop-jobs.md
// AC-03, AC-06), driven without a page: `jobBackend` and `nativeFirst` are
// rune-free. The wiring in the editor is covered by
// `desktop-jobs-routing.spec.ts` against a mocked Tauri bridge.

type Req = { readonly kind: string; readonly key?: string };

/** A worker runner that records what it ran and answers `answer`. */
function fakeWorker(answer: (req: Req) => Promise<string>): JobRunner<Req, string> & {
  ran: { req: Req; setup: JobSetup<Req> | undefined }[];
  released: string[];
} {
  const ran: { req: Req; setup: JobSetup<Req> | undefined }[] = [];
  const released: string[] = [];
  return {
    ran,
    released,
    run(req, _fallback, setup) {
      ran.push({ req, setup });
      return answer(req);
    },
    release(key) {
      released.push(key);
    },
  };
}

/** Collects `console.error` calls while `body` runs. */
async function logged(body: () => Promise<void>): Promise<string[]> {
  const lines: string[] = [];
  const original = console.error;
  console.error = (...args: unknown[]) => lines.push(args.map(String).join(' '));
  try {
    await body();
  } finally {
    console.error = original;
  }
  return lines;
}

test('jobs from the crossover size up run natively in the desktop app only (AC-03)', () => {
  const min = 1_000_000;
  expect(jobBackend(true, min, 1, min)).toBe('native');
  expect(jobBackend(true, 1000, 1000, min)).toBe('native');
  expect(jobBackend(true, 3840, 2160, min)).toBe('native');
  // Just below the crossover size: the worker.
  expect(jobBackend(true, min - 1, 1, min)).toBe('worker');
  expect(jobBackend(true, 16, 16, min)).toBe('worker');
  // The PWA: always the worker.
  expect(jobBackend(false, 3840, 2160, min)).toBe('worker');
  expect(jobBackend(false, 16, 16, min)).toBe('worker');
});

test('without a crossover size every job runs in the worker (AC-03)', () => {
  // No size pays off over Tauri's IPC yet (the spec's Decisions).
  expect(NATIVE_JOB_MIN_PIXELS).toBeNull();
  expect(jobBackend(true, 3840, 2160)).toBe('worker');
  expect(jobBackend(true, 8192, 8192, null)).toBe('worker');
});

test('a native job that succeeds never reaches the worker (AC-03)', async () => {
  const calls: Req[] = [];
  const worker = fakeWorker(async () => 'worker');
  const runner = nativeFirst<Req, string>(
    async (req) => {
      calls.push(req);
      return `native ${req.kind}`;
    },
    worker,
    'test jobs',
  );
  const setup: JobSetup<Req> = { key: 'fx', req: () => ({ kind: 'load', key: 'fx' }) };
  const lines = await logged(async () => {
    expect(await runner.run({ kind: 'encode' }, () => 'main')).toBe('native encode');
    expect(await runner.run({ kind: 'preview' }, () => 'main', setup)).toBe('native preview');
    expect(await runner.run({ kind: 'region' }, () => 'main', setup)).toBe('native region');
  });
  // The setup is loaded once, ahead of its first job.
  expect(calls.map((c) => c.kind)).toEqual(['encode', 'load', 'preview', 'region']);
  expect(worker.ran).toEqual([]);
  expect(lines).toEqual([]);
  runner.release('fx', { kind: 'drop', key: 'fx' });
  expect(calls.at(-1)?.kind).toBe('drop');
});

test('a failed or missing native job runs in the worker, logged, without an error (AC-06)', async () => {
  for (const failure of ['the job failed', 'command image_job not found']) {
    const worker = fakeWorker(async (req) => `worker ${req.kind}`);
    const runner = nativeFirst<Req, string>(
      async () => {
        throw new Error(failure);
      },
      worker,
      'test jobs',
    );
    const setup: JobSetup<Req> = { key: 'fx', req: () => ({ kind: 'load', key: 'fx' }) };
    const fallback = (): string => 'main';
    let results: string[] = [];
    const lines = await logged(async () => {
      results = [
        await runner.run({ kind: 'encode' }, fallback),
        await runner.run({ kind: 'preview' }, fallback, setup),
      ];
    });
    expect(results).toEqual(['worker encode', 'worker preview']);
    // The worker gets the same job, setup and all.
    expect(worker.ran).toEqual([
      { req: { kind: 'encode' }, setup: undefined },
      { req: { kind: 'preview' }, setup },
    ]);
    expect(lines).toHaveLength(2);
    expect(lines.every((l) => l.includes('native job failed') && l.includes(failure))).toBe(true);
    // The worker frees its copy; the native backend never loaded one.
    runner.release('fx', { kind: 'drop', key: 'fx' });
    expect(worker.released).toEqual(['fx']);
  }
});

test('a setup is loaded natively again after a native failure (AC-06)', async () => {
  const calls: string[] = [];
  let fail = true;
  const runner = nativeFirst<Req, string>(
    async (req) => {
      calls.push(req.kind);
      if (fail && req.kind === 'preview') throw new Error('lost');
      return req.kind;
    },
    fakeWorker(async () => 'worker'),
    'test jobs',
  );
  const setup: JobSetup<Req> = { key: 'fx', req: () => ({ kind: 'load', key: 'fx' }) };
  await logged(async () => {
    expect(await runner.run({ kind: 'preview' }, () => 'main', setup)).toBe('worker');
    fail = false;
    expect(await runner.run({ kind: 'preview' }, () => 'main', setup)).toBe('preview');
  });
  expect(calls).toEqual(['load', 'preview', 'load', 'preview']);
});

test('the user sees an error only if the worker path fails too (AC-06)', async () => {
  const runner = nativeFirst<Req, string>(
    async () => {
      throw new Error('native down');
    },
    fakeWorker(async () => {
      throw new Error('main-thread run failed');
    }),
    'test jobs',
  );
  await logged(async () => {
    await expect(runner.run({ kind: 'encode' }, () => 'main')).rejects.toThrow(
      'main-thread run failed',
    );
  });
});
