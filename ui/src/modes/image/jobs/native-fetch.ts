// The request the Image job worker sends the desktop app's native job
// endpoint (`src-tauri/src/job_server.rs`, docs/specs/performance-desktop-jobs.md,
// Decision Log 2026-11-11): `POST` with the job — without its snapshot — as
// JSON in the `pincel-job` header, the snapshot as the body and the bearer
// token; the answer is the result's bytes. The worker sends it, so a job's
// bytes never cross the page's main thread. No Tauri import: the worker
// loads this module.

import { jpegQuality } from './encode';
import type { ImageJob } from './protocol';

/** Where the native backend takes jobs (`JobEndpoint` in job_server.rs). */
export interface JobEndpoint {
  readonly url: string;
  readonly token: string;
}

/** The header the endpoint reads the job from. */
const JOB_HEADER = 'pincel-job';

/** `job` split into the endpoint's header JSON and body bytes. */
export function nativeRequest(job: ImageJob): { header: string; body: Uint8Array } {
  if (job.kind === 'encode') {
    const { snapshot, kind, ...rest } = job;
    // The quality the wasm encoder would get (`encodeHandle`).
    return {
      header: JSON.stringify({ kind, ...rest, quality: jpegQuality(rest.quality) }),
      body: snapshot,
    };
  }
  if (job.kind === 'load') {
    const { snapshot, ...rest } = job;
    return { header: JSON.stringify(rest), body: snapshot };
  }
  if (job.kind === 'native') throw new Error('a native job cannot wrap another');
  return { header: JSON.stringify(job), body: new Uint8Array(0) };
}

/** Runs `job` at `endpoint`. Rejects on a network error or any answer but
 *  200 with bytes. */
export async function fetchNative(endpoint: JobEndpoint, job: ImageJob): Promise<Uint8Array> {
  const { header, body } = nativeRequest(job);
  const res = await fetch(endpoint.url, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${endpoint.token}`,
      [JOB_HEADER]: header,
      'Content-Type': 'application/octet-stream',
    },
    body: body as Uint8Array<ArrayBuffer>,
  });
  if (!res.ok) throw new Error(`native job: ${res.status} ${await res.text()}`);
  return new Uint8Array(await res.arrayBuffer());
}
