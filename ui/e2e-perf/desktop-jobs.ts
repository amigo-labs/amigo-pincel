// Native vs Web Worker job timings in the running desktop app
// (docs/specs/performance-desktop-jobs.md: the crossover size under
// Decisions, AC-04 and AC-05). Manual: it needs the release desktop app on
// the reference machine, started with a debugging port —
//
//   WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 \
//     target/release/pincel.exe
//   node e2e-perf/desktop-jobs.ts [--port 9333] [--reps 5] [--only 3840x2160]
//
// In the app's page it builds document snapshots of several sizes, runs
// the same jobs in the Image job worker of the bundle and in the native
// backend — through the bundle's native fetch worker and the app's loopback
// job endpoint, as the editor does — and prints the median wall time of
// each (transfer included) and the longest frame while it ran.

import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const args = process.argv.slice(2);
const arg = (name: string, fallback: number): number => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? Number(args[i + 1]) : fallback;
};
const port = arg('port', 9333);
const reps = arg('reps', 5);
/** `--only <text>` runs the scenarios whose label contains the text. */
const only = args.includes('--only') ? (args[args.indexOf('--only') + 1] ?? '') : '';

// The desktop build (`tauri build`) puts the bundle at the root of dist/.
const assets = fileURLToPath(new URL('../dist/assets/', import.meta.url));
const workerName = readdirSync(assets).find(
  (f) =>
    /^worker-.*\.js$/.test(f) &&
    readFileSync(`${assets}${f}`, 'utf8').includes('no document loaded as'),
);
if (!workerName) throw new Error('no Image job worker in ui/dist/assets (build the desktop app)');
const nativeWorkerName = readdirSync(assets).find((f) => /^native-worker-.*\.js$/.test(f));
if (!nativeWorkerName) throw new Error('no native fetch worker in ui/dist/assets');

/** One measured job: a document size and layer count, and what to run. */
interface Scenario {
  readonly label: string;
  readonly width: number;
  readonly height: number;
  readonly layers: number;
  readonly job: 'blur10' | 'png' | 'ora';
}

const SIZES: readonly [number, number][] = [
  [256, 256],
  [512, 512],
  [724, 724],
  [1024, 1024],
  [1448, 1448],
  [2048, 2048],
  [3840, 2160],
];
const SCENARIOS: Scenario[] = [
  ...SIZES.flatMap(([width, height]): Scenario[] =>
    (['blur10', 'png', 'ora'] as const).map((job) => ({
      label: `${job} ${width}x${height} 1 layer`,
      width,
      height,
      layers: 1,
      job,
    })),
  ),
  { label: 'ora 3840x2160 10 layers', width: 3840, height: 2160, layers: 10, job: 'ora' },
  { label: 'png 3840x2160 10 layers', width: 3840, height: 2160, layers: 10, job: 'png' },
  // AC-05: an effect session on a 4K, 10-layer document.
  { label: 'blur10 3840x2160 10 layers', width: 3840, height: 2160, layers: 10, job: 'blur10' },
];

const browser = await chromium.connectOverCDP(`http://localhost:${port}`);
const page = browser.contexts().flatMap((c) => c.pages())[0];
if (!page) throw new Error('the desktop app has no page');

