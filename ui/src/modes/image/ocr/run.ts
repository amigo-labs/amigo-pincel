// One recognition (docs/specs/ocr-dialog.md): get the language's model —
// the stored one if it still matches its pinned hash, else a download that
// is checked and stored — then read the snapshot's source area in a worker
// of its own. Cancelling aborts the download (nothing partial is stored)
// or ends the worker.

import { isTauri } from '../../../lib/platform';
import { compiledCore } from '../core/wasm';
import { OCR_MODELS, modelUrl, type OcrLang } from './catalog';
import { download } from './download';
import { readModel, removeModel, storeModel } from './model-store';
import type { OcrReply, OcrRequest } from './protocol';
import { sha256Hex } from './sha256';

/** What a running recognition is doing. */
export type OcrStep =
  | {
      readonly step: 'download';
      readonly lang: OcrLang;
      readonly loaded: number;
      readonly total: number | null;
    }
  | { readonly step: 'layout' }
  | { readonly step: 'lines'; readonly done: number; readonly total: number };

/** Why a recognition ended without text. */
export type OcrFailure = 'verify' | 'download' | 'failed';

/** How a recognition ended; `null` when it was cancelled. */
export type OcrOutcome =
  | { readonly ok: true; readonly text: string }
  | { readonly ok: false; readonly failure: OcrFailure }
  | null;

/** A running recognition. */
export interface OcrRun {
  /** Resolves when the recognition ends, with `null` once cancelled. */
  readonly outcome: Promise<OcrOutcome>;
  /** Stops the download or the recognition. */
  cancel(): void;
}

/** What `startOcr` reports to its caller. */
export interface OcrCallbacks {
  /** Each new step. */
  step(step: OcrStep): void;
  /** The model is ready and reading starts. */
  recognizing(): void;
}

/** The stored model of `lang` if it matches its pinned hash; a damaged one
 *  is removed (AC-20), and unreadable storage counts as none (AC-19). */
async function storedModel(lang: OcrLang): Promise<Uint8Array<ArrayBuffer> | null> {
  const entry = OCR_MODELS[lang];
  let bytes: Uint8Array<ArrayBuffer> | null;
  try {
    bytes = await readModel(lang, entry.version);
  } catch {
    return null;
  }
  if (bytes === null) return null;
  if ((await sha256Hex(bytes)) === entry.sha256) return bytes;
  await removeModel(lang, entry.version).catch(() => undefined);
  return null;
}

/**
 * Recognizes the text of `snapshot` (bytes of `snapshotDocument`) in
 * `lang`.
 */
export function startOcr(snapshot: Uint8Array, lang: OcrLang, on: OcrCallbacks): OcrRun {
  const abort = new AbortController();
  let settleWorker: (() => void) | null = null;

  const live = () => !abort.signal.aborted;

  /** Reads `model` in a fresh worker; `null` once cancelled. */
  const read = (model: Uint8Array): Promise<OcrReply | null> =>
    new Promise((resolve) => {
      const w = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });
      const end = (reply: OcrReply | null): void => {
        w.terminate();
        settleWorker = null;
        resolve(reply);
      };
      settleWorker = () => end(null);
      w.onmessage = (e: MessageEvent<OcrReply>) => {
        const reply = e.data;
        if (reply.kind === 'progress') {
          const p = reply.progress;
          on.step(
            p.phase === 'layout'
              ? { step: 'layout' }
              : { step: 'lines', done: p.done, total: p.total },
          );
        } else {
          end(reply);
        }
      };
      w.onerror = (e: ErrorEvent) => {
        e.preventDefault();
        end({ kind: 'error', failure: 'failed', message: e.message || 'the OCR worker failed' });
      };
      w.onmessageerror = () =>
        end({ kind: 'error', failure: 'failed', message: 'an OCR message could not be read' });
      const request: OcrRequest = { module: compiledCore(), snapshot, model };
      w.postMessage(request);
    });

  const run = async (): Promise<OcrOutcome> => {
    const entry = OCR_MODELS[lang];
    let model = await storedModel(lang);
    if (!live()) return null;
    if (model === null) {
      on.step({ step: 'download', lang, loaded: 0, total: null });
      try {
        model = await download(modelUrl(lang, isTauri()), abort.signal, (loaded, total) => {
          if (live()) on.step({ step: 'download', lang, loaded, total });
        });
      } catch {
        return live() ? { ok: false, failure: 'download' } : null;
      }
      if (!live()) return null;
      if ((await sha256Hex(model)) !== entry.sha256) return { ok: false, failure: 'verify' };
      if (!live()) return null;
      // A model that cannot be stored still serves this recognition (AC-19).
      await storeModel(lang, entry.version, model).catch(() => undefined);
      if (!live()) return null;
    }
    on.recognizing();
    on.step({ step: 'layout' });
    const reply = await read(model);
    if (reply === null || !live()) return null;
    if (reply.kind === 'done') return { ok: true, text: reply.text };
    if (reply.kind === 'error' && reply.failure === 'model') {
      // The engine rejects the model: drop it so Retry downloads it again.
      await removeModel(lang, entry.version).catch(() => undefined);
    }
    return { ok: false, failure: 'failed' };
  };

  return {
    outcome: run().catch((): OcrOutcome => (live() ? { ok: false, failure: 'failed' } : null)),
    cancel() {
      abort.abort();
      settleWorker?.();
    },
  };
}
