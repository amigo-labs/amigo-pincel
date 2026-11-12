// Image generation in Image mode (docs/specs/comfyui-generate.md): the
// Generate panel's settings, the server it generates on, and each tab's
// generation — its job, progress and variants, which stay with the tab
// (AC-14). The job runs in the desktop app's backend; the input and the
// mask come from the core when the user clicks Generate (AC-08).

import { core } from '../core/wasm';
import { syncInfo } from '../core/controller';
import { editor, tabs, type ImageTab } from '../stores/editor.svelte';
import { gestureControl } from '../tools/gesture-control';
import { getPref, setPref } from '../../../lib/idb/prefs';
import { comfyConnect, onComfyStatus, type ComfyStatus } from '../../../lib/comfyui/bridge';
import {
  cancelGeneration,
  fetchVariant,
  generationModels,
  onGenerationEvent,
  releaseGeneration,
  startGeneration,
  toBase64,
  type GenEvent,
} from '../../../lib/comfyui/generate-bridge';
import {
  GEN_PREF,
  asGenError,
  familyOf,
  layerName,
  nativePixels,
  statusIssue,
  targetBox,
  type Box,
  type Family,
  type GenError,
  type GenImageArgs,
  type Models,
} from '../../../lib/comfyui/generate-params';
import { agentJobs, agentTabs, holdEarly, jobTabs, takeEarly } from './job-routing';
import { strokeDone } from '../../../lib/agent/busy';
import type { GeneratedLayer } from '../../../lib/shell/types';

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
  /** The finished variants at generation size, encoded. */
  variants: Uint8Array[];
  /** Object URLs of the variants for the strip. */
  thumbs: string[];
  selected: number;
  error: GenError | null;
}

/** Each Image tab's generation, by tab id. */
export const genTabs: Record<number, TabGen> = $state({});

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

function idle(box: Box, mask: Uint8Array, prompt: string): TabGen {
  return {
    phase: 'starting',
    job: null,
    ahead: 0,
    step: 0,
    total: 0,
    live: null,
    box,
    mask,
    prompt,
    variants: [],
    thumbs: [],
    selected: 0,
    error: null,
  };
}

/** Drops a tab's variants and forgets its job (AC-13). */
function clear(tabId: number): void {
  const gen = genTabs[tabId];
  if (!gen) return;
  for (const url of gen.thumbs) URL.revokeObjectURL(url);
  if (gen.job !== null) {
    jobTabs.delete(gen.job);
    void releaseGeneration(gen.job).catch(() => undefined);
  }
  delete genTabs[tabId];
}

/** What the core reads for a generation in `handle`'s current state. */
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

function capture(handle: number, family: Family, strength: number): Capture | null {
  const info = core.documentInfo(handle);
  const bounds = info.has_selection ? core.selectionBounds(handle) : null;
  const box = targetBox(bounds, info.width, info.height);
  if (box === null) return null;
  const [width, height] = core.generationSize(nativePixels(family), box.w, box.h);
  const mask = core.generationMask(handle, box.x, box.y, box.w, box.h);
  let input: Uint8Array | null = null;
  // Inpainting reads the context around the mask; refining reads the input.
  if (mask.length > 0 || strength < 1) {
    const png = core.generationInput(handle, box.x, box.y, box.w, box.h, width, height);
    // A fully transparent layer has nothing to refine: generate as at 100 %.
    if (png.length > 0) input = png;
  }
  return {
    box,
    mask,
    input: input && toBase64(input),
    maskPng: input && mask.length > 0 ? toBase64(core.generationMaskPng(mask, box.w, box.h, width, height)) : null,
    width,
    height,
    strength: input ? strength : 1,
  };
}

/** Generate in `tab` with the panel's settings (AC-05 – AC-07); the tab's
 *  earlier variants are dropped (AC-13). */
export async function generate(tab: ImageTab): Promise<void> {
  const handle = tab.doc.handle;
  const model = genSettings.model;
  const prompt = genInput.prompt.trim();
  if (handle === null || model === null || prompt === '') return;
  const family = currentFamily();
  const seedText = genInput.seed.trim();
  const seed = seedText === '' ? null : Number(seedText);
  if (seed !== null && !(Number.isSafeInteger(seed) && seed >= 0)) return;
  const taken = capture(handle, family, genSettings.strength / 100);
  if (taken === null) return;
  clear(tab.id);
  const gen = idle(taken.box, taken.mask, prompt);
  genTabs[tab.id] = gen;
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
    const current = genTabs[tab.id];
    if (current === undefined || current.phase !== 'starting') {
      // Dropped while starting (Discard, tab closed).
      void releaseGeneration(started.job).catch(() => undefined);
      return;
    }
    current.job = started.job;
    current.phase = 'queued';
    jobTabs.set(started.job, tab.id);
    replay(started.job);
  } catch (err) {
    const current = genTabs[tab.id];
    if (current) {
      current.phase = 'idle';
      current.error = asGenError(err);
    }
  }
}

