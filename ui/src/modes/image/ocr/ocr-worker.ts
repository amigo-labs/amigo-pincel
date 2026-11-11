// The OCR worker (docs/specs/ocr-dialog.md, Technical notes): its own wasm
// instance reads the source area of one document snapshot with one model
// and reports progress. It is not the shared job worker, so Cancel can end
// it without touching autosave, export or effects; the page starts one
// worker per recognition and terminates it afterwards.

import { core, initCore } from '../core/wasm';
import type { OcrProgress, OcrReply, OcrRequest } from './protocol';

const scope = self as unknown as {
  onmessage: ((e: MessageEvent<OcrRequest>) => unknown) | null;
  postMessage(msg: OcrReply): void;
};

const messageOf = (err: unknown): string => (err instanceof Error ? err.message : String(err));

scope.onmessage = async (e) => {
  const { module, snapshot, model } = e.data;
  try {
    // Without the page's module this fetches and compiles the core itself.
    await initCore(module ?? undefined);
  } catch (err) {
    scope.postMessage({ kind: 'error', failure: 'failed', message: messageOf(err) });
    return;
  }
  let loaded: ReturnType<typeof core.ocrModel>;
  try {
    loaded = core.ocrModel(model);
  } catch (err) {
    scope.postMessage({ kind: 'error', failure: 'model', message: messageOf(err) });
    return;
  }
  try {
    const session = core.ocrSession(snapshot);
    try {
      const text = session.read(loaded, (progress: OcrProgress) =>
        scope.postMessage({ kind: 'progress', progress }),
      );
      scope.postMessage({ kind: 'done', text });
    } finally {
      session.free();
    }
  } catch (err) {
    scope.postMessage({ kind: 'error', failure: 'failed', message: messageOf(err) });
  } finally {
    loaded.free();
  }
};
