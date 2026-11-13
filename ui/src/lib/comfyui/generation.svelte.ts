// Image generation in both editors (docs/specs/comfyui-generate.md,
// comfyui-generate-pixel.md): the Generate panel's settings, the server it
// generates on, and each tab's generation — its job, progress and
// variants, which stay with the tab (AC-14). The job runs in the desktop
// app's backend; what it reads from the document and how a variant lands
// in it comes from the tab's `GenTarget` (Image or Pixel mode), fixed when
// the user clicks Generate (AC-08).

import { getPref, setPref } from '../idb/prefs';
import { strokeDone } from '../agent/busy';
import type { GeneratedLayer } from '../shell/types';
import { comfyConnect, onComfyStatus, type ComfyStatus } from './bridge';
import {
  cancelGeneration,
  fetchVariant,
  generationModels,
  onGenerationEvent,
  releaseGeneration,
  startGeneration,
  toBase64,
  type GenEvent,
} from './generate-bridge';
import {
  BLOCK_ENGLISH,
  GEN_PREF,
  asGenError,
  familyOf,
  layerName,
  nativePixels,
  statusIssue,
  type Box,
  type Family,
  type GenError,
  type GenImageArgs,
  type GenTarget,
  type Models,
} from './generate-params';
import { agentJobs, agentTabs, holdEarly, jobTabs, takeEarly, targets } from './job-routing';

/** The panel settings that persist across restarts (AC-03). */
export interface GenSettings {
  model: string | null;
  lora: string | null;
  loraStrength: number;
  /** 1–100 %. */
  strength: number;
  count: number;
  negative: string;
  /** The model type the user picked per checkpoint outside the catalog. */
  families: Record<string, Family>;
}

export const genSettings: GenSettings = $state({
  model: null,
  lora: null,
  loraStrength: 1,
  strength: 100,
  count: 4,
  negative: '',
  families: {},
});

/** The server the panel generates on (shared by every tab). */
export const genServer = $state<{
  status: ComfyStatus;
  models: Models | null;
  checking: boolean;
  /** A stored model or LoRA the server no longer has (AC-03). */
  gone: string | null;
}>({ status: { kind: 'idle' }, models: null, checking: false, gone: null });

/** What the panel's text fields hold (not persisted). */
export const genInput = $state({ prompt: '', seed: '' });

export type GenPhase = 'idle' | 'starting' | 'queued' | 'running' | 'fetching' | 'done';

/** One tab's generation. */
export interface TabGen {
  phase: GenPhase;
  job: number | null;
  ahead: number;
  step: number;
  total: number;
  /** The live preview as a data URL. */
  live: string | null;
  /** Fixed when Generate was clicked (AC-08). */
  box: Box;
  /** The mask inside `box`, one byte per pixel; empty without one. */
  mask: Uint8Array;
  prompt: string;
  /** The generation size. */
  width: number;
  height: number;
  /** The finished variants at generation size, encoded. */
  variants: Uint8Array[];
  /** Object URLs of the strip's images. */
  thumbs: string[];
  selected: number;
  error: GenError | null;
}

/** Each tab's generation, by `GenTarget.key`. */
export const genTabs: Record<string, TabGen> = $state({});

/** Delivers the early events of `job`, now that it is bound. */
function replay(job: number): void {
  for (const event of takeEarly(job)) onEvent(event);
}

let listening: Promise<unknown> | null = null;
let loaded: Promise<void> | null = null;

/** Reads the stored settings, once. */
export function loadGenSettings(): Promise<void> {
  loaded ??= (async () => {
    try {
      const stored = (await getPref(GEN_PREF)) as Partial<GenSettings> | undefined;
      if (stored && typeof stored === 'object') Object.assign(genSettings, stored);
    } catch {
      // No IndexedDB: the defaults stay.
    }
  })();
  return loaded;
}

