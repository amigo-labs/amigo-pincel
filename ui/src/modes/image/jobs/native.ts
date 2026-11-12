// The Image job requests of `protocol.ts`, run by the desktop app's native
// job backend (`src-tauri/src/native_jobs.rs`, served on a loopback endpoint
// by `job_server.rs`; docs/specs/performance-desktop-jobs.md): the page asks
// the desktop shell once where the endpoint is (`job_endpoint`), and the
// native fetch worker posts each job there — the job without its snapshot as
// JSON in a header, the snapshot as the body — and answers the result's
// bytes, so they never pass the page's main thread.

import { invoke } from '@tauri-apps/api/core';
import { nativeFetcher, type JobEndpoint } from '../../../lib/jobs/native';
import { jpegQuality } from './encode';
import type { ImageJob } from './protocol';

const send = nativeFetcher(
  () =>
    new Worker(new URL('../../../lib/jobs/native-worker.ts', import.meta.url), {
      type: 'module',
    }),
);

let endpoint: Promise<JobEndpoint> | null = null;

/** The native job endpoint; asked again after a failure. */
function jobEndpoint(): Promise<JobEndpoint> {
  endpoint ??= invoke<JobEndpoint>('job_endpoint').catch((err: unknown) => {
    endpoint = null;
    throw err;
  });
  return endpoint;
}

/** Runs `job` in the native backend. Rejects if the endpoint is missing or
 *  the request fails, so `nativeFirst` runs it in the worker (AC-06). */
export async function runNative(job: ImageJob): Promise<Uint8Array> {
  const at = await jobEndpoint();
  if (job.kind === 'encode') {
    const { snapshot, ...rest } = job;
    // The quality the wasm encoder would get (`encodeHandle`). The snapshot
    // is copied: the worker fallback may still need it.
    return send(at, JSON.stringify({ ...rest, quality: jpegQuality(rest.quality) }), snapshot);
  }
  if (job.kind === 'load') {
    const { snapshot, ...rest } = job;
    // An effect session's copy moves: `JobSetup.req` makes a fresh one for
    // every attempt, the worker fallback included.
    return send(at, JSON.stringify(rest), snapshot, true);
  }
  return send(at, JSON.stringify(job), new Uint8Array(0));
}
