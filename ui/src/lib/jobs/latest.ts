// Latest-only scheduling for jobs whose newer requests make older ones
// pointless, like an effect preview while a slider moves
// (docs/specs/performance-background-jobs.md AC-06, AC-08). At most one job
// runs; a request made meanwhile waits and replaces any earlier waiting
// one, and a result is shown only if no request or cancel came after its
// own. Rune-free, so a Node-side Playwright spec can drive it.

/** A latest-only job queue. */
export interface LatestOnly<P> {
  /** Asks for `params`; supersedes every earlier request. */
  request(params: P): void;
  /** Drops the waiting request and the running one's result. */
  cancel(): void;
}

/**
 * Runs `run` for the newest request only. `show` gets a result whose
 * request is still the newest; `fail` gets such a request's error.
 */
export function latestOnly<P, R>(
  run: (params: P) => Promise<R>,
  show: (result: R, params: P) => void,
  fail: (err: unknown) => void,
): LatestOnly<P> {
  let running = false;
  let waiting: { readonly params: P } | null = null;
  // Bumped by every request and cancel: a result counts only while its
  // own generation is still the current one.
  let generation = 0;

  const start = (params: P): void => {
    running = true;
    const mine = generation;
    let job: Promise<R>;
    try {
      job = run(params);
    } catch (err) {
      job = Promise.reject(err);
    }
    job
      .then(
        (result) => {
          if (mine === generation) show(result, params);
        },
        (err: unknown) => {
          if (mine === generation) fail(err);
        },
      )
      .finally(() => {
        running = false;
        const next = waiting;
        waiting = null;
        if (next) start(next.params);
      });
  };

  return {
    request(params) {
      generation += 1;
      if (running) waiting = { params };
      else start(params);
    },
    cancel() {
      generation += 1;
      waiting = null;
    },
  };
}