/** Stores the settings (AC-03). */
export function saveGenSettings(): void {
  void setPref(GEN_PREF, $state.snapshot(genSettings)).catch(() => undefined);
}

/** A server is needed (the panel opened): resolve it and its models
 *  (comfyui-connect AC-03), and drop stored names it no longer has. */
export async function refreshServer(): Promise<void> {
  ensureListening();
  await loadGenSettings();
  genServer.checking = true;
  try {
    genServer.status = await comfyConnect();
    await readModels();
  } catch (err) {
    genServer.status = { kind: 'failed', message: String(err) };
    genServer.models = null;
  } finally {
    genServer.checking = false;
  }
}

async function readModels(): Promise<void> {
  if (statusIssue(genServer.status) !== null) {
    genServer.models = null;
    return;
  }
  const models = await generationModels();
  genServer.models = models;
  const names = models.checkpoints.map((c) => c.name);
  let gone: string | null = null;
  if (genSettings.model === null || !names.includes(genSettings.model)) {
    if (genSettings.model !== null) gone = genSettings.model;
    genSettings.model = names[0] ?? null;
  }
  if (genSettings.lora !== null && !models.loras.includes(genSettings.lora)) {
    gone ??= genSettings.lora;
    genSettings.lora = null;
  }
  genServer.gone = gone;
  if (gone !== null) saveGenSettings();
}

/** The family the chosen model generates with. */
export function currentFamily(): Family {
  const checkpoint = genServer.models?.checkpoints.find((c) => c.name === genSettings.model);
  return familyOf(checkpoint, genSettings.families);
}

/** Whether the checkpoint is a catalog model (no Model type choice). */
export function hasCatalogFamily(): boolean {
  return genServer.models?.checkpoints.find((c) => c.name === genSettings.model)?.family != null;
}

function ensureListening(): void {
  listening ??= Promise.all([
    onGenerationEvent(onEvent),
    onComfyStatus((status) => {
      genServer.status = status;
      if (statusIssue(status) !== null) genServer.models = null;
      else if (genServer.models === null) void readModels().catch(() => undefined);
    }),
  ]).catch((err: unknown) => console.error('comfyui listen failed', err));
}

function starting(taken: Capture, prompt: string): TabGen {
  return {
    phase: 'starting',
    job: null,
    ahead: 0,
    step: 0,
    total: 0,
    live: null,
    box: taken.box,
    mask: taken.mask,
    prompt,
    width: taken.width,
    height: taken.height,
    variants: [],
    thumbs: [],
    selected: 0,
    error: null,
  };
}

/** Drops a tab's variants and forgets its job (AC-13). */
function clear(key: string): void {
  const gen = genTabs[key];
  if (!gen) return;
  for (const url of gen.thumbs) URL.revokeObjectURL(url);
  if (gen.job !== null) {
    jobTabs.delete(gen.job);
    void releaseGeneration(gen.job).catch(() => undefined);
  }
  delete genTabs[key];
}

/** What a generation reads from the document when it starts. */
interface Capture {
  box: Box;
  mask: Uint8Array;
  input: string | null;
  maskPng: string | null;
  width: number;
  height: number;
  /** Strength after "nothing to refine" (edge cases). */
  strength: number;
}

function capture(target: GenTarget, family: Family, strength: number): Capture | null {
  const box = target.box();
  if (box === null) return null;
  const [width, height] = target.size(nativePixels(family), box);
  const mask = target.mask(box);
  let input: Uint8Array | null = null;
  // Inpainting reads the context around the mask; refining reads the input.
  if (mask.length > 0 || strength < 1) {
    const png = target.input(box, width, height);
    // A fully transparent layer has nothing to refine: generate as at 100 %.
    if (png.length > 0) input = png;
  }
  return {
    box,
    mask,
    input: input && toBase64(input),
    maskPng: input && mask.length > 0 ? toBase64(target.maskPng(mask, box, width, height)) : null,
    width,
    height,
    strength: input ? strength : 1,
  };
}