console.log('| Scenario | Worker (ms) | Native (ms) | Longest frame worker / native (ms) |');
console.log('| --- | ---: | ---: | ---: |');
for (const s of SCENARIOS.filter((x) => x.label.includes(only))) {
  // A fresh page per scenario: the large snapshots of earlier ones would
  // otherwise pile up until the renderer runs out of memory.
  await page.reload();
  await page.waitForLoadState('load');
  const result = await page.evaluate(
    async ({ s, reps, url, nativeUrl }) => {
      type Bytes = Uint8Array<ArrayBuffer>;
      // A snapshot (codec/snapshot.rs layout) of `s`: noise-like pixels.
      const header = JSON.stringify({
        doc: {
          id: '00000000-0000-4000-8000-000000000001',
          title: 'bench',
          canvas: { width: s.width, height: s.height },
          layers: Array.from({ length: s.layers }, (_, i) => ({
            id: `00000000-0000-4000-8000-${String(i + 2).padStart(12, '0')}`,
            name: `Layer ${i + 1}`,
            kind: 'Raster',
            parent: null,
            pixels: { width: 0, height: 0, data: [] },
            opacity: 1.0,
            blend_mode: i % 2 ? 'Multiply' : 'Normal',
            visible: true,
            locked: false,
          })),
          active_layer: 0,
          selection: null,
          metadata: { dpi: 96.0, color_profile: 'Srgb', created_at: 0, modified_at: 0 },
        },
        sizes: Array.from({ length: s.layers }, () => [s.width, s.height]),
        selection: null,
      });
      const head = new TextEncoder().encode(header);
      const layerBytes = s.width * s.height * 4;
      const snapshot: Bytes = new Uint8Array(12 + head.length + layerBytes * s.layers);
      snapshot.set(new TextEncoder().encode('PNCLSNAP'), 0);
      new DataView(snapshot.buffer).setUint32(8, head.length, true);
      snapshot.set(head, 12);
      let seed = 1;
      for (let i = 12 + head.length; i < snapshot.length; i += 4) {
        seed = (seed * 1103515245 + 12345) >>> 0;
        snapshot[i] = seed >>> 24;
        snapshot[i + 1] = (seed >>> 16) & 255;
        snapshot[i + 2] = (i >>> 10) & 255;
        snapshot[i + 3] = 160 + ((seed >>> 8) & 63);
      }

      // The longest frame while `run` runs.
      const timed = async (run: () => Promise<unknown>): Promise<[number, number]> => {
        let longest = 0;
        let last = performance.now();
        let on = true;
        const tick = (now: number): void => {
          longest = Math.max(longest, now - last);
          last = now;
          if (on) requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
        const t0 = performance.now();
        await run();
        const ms = performance.now() - t0;
        on = false;
        return [ms, Math.max(longest, performance.now() - last)];
      };

      const worker = new Worker(url, { type: 'module' });
      worker.postMessage({ init: null });
      let next = 1;
      const viaWorker = (req: Record<string, unknown>): Promise<Bytes> =>
        new Promise((resolve, reject) => {
          const id = next++;
          const on = (e: MessageEvent): void => {
            const msg = e.data as { id: number; ok: boolean; res: Bytes; error: string };
            if (msg.id !== id) return;
            worker.removeEventListener('message', on);
            if (msg.ok) resolve(msg.res);
            else reject(new Error(msg.error));
          };
          worker.addEventListener('message', on);
          worker.postMessage({ id, req });
        });
      const internals = (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke(cmd: string): Promise<unknown> };
        }
      ).__TAURI_INTERNALS__;
      const endpoint = (await internals.invoke('job_endpoint')) as { url: string; token: string };
      const fetcher = new Worker(nativeUrl, { type: 'module' });
      const viaNative = (req: Record<string, unknown>): Promise<Bytes> =>
        new Promise((resolve, reject) => {
          const { snapshot: body = new Uint8Array(0), ...job } = req as { snapshot?: Bytes };
          const id = next++;
          const on = (e: MessageEvent): void => {
            const msg = e.data as { id: number; ok: boolean; res: Bytes; error: string };
            if (msg.id !== id) return;
            fetcher.removeEventListener('message', on);
            if (msg.ok) resolve(msg.res);
            else reject(new Error(msg.error));
          };
          fetcher.addEventListener('message', on);
          // As the editor does: an effect session's copy moves, an encode's
          // snapshot is copied (it stays for the worker fallback).
          // Fresh copies are made before the timing starts (`measure`).
          const moved = (job as { kind?: string }).kind === 'load' ? (copies.pop() ?? null) : null;
          fetcher.postMessage(
            { id, ...endpoint, job: JSON.stringify(job), body: moved ?? body },
            moved ? [moved.buffer] : [],
          );
        });

      const effect = { layer: 0, name: 'gaussian_blur', params: [10] };
      const copies: Bytes[] = [];
      const job = async (run: (req: Record<string, unknown>) => Promise<Bytes>) => {
        if (s.job === 'blur10') {
          // One effect session: the copy, then the applied region.
          await run({ kind: 'load', key: 'bench', snapshot });
          await run({ kind: 'region', key: 'bench', effect });
          await run({ kind: 'drop', key: 'bench' });
        } else {
          await run({ kind: 'encode', format: s.job, level: 6, quality: 90, snapshot });
        }
      };
      const median = (xs: number[]): number => {
        const v = [...xs].sort((a, b) => a - b);
        return v[Math.floor(v.length / 2)] ?? NaN;
      };
      const measure = async (run: (req: Record<string, unknown>) => Promise<Bytes>) => {
        // The native effect session's copies, one per run, made untimed.
        if (run === viaNative && s.job === 'blur10') {
          for (let i = 0; i <= reps; i++) copies.push(snapshot.slice());
        }
        await job(run); // warm-up (the worker compiles the core)
        const times: number[] = [];
        let frame = 0;
        for (let i = 0; i < reps; i++) {
          const [ms, longest] = await timed(() => job(run));
          times.push(ms);
          frame = Math.max(frame, longest);
        }
        return { ms: median(times), frame };
      };
      const w = await measure(viaWorker);
      worker.terminate();
      const n = await measure(viaNative);
      fetcher.terminate();
      return { w, n };
    },
    { s, reps, url: `/assets/${workerName}`, nativeUrl: `/assets/${nativeWorkerName}` },
  );
  const f = (x: number): string => x.toFixed(1);
  console.log(
    `| ${s.label} | ${f(result.w.ms)} | ${f(result.n.ms)} | ${f(result.w.frame)} / ${f(result.n.frame)} |`,
  );
}
await browser.close();
