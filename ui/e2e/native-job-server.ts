import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import type { AddressInfo } from 'node:net';

// A stand-in for the desktop app's loopback job endpoint
// (`src-tauri/src/job_server.rs`) for specs that drive the desktop paths
// against the mocked Tauri bridge: it records the jobs it gets and answers
// with fixed bytes or fails (docs/specs/performance-desktop-jobs.md).

export interface NativeJobServer {
  readonly url: string;
  readonly token: string;
  /** The jobs it got (their `pincel-job` header), oldest first. */
  readonly jobs: Record<string, unknown>[];
  /** `answer`: 200 with `answer(job)`; `fail`: 500. */
  mode: 'answer' | 'fail';
  answer: (job: Record<string, unknown>) => Buffer;
  close(): Promise<void>;
}

/** Starts a job endpoint on 127.0.0.1 with a random port. */
export async function startNativeJobServer(): Promise<NativeJobServer> {
  const token = 'e2e-token';
  const cors = (req: IncomingMessage): Record<string, string> => ({
    'access-control-allow-origin': req.headers.origin ?? '*',
    'access-control-allow-methods': 'POST',
    'access-control-allow-headers': 'authorization, content-type, pincel-job',
    'access-control-allow-private-network': 'true',
  });
  const state = {
    jobs: [] as Record<string, unknown>[],
    mode: 'answer' as 'answer' | 'fail',
    answer: (() => Buffer.alloc(0)) as (job: Record<string, unknown>) => Buffer,
  };
  const server = createServer((req: IncomingMessage, res: ServerResponse) => {
    if (req.method === 'OPTIONS') {
      res.writeHead(204, cors(req)).end();
      return;
    }
    const chunks: Buffer[] = [];
    req.on('data', (c: Buffer) => chunks.push(c));
    req.on('end', () => {
      if (req.headers.authorization !== `Bearer ${token}`) {
        res.writeHead(401, cors(req)).end('wrong token');
        return;
      }
      const job = JSON.parse(String(req.headers['pincel-job'] ?? '{}')) as Record<string, unknown>;
      state.jobs.push(job);
      if (state.mode === 'fail') {
        res.writeHead(500, cors(req)).end('the native job failed');
        return;
      }
      res
        .writeHead(200, { ...cors(req), 'content-type': 'application/octet-stream' })
        .end(state.answer(job));
    });
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const { port } = server.address() as AddressInfo;
  return {
    url: `http://127.0.0.1:${port}/job`,
    token,
    get jobs() {
      return state.jobs;
    },
    get mode() {
      return state.mode;
    },
    set mode(m) {
      state.mode = m;
    },
    get answer() {
      return state.answer;
    },
    set answer(a) {
      state.answer = a;
    },
    close: () => new Promise<void>((resolve) => server.close(() => resolve())),
  };
}
