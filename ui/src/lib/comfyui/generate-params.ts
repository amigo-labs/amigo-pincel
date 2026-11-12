// Pure rules of image generation (docs/specs/comfyui-generate.md): the
// target box, model families, layer names, the server status the Generate
// panel shows, the English errors `gen.*` tools return and the checks of
// `gen.image`'s arguments. Rune-free, so a Node-side spec can test it.

import type { MessageKey } from '../i18n/en';
import type { ComfyStatus } from './bridge';

/** `Family` in src-tauri/src/comfyui/generate/workflow.rs. */
export type Family = 'sd15' | 'sdxl' | 'flux';

export const FAMILIES: readonly Family[] = ['sd15', 'sdxl', 'flux'];

/** Model family names (product names, not translated). */
export const FAMILY_NAMES: Readonly<Record<Family, string>> = {
  sd15: 'SD 1.5',
  sdxl: 'SDXL',
  flux: 'Flux',
};

/** A checkpoint of the server; `family` for catalog models. */
export interface Checkpoint {
  name: string;
  family: Family | null;
}

/** `Models` in src-tauri/src/comfyui/generate.rs. */
export interface Models {
  checkpoints: Checkpoint[];
  loras: string[];
}

/** The native pixel count of a family (Terms, "Generation size"). */
export function nativePixels(family: Family): number {
  return family === 'sd15' ? 512 * 512 : 1024 * 1024;
}

/** The family a checkpoint generates with: the catalog's, else the model
 *  type the user picked for it, else SDXL (AC-03). */
export function familyOf(checkpoint: Checkpoint | undefined, chosen: Readonly<Record<string, Family>>): Family {
  if (!checkpoint) return 'sdxl';
  return checkpoint.family ?? chosen[checkpoint.name] ?? 'sdxl';
}

/** The target box: the selection's bounding box clipped to the canvas, or
 *  the canvas without a selection; `null` when it is empty (AC-04). */
export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

export function targetBox(
  selection: readonly [number, number, number, number] | null,
  width: number,
  height: number,
): Box | null {
  if (width <= 0 || height <= 0) return null;
  if (selection === null) return { x: 0, y: 0, w: width, h: height };
  const [sx, sy, sw, sh] = selection;
  const x0 = Math.max(0, sx);
  const y0 = Math.max(0, sy);
  const x1 = Math.min(width, sx + sw);
  const y1 = Math.min(height, sy + sh);
  return x1 > x0 && y1 > y0 ? { x: x0, y: y0, w: x1 - x0, h: y1 - y0 } : null;
}

/** "Generated: " plus the first 30 characters of the prompt (AC-12). A
 *  layer name is document content and stays English. */
export function layerName(prompt: string): string {
  return `Generated: ${Array.from(prompt.trim()).slice(0, 30).join('')}`;
}

/** The seed field: `null` when empty (random), `undefined` when invalid. */
export function parseSeed(text: string): number | null | undefined {
  const s = text.trim();
  if (s === '') return null;
  if (!/^\d+$/.test(s)) return undefined;
  const n = Number(s);
  return Number.isSafeInteger(n) ? n : undefined;
}

/** `GenError` in src-tauri/src/comfyui/generate.rs. */
export type GenError =
  | { kind: 'notUsable'; message: string }
  | { kind: 'badParameters'; message: string }
  | { kind: 'missingModel'; name: string }
  | { kind: 'unreachable' }
  | { kind: 'outOfMemory' }
  | { kind: 'failed'; message: string }
  | { kind: 'badAnswer' }
  | { kind: 'fewer'; got: number; wanted: number };

/** A message key with its parameters. */
export interface Message {
  key: MessageKey;
  params?: Record<string, string | number>;
}

/** What the panel shows for a generation error (AC-15). */
export function errorMessage(e: GenError): Message {
  switch (e.kind) {
    case 'notUsable':
      return { key: 'gen.errNotUsable', params: { message: e.message } };
    case 'badParameters':
      return { key: 'gen.errBadParameters', params: { message: e.message } };
    case 'missingModel':
      return { key: 'gen.errMissingModel', params: { name: e.name } };
    case 'unreachable':
      return { key: 'gen.errUnreachable' };
    case 'outOfMemory':
      return { key: 'gen.errOutOfMemory' };
    case 'failed':
      return { key: 'gen.errFailed', params: { message: e.message } };
    case 'badAnswer':
      return { key: 'gen.errBadAnswer' };
    case 'fewer':
      return { key: 'gen.errFewer', params: { got: e.got, wanted: e.wanted } };
  }
}

