import { expect, test } from '@playwright/test';
import { latestOnly } from '../src/lib/jobs/latest';

// Latest-only scheduling of effect previews
// (docs/specs/performance-background-jobs.md AC-06, AC-08), driven with
// jobs that finish when the test says so.

interface Job {
  readonly params: number;
  finish(): Promise<void>;
}

function harness() {
  const jobs: Job[] = [];
  const shown: number[] = [];
  const failed: unknown[] = [];
  const queue = latestOnly<number, number>(
    (params) =>
      new Promise<number>((resolve) => {
        jobs.push({
          params,
          finish: async () => {
            resolve(params * 10);
            // Let the result and the next start run.
            for (let i = 0; i < 5; i++) await Promise.resolve();
          },
        });
      }),
    (result) => shown.push(result),
    (err) => failed.push(err),
  );
  return { queue, jobs, shown, failed };
}

test('a result for older parameters never replaces a newer one (AC-06)', async () => {
  const { queue, jobs, shown } = harness();
  queue.request(1);
  queue.request(2);
  queue.request(3);
  // One job at a time: 2 was superseded while 1 ran and never starts.
  expect(jobs.map((j) => j.params)).toEqual([1]);
  await jobs[0]?.finish();
  expect(shown, 'the result for 1 is stale').toEqual([]);
  expect(jobs.map((j) => j.params)).toEqual([1, 3]);
  await jobs[1]?.finish();
  expect(shown).toEqual([30]);
});

test('an idle queue shows every request’s result', async () => {
  const { queue, jobs, shown } = harness();
  queue.request(1);
  await jobs[0]?.finish();
  queue.request(2);
  await jobs[1]?.finish();
  expect(shown).toEqual([10, 20]);
});

test('cancel discards the running result and the waiting request (AC-08)', async () => {
  const { queue, jobs, shown, failed } = harness();
  queue.request(1);
  queue.request(2);
  queue.cancel();
  await jobs[0]?.finish();
  expect(jobs).toHaveLength(1);
  expect(shown).toEqual([]);
  expect(failed).toEqual([]);
});

test('only the newest request’s failure is reported', async () => {
  const failed: unknown[] = [];
  let reject: (err: Error) => void = () => undefined;
  const queue = latestOnly<number, number>(
    () => new Promise<number>((_, r) => (reject = r)),
    () => undefined,
    (err) => failed.push(err),
  );
  queue.request(1);
  queue.request(2);
  reject(new Error('stale'));
  for (let i = 0; i < 5; i++) await Promise.resolve();
  expect(failed).toEqual([]);
  reject(new Error('newest'));
  for (let i = 0; i < 5; i++) await Promise.resolve();
  expect(failed.map((e) => (e as Error).message)).toEqual(['newest']);
});
