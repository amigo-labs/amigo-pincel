// Which tab or agent call a generation job belongs to
// (docs/specs/comfyui-generate.md, comfyui-generate-pixel.md). Plain,
// non-reactive maps: events look them up, nothing renders from them. Tabs
// are named by their generation key (`image:3`, `pixel:1`), so jobs of
// both editors share one routing.
import type { GenEvent } from './generate-bridge';
import type { GenTarget } from './generate-params';

/** The tab key of each panel job. */
export const jobTabs = new Map<number, string>();

/** The target each tab generated for last, by tab key. */
export const targets = new Map<string, GenTarget>();

/** The handler of each agent job (`gen.image`). */
export const agentJobs = new Map<number, (event: GenEvent) => void>();

/** The tab key of each agent job, so closing the tab cancels it. */
export const agentTabs = new Map<number, string>();

/** Events of jobs not bound yet: the backend may report before the start
 *  call has returned the job id. */
const early: GenEvent[] = [];

/** Keeps an event of a job nobody claimed yet. */
export function holdEarly(event: GenEvent): void {
  early.push(event);
  if (early.length > 200) early.shift();
}

/** Takes the held events of `job`, in arrival order. */
export function takeEarly(job: number): GenEvent[] {
  const mine = early.filter((e) => e.job === job);
  if (mine.length > 0) early.splice(0, early.length, ...early.filter((e) => e.job !== job));
  return mine;
}