/** The same error in English, for `gen.*` tool results (AC-19). */
export function errorEnglish(e: GenError): string {
  switch (e.kind) {
    case 'notUsable':
      return e.message;
    case 'badParameters':
      return `ComfyUI refused the request: ${e.message}`;
    case 'missingModel':
      return `the server has no model named "${e.name}"`;
    case 'unreachable':
      return 'ComfyUI stopped answering';
    case 'outOfMemory':
      return 'Not enough GPU memory — try fewer variants, a smaller model or a smaller selection';
    case 'failed':
      return `the generation failed: ${e.message}`;
    case 'badAnswer':
      return 'ComfyUI sent an answer Pincel cannot read';
    case 'fewer':
      return `ComfyUI returned ${e.got} of ${e.wanted} images`;
  }
}

/** Reads a rejected `invoke` as a generation error. */
export function asGenError(err: unknown): GenError {
  if (typeof err === 'object' && err !== null && 'kind' in err) return err as GenError;
  return { kind: 'failed', message: String(err) };
}

/** Why no generation can run on this server, or `null` when it can
 *  (AC-02): the comfyui-connect status as the panel shows it. */
export function statusIssue(status: ComfyStatus): Message | null {
  switch (status.kind) {
    case 'connected':
      if (status.server.missingNodes.length > 0) return { key: 'comfy.tooOld' };
      if (status.server.checkpoints.length === 0) return { key: 'comfy.noModel' };
      return null;
    case 'idle':
    case 'notConfigured':
    case 'installationInvalid':
      return { key: 'gen.notSetUp' };
    case 'notFound':
      return { key: 'comfy.notFound' };
    case 'starting':
      return { key: 'comfy.starting' };
    case 'stopped':
      return { key: 'comfy.stopped' };
    case 'installationNotFound':
      return { key: 'comfy.installationNotFound', params: { path: status.path } };
    case 'invalidUrl':
      return { key: 'comfy.invalidUrl' };
    case 'unreachable':
      return { key: 'comfy.unreachable' };
    case 'notComfy':
      return { key: 'comfy.notComfy' };
    case 'refused':
      return { key: 'comfy.refused', params: { status: status.status } };
    case 'failed':
      return { key: 'comfy.failed', params: { error: status.message } };
  }
}

/** The checked arguments of `gen.image` (AC-16). */
export interface GenImageArgs {
  prompt: string;
  negative: string;
  model: string;
  lora: string | null;
  loraStrength: number;
  strength: number;
  seed: number | null;
}

function list(names: readonly string[]): string {
  return names.length === 0 ? '(none)' : names.map((n) => `"${n}"`).join(', ');
}

/** `gen.image`'s arguments checked against the server's models, or the
 *  English error listing the valid names or the allowed range (AC-17). */
export function checkGenImage(
  args: Record<string, unknown>,
  models: Models,
  panelModel: string | null,
): GenImageArgs | string {
  const prompt = args.prompt;
  if (typeof prompt !== 'string' || prompt.trim() === '') return '`prompt` must be a non-empty string';
  const negative = args.negative ?? '';
  if (typeof negative !== 'string') return '`negative` must be a string';
  const names = models.checkpoints.map((c) => c.name);
  const fallback = panelModel !== null && names.includes(panelModel) ? panelModel : (names[0] ?? null);
  const model = args.model ?? fallback;
  if (typeof model !== 'string' || !names.includes(model)) {
    return `unknown model ${JSON.stringify(model)}; valid models: ${list(names)}`;
  }
  const lora = args.lora ?? null;
  if (lora !== null && (typeof lora !== 'string' || !models.loras.includes(lora))) {
    return `unknown LoRA ${JSON.stringify(lora)}; valid LoRAs: ${list(models.loras)}`;
  }
  const loraStrength = args.lora_strength ?? 1;
  if (typeof loraStrength !== 'number' || !(loraStrength >= 0 && loraStrength <= 2)) {
    return '`lora_strength` must be a number from 0 to 2';
  }
  const strength = args.strength ?? 1;
  if (typeof strength !== 'number' || !(strength >= 0.01 && strength <= 1)) {
    return '`strength` must be a number from 0.01 to 1.0';
  }
  const seed = args.seed ?? null;
  if (seed !== null && (typeof seed !== 'number' || !Number.isSafeInteger(seed) || seed < 0)) {
    return '`seed` must be an integer of at least 0';
  }
  return { prompt: prompt.trim(), negative, model, lora, loraStrength, strength, seed };
}
