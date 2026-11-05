// The Image-mode job worker: its own wasm instance encodes autosaves and
// exports from document snapshots (docs/specs/performance-background-jobs.md).

import { serveJobs } from '../../../lib/jobs/runner';
import { initCore } from '../core/wasm';
import { encodeSnapshot, type EncodeJob } from './encode';

serveJobs<EncodeJob, Uint8Array>(self as unknown as Parameters<typeof serveJobs>[0], async (job) => {
  await initCore();
  const bytes = encodeSnapshot(job);
  return { res: bytes, transfer: [bytes.buffer] };
});
