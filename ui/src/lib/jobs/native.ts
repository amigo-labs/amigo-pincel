// The desktop app's native job backend (docs/specs/performance-desktop-jobs.md):
// heavy one-shot jobs at or above a crossover size run in the Tauri process
// on all cores; smaller jobs, and every job in the PWA, run in the Web Worker
// (AC-03). A native job that fails, or a native command that is missing,
// falls back to the Web Worker silently and is logged (AC-06). Rune-free, so
// a Node-side Playwright spec can drive it.

import type { JobRunner, JobSetup } from './runner';

/**
 * The smallest document, in canvas pixels (width × height), whose jobs run
 * natively; `null` runs every job in the worker. Measured on the reference
 * machine, no size pays off yet: Tauri's IPC moves a job's bytes at about
 * 18 MB/s on WebView2 and blocks the page meanwhile, so the native backend
 * is slower than the worker at every size from 256² to 4K (see the spec's
 * Decisions and open question).
 */
export const NATIVE_JOB_MIN_PIXELS: number | null = null;

/** Where a job on a `width`×`height` document runs: natively in the
 *  desktop app from `minPixels` up, in the worker otherwise. */
export function jobBackend(
  desktop: boolean,
  width: number,
  height: number,
  minPixels: number | null = NATIVE_JOB_MIN_PIXELS,
): 'native' | 'worker' {
  return desktop && minPixels !== null && width * height >= minPixels ? 'native' : 'worker';
}

/**
 * A runner that calls `native` first and, if it fails, runs the job on
 * `worker` (with that runner's own retry and main-thread fallback); `label`
 * names it in logs. A setup is loaded natively once, ahead of the first
 * job that needs it, and again after a failure.
 */
export function nativeFirst<Req, Res>(
  native: (req: Req) => Promise<Res>,
  worker: JobRunner<Req, Res>,
  label: string,
): JobRunner<Req, Res> {
  // Setups the native backend has loaded or is loading.
  const loaded = new Map<string, Promise<Res>>();

  const attempt = async (req: Req, setup: JobSetup<Req> | undefined): Promise<Res> => {
    if (setup) {
      let load = loaded.get(setup.key);
      if (!load) {
        load = native(setup.req());
        loaded.set(setup.key, load);
      }
      await load;
    }
    return native(req);
  };

  return {
    async run(req, fallback, setup) {
      try {
        return await attempt(req, setup);
      } catch (err) {
        if (setup) loaded.delete(setup.key);
        console.error(`${label}: native job failed, running it in the worker`, err);
        return worker.run(req, fallback, setup);
      }
    },
    release(key, req) {
      if (loaded.delete(key)) native(req).catch(() => undefined);
      worker.release(key, req);
    },
  };
}