async function finish(tabId: number, job: number, count: number, error: GenError | null): Promise<void> {
  const gen = genTabs[tabId];
  if (!gen || gen.job !== job) return;
  gen.phase = 'fetching';
  const variants: Uint8Array[] = [];
  try {
    for (let i = 0; i < count; i++) variants.push(await fetchVariant(job, i));
  } catch (err) {
    error ??= asGenError(err);
  }
  jobTabs.delete(job);
  void releaseGeneration(job).catch(() => undefined);
  const now = genTabs[tabId];
  if (!now || now.job !== job) return;
  now.job = null;
  now.variants = variants;
  now.thumbs = variants.map((v) => URL.createObjectURL(new Blob([v as BlobPart], { type: 'image/png' })));
  now.selected = 0;
  now.error = error;
  now.live = null;
  now.phase = variants.length > 0 ? 'done' : 'idle';
}

function onEvent(event: GenEvent): void {
  const agent = agentJobs.get(event.job);
  if (agent) {
    agent(event);
    return;
  }
  const tabId = jobTabs.get(event.job);
  if (tabId === undefined) {
    holdEarly(event);
    return;
  }
  const gen = genTabs[tabId];
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
      void finish(tabId, event.job, event.count, event.error);
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
      delete genTabs[tabId];
      break;
  }
}

/** Cancel: off the server's queue, or stopped if it runs (AC-09). */
export function cancel(tabId: number): void {
  const gen = genTabs[tabId];
  if (!gen) return;
  if (gen.job === null) {
    // Still starting: the start drops its job when it returns.
    delete genTabs[tabId];
    return;
  }
  void cancelGeneration(gen.job).catch(() => undefined);
}

/** Shows variant `index` on the canvas instead (AC-11). */
export function selectVariant(tabId: number, index: number): void {
  const gen = genTabs[tabId];
  if (gen && index >= 0 && index < gen.variants.length) gen.selected = index;
}

/** Discard: the variants and their preview go (AC-13). */
export function discard(tabId: number): void {
  clear(tabId);
}

/** The variant fitted to its target box with the mask applied. */
export function fittedVariant(gen: TabGen, index: number): Uint8ClampedArray | null {
  const data = gen.variants[index];
  return data ? core.fitVariant(data, gen.box.w, gen.box.h, gen.mask) : null;
}

/** Adds `rgba` over `box` as a new layer above the active layer of
 *  `handle` — one undo step after committing a live object. */
function addLayer(handle: number, rgba: Uint8ClampedArray, box: Box, prompt: string): string {
  gestureControl.commitLive();
  gestureControl.commitTransform();
  const name = layerName(prompt);
  core.addGeneratedLayer(handle, rgba, box.x, box.y, box.w, box.h, name);
  if (editor.handle === handle) syncInfo();
  return name;
}

/** Apply: the selected variant becomes a new layer (AC-12). */
export function apply(tab: ImageTab): void {
  const gen = genTabs[tab.id];
  const handle = tab.doc.handle;
  if (!gen || handle === null || tabs.active !== tab) return;
  const rgba = fittedVariant(gen, gen.selected);
  if (rgba === null) return;
  addLayer(handle, rgba, gen.box, gen.prompt);
  clear(tab.id);
}

/** The tab closed: its generation is cancelled and its variants go
 *  (AC-21). */
export function dropTabGeneration(tabId: number): void {
  const gen = genTabs[tabId];
  if (gen?.job != null) void cancelGeneration(gen.job).catch(() => undefined);
  clear(tabId);
  // An agent's generation for this tab ends too, adding nothing (AC-20).
  for (const [job, owner] of agentTabs) {
    if (owner === tabId) void cancelGeneration(job).catch(() => undefined);
  }
}

/** One variant for an agent call, applied without preview (AC-16); the
 *  panel's own variants stay. `signal` cancels the job (AC-20). */
export async function generateForAgent(
  tab: ImageTab,
  args: GenImageArgs,
  models: Models,
  signal: AbortSignal,
): Promise<GeneratedLayer> {
  ensureListening();
  await loadGenSettings();
  const handle = tab.doc.handle;
  if (handle === null) throw new Error('no document is open');
  const checkpoint = models.checkpoints.find((c) => c.name === args.model);
  const family = familyOf(checkpoint, genSettings.families);
  const taken = capture(handle, family, args.strength);
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
  agentTabs.set(started.job, tab.id);
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
    if (signal.aborted || tab.doc.handle !== handle) throw new Error('the generation was cancelled');
    const rgba = core.fitVariant(variant, taken.box.w, taken.box.h, taken.mask);
    const layer = addLayer(handle, rgba, taken.box, args.prompt);
    return { layer, seed: started.seed, model: args.model };
  } finally {
    signal.removeEventListener('abort', abort);
    agentJobs.delete(started.job);
    agentTabs.delete(started.job);
    void releaseGeneration(started.job).catch(() => undefined);
  }
}
