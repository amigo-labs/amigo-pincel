// Messages between the page and the OCR worker (docs/specs/ocr-dialog.md,
// Technical notes): the page sends one request per worker, the worker
// reports progress and ends with the text or a failure. Cancelling ends
// the worker. Rune-free.

import type { OcrProgress } from '../core/generated/OcrProgress';

export type { OcrProgress };

/** What the page sends the OCR worker, once. */
export interface OcrRequest {
  /** The page's compiled core, instantiated instead of fetched again. */
  readonly module: WebAssembly.Module | null;
  /** `snapshotDocument` bytes of the document to read. */
  readonly snapshot: Uint8Array;
  /** The language model's bytes. */
  readonly model: Uint8Array;
}

/** Why a recognition failed: the engine rejected the model, or anything
 *  else went wrong (start-up, memory, an error while reading). */
export type OcrFailureKind = 'model' | 'failed';

/** What the OCR worker answers. */
export type OcrReply =
  | { readonly kind: 'progress'; readonly progress: OcrProgress }
  | { readonly kind: 'done'; readonly text: string }
  | { readonly kind: 'error'; readonly failure: OcrFailureKind; readonly message: string };
