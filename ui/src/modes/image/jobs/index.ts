// Image-mode heavy jobs off the main thread
// (docs/specs/performance-background-jobs.md): the page takes a snapshot of
// the document synchronously — so later edits are not part of the file —
// and the job worker encodes it.

import { jobRunner } from '../../../lib/jobs/runner';
import { compiledCore, core } from '../core/wasm';
import { encodeSnapshot, type EncodeFormat, type EncodeJob } from './encode';

export type { EncodeFormat };

// Each worker gets the page's compiled core, so it only instantiates it.
const encoder = jobRunner<EncodeJob, Uint8Array, WebAssembly.Module | null>(
  () => new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' }),
  'image encode',
  compiledCore,
);

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
  return encoder.run(job, () => encodeSnapshot(job));
}
