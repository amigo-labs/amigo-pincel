// The Image job requests of `protocol.ts`, run by the desktop app's native
// job backend (`src-tauri/src/native_jobs.rs`, served by `job_server.rs`;
// docs/specs/performance-desktop-jobs.md): the page asks the desktop shell
// once where the endpoint is, and the job worker `fetch`es it
// (`native-fetch.ts`), so the snapshot and the result never pass the page's
// main thread.

import { invoke } from '@tauri-apps/api/core';
import type { JobRunner } from '../../../lib/jobs/runner';
import type { JobEndpoint } from './native-fetch';
import type { ImageJob } from './protocol';

let endpoint: Promise<JobEndpoint> | null = null;

/** The native job endpoint; asked again after a failure. */
function jobEndpoint(): Promise<JobEndpoint> {
  endpoint ??= invoke<JobEndpoint>('job_endpoint').catch((err: unknown) => {
    endpoint = null;
    throw err;
  });
  return endpoint;
}

/**
 * Runs Image jobs natively: the endpoint from the desktop shell, the
 * request from `worker`'s job worker. Rejects when the endpoint is missing
 * or the request fails, so `nativeFirst` runs the job in the worker instead
 * (AC-06).
 */
export function nativeVia(
  worker: JobRunner<ImageJob, Uint8Array>,
): (job: ImageJob) => Promise<Uint8Array> {
  return async (job) => {
    const at = await jobEndpoint();
    return worker.run({ kind: 'native', endpoint: at, job }, () => {
      throw new Error('the native job backend could not be reached');
    });
  };
}
