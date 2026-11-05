// The Image-mode job worker: its own wasm instance encodes autosaves and
// exports from document snapshots (docs/specs/performance-background-jobs.md).
// It instantiates the core the page has compiled (the `JobInit` message);
// fetching and compiling the 12 MB core again made the first job wait one
// to two seconds.

import { serveJobs } from '../../../lib/jobs/runner';
import { initCore } from '../core/wasm';
import { encodeSnapshot, type EncodeJob } from './encode';

serveJobs<EncodeJob, Uint8Array, WebAssembly.Module | null>(
  self as unknown as Parameters<typeof serveJobs<EncodeJob, Uint8Array, WebAssembly.Module | null>>[0],
  async (job) => {
    // Without the page's module (or if instantiating it failed) this
    // fetches and compiles the core itself.
    await initCore();
    const bytes = encodeSnapshot(job);
    return { res: bytes, transfer: [bytes.buffer] };
  },
  (module) => initCore(module ?? undefined),
);