/** Generate for `target` with the panel's settings (AC-05 – AC-07); the
 *  tab's earlier variants are dropped (AC-13). */
export async function generate(target: GenTarget): Promise<void> {
  const model = genSettings.model;
  const prompt = genInput.prompt.trim();
  if (!target.alive() || model === null || prompt === '' || target.blocked() !== null) return;
  const family = currentFamily();
  const seedText = genInput.seed.trim();
  const seed = seedText === '' ? null : Number(seedText);
  if (seed !== null && !(Number.isSafeInteger(seed) && seed >= 0)) return;
  const taken = capture(target, family, genSettings.strength / 100);
  if (taken === null) return;
  const key = target.key;
  clear(key);
  targets.set(key, target);
  genTabs[key] = starting(taken, prompt);
  try {
    const started = await startGeneration({
      model,
      family,
      lora: genSettings.lora,
      loraStrength: genSettings.loraStrength,
      prompt,
      negative: family === 'flux' ? '' : genSettings.negative,
      seed,
      count: genSettings.count,
      strength: taken.strength,
      width: taken.width,
      height: taken.height,
      input: taken.input,
      mask: taken.maskPng,
    });
    const current = genTabs[key];
    if (current === undefined || current.phase !== 'starting') {
      // Dropped while starting (Discard, tab closed).
      void releaseGeneration(started.job).catch(() => undefined);
      return;
    }
    current.job = started.job;
    current.phase = 'queued';
    jobTabs.set(started.job, key);
    replay(started.job);
  } catch (err) {
    const current = genTabs[key];
    if (current) {
      current.phase = 'idle';
      current.error = asGenError(err);
    }
  }
}

async function finish(key: string, job: number, count: number, error: GenError | null): Promise<void> {
  const gen = genTabs[key];
  if (!gen || gen.job !== job) return;
  gen.phase = 'fetching';
  const variants: Uint8Array[] = [];
  const thumbs: Uint8Array[] = [];
  const target = targets.get(key);
  try {
    for (let i = 0; i < count; i++) {
      const variant = await fetchVariant(job, i);
      if (target) thumbs.push(target.thumbnail(variant, gen.box, gen.mask));
      variants.push(variant);
    }
  } catch (err) {
    error ??= asGenError(err);
  }
  jobTabs.delete(job);
  void releaseGeneration(job).catch(() => undefined);
  const now = genTabs[key];
  if (!now || now.job !== job) return;
  now.job = null;
  now.variants = variants.slice(0, thumbs.length);
  now.thumbs = thumbs.map((v) => URL.createObjectURL(new Blob([v as BlobPart], { type: 'image/png' })));
  now.selected = 0;
  now.error = error;
  now.live = null;
  now.phase = now.variants.length > 0 ? 'done' : 'idle';
}

function onEvent(event: GenEvent): void {
  const agent = agentJobs.get(event.job);
  if (agent) {
    agent(event);
    return;
  }
  const key = jobTabs.get(event.job);
  if (key === undefined) {
    holdEarly(event);
    return;
  }
  const gen = genTabs[key];
  if (!gen || gen.job !== event.job) return;
  switch (event.kind) {
    case 'queued':
      gen.phase = 'queued';
      gen.ahead = event.ahead;
      break;
    case 'running':
      gen.phase = 'running';
      break;
    case 'progress':
      gen.phase = 'running';
      gen.step = event.step;
      gen.total = event.total;
      break;
    case 'preview':
      gen.live = event.image;
      break;
    case 'finished':
      void finish(key, event.job, event.count, event.error);
      break;
    case 'failed':
      jobTabs.delete(event.job);
      gen.job = null;
      gen.phase = 'idle';
      gen.live = null;
      gen.error = event.error;
      break;
    case 'cancelled':
      jobTabs.delete(event.job);
      delete genTabs[key];
      break;
  }
}

