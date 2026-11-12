// The desktop app's native job backend (docs/specs/performance-desktop-jobs.md):
// heavy one-shot jobs at or above a crossover size run in the Tauri process
// on all cores; smaller jobs, and every job in the PWA, run in the Web Worker
// (AC-03). A native job that fails, or a native command that is missing,
// falls back to the Web Worker silently and is logged (AC-06). Native jobs
// travel to the desktop app's loopback endpoint through a small fetch worker
// (`native-worker.ts`), so their bytes never pass the page's main thread.
// Rune-free, so a Node-side Playwright spec can drive it.

import type { JobRunner, JobSetup } from './runner';

/** Smallest documents, in canvas pixels (width × height), whose jobs of a
 *  kind run natively; `null` keeps that kind in the worker. */
export interface NativeMinPixels {
  /** Effect sessions: previews and the full-resolution apply. */
  readonly effect: number | null;
  /** Encodes: save, export, autosave. */
  readonly encode: number | null;
}

/**
 * Measured on the reference machine over the loopback endpoint (the spec's
 * Decisions, 2026-11-12): against the SIMD worker no size pays off — a
 * Gaussian blur r 10 takes 57 ms in the worker and 109 ms natively at 1024²,
 * 592 vs 606 ms at 3840×2160 — and encodes run on one core either way, so
 * every job stays in the worker until the owner decides (Open questions).
 */
export const NATIVE_JOB_MIN_PIXELS: NativeMinPixels = { effect: null, encode: null };

/** Where a job on a `width`×`height` document runs: natively in the
 *  desktop app from `minPixels` up, in the worker otherwise. */
export function jobBackend(
  desktop: boolean,
  width: number,
  height: number,
  minPixels: number | null,
): 'native' | 'worker' {
  return desktop && minPixels !== null && width * height >= minPixels ? 'native' : 'worker';
}

/** Where the native backend takes jobs (`JobEndpoint` in job_server.rs). */
export interface JobEndpoint {
  readonly url: string;
  readonly token: string;
}

/** What the page sends the native fetch worker (`native-worker.ts`). */
export interface NativeFetch extends JobEndpoint {
  readonly id: number;
  /** The job as JSON, for the `pincel-job` header. */
  readonly job: string;
  /** The request body (a document snapshot, or empty). */
  readonly body: Uint8Array;
}

/** What the native fetch worker answers. */
export type NativeFetched =
  | { readonly id: number; readonly ok: true; readonly res: Uint8Array }
  | { readonly id: number; readonly ok: false; readonly error: string };

/**
 * Sends native jobs through one fetch worker made by `spawn`, started at the
 * first job (so the PWA never starts one). Unlike `jobRunner` it retries
 * nothing: a failure rejects at once, and `nativeFirst` runs the job in the
 * Web Worker instead (AC-06). `move` hands the body's buffer over instead of
 * copying it.
 */
export function nativeFetcher(
  spawn: () => Worker,
): (endpoint: JobEndpoint, job: string, body: Uint8Array, move?: boolean) => Promise<Uint8Array> {
  let worker: Worker | null = null;
  let nextId = 1;
  const pending = new Map<number, { resolve(res: Uint8Array): void; reject(err: Error): void }>();
  const fail = (reason: string): void => {
    worker?.terminate();
    worker = null;
    const owed = [...pending.values()];
    pending.clear();
    for (const p of owed) p.reject(new Error(reason));
  };
  const ensure = (): Worker => {
    if (worker) return worker;
    const w = spawn();
    w.onmessage = (e: MessageEvent<NativeFetched>) => {
      const p = pending.get(e.data.id);
      if (!p) return;
      pending.delete(e.data.id);
      if (e.data.ok) p.resolve(e.data.res);
      else p.reject(new Error(e.data.error));
    };
    w.onerror = (e: ErrorEvent) => {
      e.preventDefault();
      fail(e.message || 'the native fetch worker failed');
    };
    worker = w;
    return w;
  };
  return (endpoint, job, body, move = false) =>
    new Promise<Uint8Array>((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      const msg: NativeFetch = { id, url: endpoint.url, token: endpoint.token, job, body };
      try {
        ensure().postMessage(msg, move ? [body.buffer] : []);
      } catch (err) {
        pending.delete(id);
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
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
