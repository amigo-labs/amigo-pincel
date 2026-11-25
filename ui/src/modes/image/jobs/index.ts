// Image-mode heavy jobs off the main thread
// (docs/specs/performance-background-jobs.md): the page takes a snapshot of
// the document synchronously — so later edits are not part of the file —
// and the job worker encodes it; effect previews and applies run on a copy
// of the document the worker keeps for one effect session.

import { latestOnly } from '../../../lib/jobs/latest';
import { jobRunner, type JobSetup } from '../../../lib/jobs/runner';
import { compiledCore, core } from '../core/wasm';
import { encodeHandle, type EncodeFormat, type EncodeJob } from './encode';
import { jobTransfer, previewBytes, regionBytes, type EffectJob, type ImageJob } from './protocol';
import { readParts } from './snapshot';

export type { EncodeFormat, EffectJob };

// Each worker gets the page's compiled core, so it only instantiates it.
const jobs = jobRunner<ImageJob, Uint8Array, WebAssembly.Module | null>(
  () => new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' }),
  'image jobs',
  compiledCore,
  jobTransfer,
);

/** Encodes document `handle` as it is now, in the job worker. */
export async function encodeInBackground(
  handle: number,
  format: EncodeFormat,
  options: { level?: number; quality?: number } = {},
): Promise<Uint8Array> {
  const level = options.level ?? 6;
  const quality = options.quality ?? 90;
  // Frozen now, copied out over the next tasks (AC-03, AC-04); each attempt
  // copies it out again, since the worker takes the buffers.
  const frozen = core.freezeDocument(handle);
  try {
    return await jobs.run(
      async (): Promise<ImageJob> => {
        const job: EncodeJob = { snapshot: await readParts(frozen), format, level, quality };
        return { kind: 'encode', ...job };
      },
      () => encodeHandle(frozen, format, level, quality),
    );
  } finally {
    core.closeDocument(frozen);
  }
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
  const setup: JobSetup<ImageJob> = {
    key,
    req: async () => {
      const frozen = core.freezeDocument(handle);
      try {
        return { kind: 'load', key, snapshot: await readParts(frozen) };
      } finally {
        core.closeDocument(frozen);
      }
    },
  };
  return {
    async preview(effect) {
      const bytes = await jobs.run(
        { kind: 'preview', key, effect },
        () => previewBytes(handle, effect),
        setup,
      );
      return new Uint8ClampedArray(bytes.buffer as ArrayBuffer, bytes.byteOffset, bytes.byteLength);
    },
    region: (effect) =>
      jobs.run({ kind: 'region', key, effect }, () => regionBytes(handle, effect), setup),
    close: () => jobs.release(key, { kind: 'drop', key }),
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
