// The Image-mode job worker: its own wasm instance encodes autosaves and
// exports from document snapshots and runs effects on a document copy
// (docs/specs/performance-background-jobs.md). It instantiates the core the
// page has compiled (the `JobInit` message); fetching and compiling the
// 12 MB core again made the first job wait one to two seconds.

import { serveJobs } from '../../../lib/jobs/runner';
import { initCore } from '../core/wasm';
import { runImageJob, type ImageJob } from './protocol';

type Scope = Parameters<typeof serveJobs<ImageJob, Uint8Array, WebAssembly.Module | null>>[0];

const docs = new Map<string, number>();

serveJobs<ImageJob, Uint8Array, WebAssembly.Module | null>(
  self as unknown as Scope,
  async (job) => {
    // Without the page's module (or if instantiating it failed) this
    // fetches and compiles the core itself.
    await initCore();
    const bytes = runImageJob(job, docs);
    return { res: bytes, transfer: bytes.byteLength > 0 ? [bytes.buffer] : [] };
  },
  (module) => initCore(module ?? undefined),
);
