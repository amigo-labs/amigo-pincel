import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';
import { defaultValues, EFFECTS, forMode, toParams } from '../src/lib/effects/catalog';

// The desktop app's native job backend (docs/specs/performance-desktop-jobs.md
// AC-01): for the same input, its results are byte-identical to the Web
// Worker backend's. Playwright cannot run the native backend, so the two
// meet at shared fixtures: this spec runs every job case in the real Image
// job worker of the built bundle and checks its results against the digests
// in `src-tauri/tests/fixtures/jobs/cases.json`, the reference a native test
// in `src-tauri/src/native_jobs.rs` compares the native results with (see
// the spec's open question on float math).
//
// PINCEL_RECORD_JOB_DIGESTS=1 records the digests (and the case list) from
// the worker instead, after a change to the fixture document or the jobs.

const root = fileURLToPath(new URL('../../', import.meta.url));
const fixtures = `${root}src-tauri/tests/fixtures/jobs/`;

/** A job as the native backend takes it: the worker's `ImageJob` without
 *  its snapshot (`encode` and `load` get `doc.snapshot`). */
type Job = Record<string, unknown> & { kind: string };

interface Case {
  job: Job;
  len: number;
  fnv1a64: string;
}

/** FNV-1a, 64 bit, as lowercase hex (the native test computes the same). */
function fnv1a64(bytes: Uint8Array): string {
  let hash = 0xcbf29ce484222325n;
  for (const b of bytes) {
    hash ^= BigInt(b);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, '0');
}

const IMAGE_EFFECTS = EFFECTS.filter((e) => e.modes.includes('image')).map((e) =>
  forMode(e, 'image'),
);

/** Every job kind: the effect copy, both effect jobs for every Image
 *  effect at its defaults (and Gaussian blur at radius 10), and every
 *  encode format. Effects run on the translucent top layer, which the
 *  fixture's selection clips. */
function allJobs(): Job[] {
  const effect = (name: string, params: number[]): Job[] => [
    { kind: 'preview', key: 'fx', effect: { layer: 1, name, params } },
    { kind: 'region', key: 'fx', effect: { layer: 1, name, params } },
  ];
  const encode = (format: string, level = 6, quality = 90): Job => ({
    kind: 'encode',
    format,
    level,
    quality,
  });
  return [
    { kind: 'load', key: 'fx' },
    ...IMAGE_EFFECTS.flatMap((def) => effect(def.id, toParams(def, defaultValues(def)))),
    ...effect('gaussian_blur', [10]),
    encode('ora'),
    encode('ora', 0),
    encode('png'),
    encode('png', 9),
    encode('jpeg'),
    encode('jpeg', 6, 40),
    encode('webp'),
    encode('bmp'),
    encode('tiff'),
    encode('tga'),
    encode('ico'),
  ];
}

/** The built bundle's Image job worker script. */
function workerScript(): string {
  const assets = `${root}ui/dist/assets/`;
  const name = readdirSync(assets).find(
    (f) =>
      /^worker-.*\.js$/.test(f) &&
      readFileSync(`${assets}${f}`, 'utf8').includes('no document loaded as'),
  );
  if (!name) throw new Error('no Image job worker in ui/dist/assets (run pnpm build)');
  return `./assets/${name}`;
}

test('the Web Worker backend gives the recorded result for every job kind (AC-01)', async ({
  page,
}) => {
  test.setTimeout(120_000);
  const record = Boolean(process.env.PINCEL_RECORD_JOB_DIGESTS);
  const snapshot = [...readFileSync(`${fixtures}doc.snapshot`)];
  const recorded: Case[] = record
    ? []
    : (JSON.parse(readFileSync(`${fixtures}cases.json`, 'utf8')) as { cases: Case[] }).cases;
  const jobs = record ? allJobs() : recorded.map((c) => c.job);

  await page.goto('./');
  const results = await page.evaluate(
    async ({ url, snapshot, jobs }) => {
      const worker = new Worker(url, { type: 'module' });
      // No compiled module: the worker loads the core itself.
      worker.postMessage({ init: null });
      let next = 1;
      const send = (req: unknown): Promise<Uint8Array> =>
        new Promise((resolve, reject) => {
          const id = next++;
          const on = (e: MessageEvent): void => {
            const msg = e.data as { id: number; ok: boolean; res?: Uint8Array; error?: string };
            if (msg.id !== id) return;
            worker.removeEventListener('message', on);
            if (msg.ok && msg.res) resolve(msg.res);
            else reject(new Error(msg.error ?? 'no result'));
          };
          worker.addEventListener('message', on);
          worker.postMessage({ id, req });
        });
      const snap = new Uint8Array(snapshot);
      const out: ({ bytes: number[] } | { error: string })[] = [];
      for (const job of jobs) {
        const withBody = job.kind === 'encode' || job.kind === 'load';
        try {
          const bytes = await send(withBody ? { ...job, snapshot: snap } : job);
          out.push({ bytes: Array.from(bytes) });
        } catch (err) {
          out.push({ error: String(err) });
        }
      }
      worker.terminate();
      return out;
    },
    { url: workerScript(), snapshot, jobs },
  );

  const cases = results.map((r, i): Case | string => {
    const job = jobs[i] as Job;
    if ('error' in r) return `${JSON.stringify(job)}: ${r.error}`;
    const bytes = Uint8Array.from(r.bytes);
    return { job, len: bytes.length, fnv1a64: fnv1a64(bytes) };
  });
  expect(cases.filter((c) => typeof c === 'string')).toEqual([]);
  if (record) {
    // One case per line, so a change shows as the cases it touches.
    const lines = (cases as Case[]).map((c) => `    ${JSON.stringify(c)}`);
    writeFileSync(`${fixtures}cases.json`, `{\n  "cases": [\n${lines.join(',\n')}\n  ]\n}\n`);
    return;
  }
  expect(cases).toEqual(recorded);
  // A new Image effect needs its cases recorded.
  const covered = new Set(
    recorded.map((c) => (c.job.effect as { name?: string } | undefined)?.name),
  );
  expect(IMAGE_EFFECTS.map((e) => e.id).filter((id) => !covered.has(id))).toEqual([]);
});
