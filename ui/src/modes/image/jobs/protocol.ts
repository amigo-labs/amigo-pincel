// What the page asks the Image job worker, and how the worker answers it
// (docs/specs/performance-background-jobs.md). Shared by the worker and the
// page's main-thread fallbacks, so both compute with the same functions.

import { core } from '../core/wasm';
import { encodeSnapshot, type EncodeJob } from './encode';

/** One effect run on a layer (the shared `lib/effects/catalog.ts` format). */
export interface EffectJob {
  readonly layer: number;
  readonly name: string;
  readonly params: readonly number[];
}

/** A request to the Image job worker. Every answer is bytes. */
export type ImageJob =
  /** Encode a document snapshot (autosave, save, export). */
  | ({ readonly kind: 'encode' } & EncodeJob)
  /** Keep a document copy under `key` for the effect jobs of one session. */
  | { readonly kind: 'load'; readonly key: string; readonly snapshot: Uint8Array }
  /** Free the copy under `key`. */
  | { readonly kind: 'drop'; readonly key: string }
  /** The composite with the effect previewed (canvas-sized RGBA8). */
  | { readonly kind: 'preview'; readonly key: string; readonly effect: EffectJob }
  /** The effect's region for `core.applyEffectRegion`. */
  | { readonly kind: 'region'; readonly key: string; readonly effect: EffectJob };

/** The composite of document `handle` with `effect` previewed. */
export function previewBytes(handle: number, effect: EffectJob): Uint8Array {
  const rgba = core.previewEffect(handle, effect.layer, effect.name, [...effect.params]);
  return new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength);
}

/** The region `effect` changes in document `handle`. */
export function regionBytes(handle: number, effect: EffectJob): Uint8Array {
  return core.effectRegionBytes(handle, effect.layer, effect.name, [...effect.params]);
}

/** Answers `job` in this thread; `docs` holds the loaded document copies. */
export function runImageJob(job: ImageJob, docs: Map<string, number>): Uint8Array {
  const docOf = (key: string): number => {
    const handle = docs.get(key);
    if (handle === undefined) throw new Error(`no document loaded as ${key}`);
    return handle;
  };
  const drop = (key: string): void => {
    const handle = docs.get(key);
    if (handle === undefined) return;
    docs.delete(key);
    core.closeDocument(handle);
  };
  switch (job.kind) {
    case 'encode':
      return encodeSnapshot(job);
    case 'load':
      drop(job.key);
      docs.set(job.key, core.openSnapshot(job.snapshot));
      return new Uint8Array(0);
    case 'drop':
      drop(job.key);
      return new Uint8Array(0);
    case 'preview':
      return previewBytes(docOf(job.key), job.effect);
    case 'region':
      return regionBytes(docOf(job.key), job.effect);
  }
}
