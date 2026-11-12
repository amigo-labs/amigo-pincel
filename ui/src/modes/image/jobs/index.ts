// Image-mode heavy jobs off the main thread
// (docs/specs/performance-background-jobs.md): the page takes a snapshot of
// the document synchronously — so later edits are not part of the file —
// and the job worker encodes it; effect previews and applies run on a copy
// of the document the worker keeps for one effect session. In the desktop
// app, jobs on large documents run natively first
// (docs/specs/performance-desktop-jobs.md).

import { latestOnly } from '../../../lib/jobs/latest';
import { jobBackend, NATIVE_JOB_MIN_PIXELS, nativeFirst } from '../../../lib/jobs/native';
import { jobRunner, type JobRunner, type JobSetup } from '../../../lib/jobs/runner';
import { isTauri } from '../../../lib/platform';
import { compiledCore, core } from '../core/wasm';
import { encodeSnapshot, type EncodeFormat, type EncodeJob } from './encode';
import { runNative } from './native';
import { previewBytes, regionBytes, type EffectJob, type ImageJob } from './protocol';

export type { EncodeFormat, EffectJob };

// Each worker gets the page's compiled core, so it only instantiates it.
const jobs = jobRunner<ImageJob, Uint8Array, WebAssembly.Module | null>(
  () => new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' }),
  'image jobs',
  compiledCore,
);

// Native first, the worker as its fallback (AC-06); the desktop app only.
const nativeJobs = nativeFirst(runNative, jobs, 'image jobs');

/** The runner for `kind` jobs on document `handle`: native in the desktop
 *  app from that kind's crossover size up, the worker otherwise (AC-03). */
function runnerFor(handle: number, kind: 'effect' | 'encode'): JobRunner<ImageJob, Uint8Array> {
  const { width, height } = core.documentInfo(handle);
  const min = NATIVE_JOB_MIN_PIXELS[kind];
  return jobBackend(isTauri(), width, height, min) === 'native' ? nativeJobs : jobs;
}

/** Encodes document `handle` as it is now, in the job worker. */
export function encodeInBackground(
  handle: number,
  format: EncodeFormat,
  options: { level?: number; quality?: number } = {},
): Promise<Uint8Array> {
  const job: EncodeJob = {
    snapshot: core.snapshotDocument(handle),
    format,
    level: options.level ?? 6,
    quality: options.quality ?? 90,
  };
  return runnerFor(handle, 'encode').run({ kind: 'encode', ...job }, () => encodeSnapshot(job));
}

/** Effect jobs of one effect session on one document. */
export interface EffectJobs {
  /** The composite with `effect` previewed (canvas-sized RGBA8). */
  preview(effect: EffectJob): Promise<Uint8ClampedArray<ArrayBuffer>>;
  /** The region `effect` changes, for `core.applyEffectRegion`. */
  region(effect: EffectJob): Promise<Uint8Array>;
  /** Frees the worker's copy of the document. */
  close(): void;
}

let sessions = 0;

/**
 * Effect jobs on document `handle`, which must not change while they run
 * (the effect panel keeps the editor inert): the worker copies the document
 * once, at the first job, and works on that copy.
 */
export function effectJobs(handle: number): EffectJobs {
  sessions += 1;
  const key = `fx-${sessions}`;
  const runner = runnerFor(handle, 'effect');
  const setup: JobSetup<ImageJob> = {
    key,
    req: () => ({ kind: 'load', key, snapshot: core.snapshotDocument(handle) }),
  };
  return {
    async preview(effect) {
      const bytes = await runner.run(
        { kind: 'preview', key, effect },
        () => previewBytes(handle, effect),
        setup,
      );
      return new Uint8ClampedArray(bytes.buffer as ArrayBuffer, bytes.byteOffset, bytes.byteLength);
    },
    region: (effect) =>
      runner.run({ kind: 'region', key, effect }, () => regionBytes(handle, effect), setup),
    close: () => runner.release(key, { kind: 'drop', key }),
  };
}

/** One open effect panel's jobs: previews where only the newest result is
 *  shown (AC-06), and the apply. */
export interface EffectRun {
  /** Asks for a preview of `effect`; supersedes earlier ones. */
  preview(effect: EffectJob): void;
  /** Drops the running and waiting previews (Preview off, Cancel). */
  stopPreview(): void;
  /** The region `effect` changes; drops the previews first. */
  region(effect: EffectJob): Promise<Uint8Array>;
  /** Ends the session: drops previews and the worker's document copy. */
  close(): void;
}

/** Starts the effect jobs of one panel session on document `handle`;
 *  `show` gets the newest preview, `fail` the newest preview's error. */
export function effectRun(
  handle: number,
  show: (composite: Uint8ClampedArray<ArrayBuffer>) => void,
  fail: (err: unknown) => void,
): EffectRun {
  const fx = effectJobs(handle);
  const previews = latestOnly<EffectJob, Uint8ClampedArray<ArrayBuffer>>(
    (effect) => fx.preview(effect),
    show,
    fail,
  );
  return {
    preview: (effect) => previews.request(effect),
    stopPreview: () => previews.cancel(),
    region(effect) {
      previews.cancel();
      return fx.region(effect);
    },
    close() {
      previews.cancel();
      fx.close();
    },
  };
}