/** Cancel: off the server's queue, or stopped if it runs (AC-09). */
export function cancel(key: string): void {
  const gen = genTabs[key];
  if (!gen) return;
  if (gen.job === null) {
    // Still starting: the start drops its job when it returns.
    delete genTabs[key];
    return;
  }
  void cancelGeneration(gen.job).catch(() => undefined);
}

/** Shows variant `index` on the canvas instead (AC-11). */
export function selectVariant(key: string, index: number): void {
  const gen = genTabs[key];
  if (gen && index >= 0 && index < gen.variants.length) gen.selected = index;
}

/** Discard: the variants and their preview go (AC-13). */
export function discard(key: string): void {
  clear(key);
}

/** Apply: the selected variant becomes a new layer (AC-12). */
export function apply(target: GenTarget): void {
  const gen = genTabs[target.key];
  const variant = gen?.variants[gen.selected];
  if (!gen || variant === undefined || !target.alive()) return;
  target.apply(variant, gen.box, gen.mask, layerName(gen.prompt));
  clear(target.key);
}

/** The tab closed: its generation is cancelled and its variants go
 *  (AC-21). */
export function dropTabGeneration(key: string): void {
  const gen = genTabs[key];
  if (gen?.job != null) void cancelGeneration(gen.job).catch(() => undefined);
  clear(key);
  targets.delete(key);
  // An agent's generation for this tab ends too, adding nothing (AC-20).
  for (const [job, owner] of agentTabs) {
    if (owner === key) void cancelGeneration(job).catch(() => undefined);
  }
}

/** One variant for an agent call, applied without preview (AC-16); the
 *  panel's own variants stay. `signal` cancels the job (AC-20). */
export async function generateForAgent(
  target: GenTarget,
  args: GenImageArgs,
  models: Models,
  signal: AbortSignal,
): Promise<GeneratedLayer> {
  ensureListening();
  await loadGenSettings();
  if (!target.alive()) throw new Error('no document is open');
  const blocked = target.blocked();
  if (blocked !== null) throw new Error(BLOCK_ENGLISH[blocked]);
  const checkpoint = models.checkpoints.find((c) => c.name === args.model);
  const family = familyOf(checkpoint, genSettings.families);
  const taken = capture(target, family, args.strength);
  if (taken === null) throw new Error('the selection lies outside the canvas');
  const started = await startGeneration({
    model: args.model,
    family,
    lora: args.lora,
    loraStrength: args.loraStrength,
    prompt: args.prompt,
    negative: family === 'flux' ? '' : args.negative,
    seed: args.seed,
    count: 1,
    strength: taken.strength,
    width: taken.width,
    height: taken.height,
    input: taken.input,
    mask: taken.maskPng,
  });
  agentTabs.set(started.job, target.key);
  const ended = new Promise<GenEvent>((resolve) => {
    agentJobs.set(started.job, (event) => {
      if (event.kind === 'finished' || event.kind === 'failed' || event.kind === 'cancelled') resolve(event);
    });
  });
  replay(started.job);
  const abort = (): void => void cancelGeneration(started.job).catch(() => undefined);
  if (signal.aborted) abort();
  signal.addEventListener('abort', abort, { once: true });
  try {
    const end = await ended;
    if (end.kind === 'cancelled') throw new Error('the generation was cancelled');
    if (end.kind === 'failed') throw end.error;
    if (end.kind !== 'finished' || end.count === 0) throw new Error('no image came back');
    const variant = await fetchVariant(started.job, 0);
    // Applied like any command: not in the middle of the user's stroke.
    await strokeDone();
    if (signal.aborted || !target.alive()) throw new Error('the generation was cancelled');
    const layer = layerName(args.prompt);
    target.apply(variant, taken.box, taken.mask, layer);
    return { layer, seed: started.seed, model: args.model };
  } finally {
    signal.removeEventListener('abort', abort);
    agentJobs.delete(started.job);
    agentTabs.delete(started.job);
    void releaseGeneration(started.job).catch(() => undefined);
  }
}
