// The Image job requests of `protocol.ts`, run by the desktop app's native
// job backend (`src-tauri/src/native_jobs.rs`,
// docs/specs/performance-desktop-jobs.md): the job without its snapshot goes
// as JSON in a request header, the snapshot as the raw body, and the result
// comes back as raw bytes.

import { invoke } from '@tauri-apps/api/core';
import { jpegQuality } from './encode';
import type { ImageJob } from './protocol';

/** The request header the native command reads the job from. */
const JOB_HEADER = 'pincel-job';

/** Runs `job` in the Tauri process. Rejects if the command fails or is
 *  missing, or answers anything but bytes. */
export async function runNative(job: ImageJob): Promise<Uint8Array> {
  let body: Uint8Array = new Uint8Array(0);
  let header: object = job;
  if (job.kind === 'encode') {
    const { snapshot, ...rest } = job;
    body = snapshot;
    // The quality the wasm encoder would get (`encodeHandle`).
    header = { ...rest, quality: jpegQuality(rest.quality) };
  } else if (job.kind === 'load') {
    const { snapshot, ...rest } = job;
    body = snapshot;
    header = rest;
  }
  const res = await invoke<unknown>('image_job', body, {
    headers: { [JOB_HEADER]: JSON.stringify(header) },
  });
  if (!(res instanceof ArrayBuffer)) throw new Error('the native job answered no bytes');
  return new Uint8Array(res);
}
