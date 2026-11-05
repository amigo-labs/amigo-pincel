// Heavy one-shot jobs (encoding, effects) run in a dedicated Web Worker with
// its own wasm instance (docs/specs/performance-background-jobs.md). Plain
// typed `postMessage` requests and responses with a job id; results come
// back as transferable buffers. A job the worker fails is retried once in a
// fresh worker, then run on the main thread (AC-09).

/** What the page sends a job worker. */
export interface JobRequest<Req> {
  readonly id: number;
  readonly req: Req;
}

/** What the page sends a job worker once, before its first job: what the
 *  worker needs to set itself up (e.g. the page's compiled wasm module). */
export interface JobInit<Init> {
  readonly init: Init;
}

/** What a job worker answers. */
export type JobResponse<Res> =
  | { readonly id: number; readonly ok: true; readonly res: Res }
  | { readonly id: number; readonly ok: false; readonly error: string };

/** Runs jobs in a worker, with retry and main-thread fallback. */
export interface JobRunner<Req, Res> {
  /**
   * Runs `req` in the worker. If the worker fails, it is restarted and the
   * job tried once more; if that fails too, `fallback` runs on the main
   * thread and its result (or error) is the job's.
   */
  run(req: Req, fallback: () => Res | Promise<Res>): Promise<Res>;
}

interface Pending<Res> {
  resolve(res: Res): void;
  reject(err: Error): void;
}

/**
 * A runner over workers made by `spawn`; `label` names it in logs. `init`,
 * if given, is sent to every worker it starts, ahead of the first job.
 */
export function jobRunner<Req, Res, Init = never>(
  spawn: () => Worker,
  label: string,
  init?: () => Init,
): JobRunner<Req, Res> {
  let worker: Worker | null = null;
  let nextId = 1;
  const pending = new Map<number, Pending<Res>>();

  /** Drop the worker and fail what it still owed. */
  const fail = (reason: string): void => {
    worker?.terminate();
    worker = null;
    const owed = [...pending.values()];
    pending.clear();
    for (const p of owed) p.reject(new Error(reason));
  };

  const ensure = (): Worker => {
    if (worker) return worker;
    const w = spawn();
    w.onmessage = (e: MessageEvent<JobResponse<Res>>) => {
      const msg = e.data;
      const p = pending.get(msg.id);
      if (!p) return;
      pending.delete(msg.id);
      if (msg.ok) p.resolve(msg.res);
      else p.reject(new Error(msg.error));
    };
    w.onerror = (e: ErrorEvent) => {
      e.preventDefault();
      fail(e.message || 'the job worker failed');
    };
    w.onmessageerror = () => fail('a job message could not be read');
    if (init) {
      const msg: JobInit<Init> = { init: init() };
      w.postMessage(msg);
    }
    worker = w;
    return w;
  };

  const attempt = (req: Req): Promise<Res> =>
    new Promise<Res>((resolve, reject) => {
      let w: Worker;
      try {
        w = ensure();
      } catch (err) {
        reject(err instanceof Error ? err : new Error(String(err)));
        return;
      }
      const id = nextId++;
      pending.set(id, { resolve, reject });
      const msg: JobRequest<Req> = { id, req };
      w.postMessage(msg);
    });

  return {
    async run(req, fallback) {
      try {
        return await attempt(req);
      } catch (first) {
        // A worker that failed a job may be broken: start a fresh one.
        fail('restarting the job worker');
        try {
          return await attempt(req);
        } catch (second) {
          fail('the job worker failed twice');
          console.error(`${label}: worker job failed twice, running it here`, first, second);
          return fallback();
        }
      }
    },
  };
}

/** Answers job requests in a worker: `handle` computes a job's result and
 *  the buffers to transfer back with it. `setup` takes the page's
 *  `JobInit` message; jobs wait for it to finish (whether it succeeds or
 *  not — `handle` must cope without it). */
export function serveJobs<Req, Res, Init = never>(
  scope: {
    onmessage: ((e: MessageEvent<JobRequest<Req> | JobInit<Init>>) => unknown) | null;
    postMessage(msg: JobResponse<Res>, transfer: Transferable[]): void;
  },
  handle: (req: Req) => Promise<{ res: Res; transfer: Transferable[] }>,
  setup?: (init: Init) => Promise<void>,
): void {
  let ready: Promise<void> = Promise.resolve();
  scope.onmessage = async (e) => {
    const msg = e.data;
    if ('init' in msg) {
      if (setup) ready = setup(msg.init).catch(() => undefined);
      return;
    }
    const { id, req } = msg;
    try {
      await ready;
      const { res, transfer } = await handle(req);
      scope.postMessage({ id, ok: true, res }, transfer);
    } catch (err) {
      scope.postMessage({ id, ok: false, error: err instanceof Error ? err.message : String(err) }, []);
    }
  };
}
