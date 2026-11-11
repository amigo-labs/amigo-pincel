// Stored OCR models (docs/specs/ocr-dialog.md, Technical notes): the
// `ocr_models` IndexedDB store holds each downloaded model's bytes, keyed
// by language and `tessdata_fast` version. Storing a version removes the
// language's other versions in the same transaction, so updates do not
// pile up old models. Rune-free.

import { STORE_OCR_MODELS, idbRequest, openDb, transactionDone } from '../../../lib/idb/db';
import type { OcrLang } from './catalog';

/** One stored model. */
interface ModelRow {
  readonly lang: OcrLang;
  readonly version: string;
  readonly bytes: Uint8Array<ArrayBuffer>;
}

/** The stored bytes of `lang`'s model `version`, or `null`. Throws when
 *  the storage cannot be read. */
export async function readModel(
  lang: OcrLang,
  version: string,
): Promise<Uint8Array<ArrayBuffer> | null> {
  const db = await openDb();
  const tx = db.transaction(STORE_OCR_MODELS, 'readonly');
  const row = (await idbRequest(tx.objectStore(STORE_OCR_MODELS).get([lang, version]))) as
    ModelRow | undefined;
  return row?.bytes ?? null;
}

/**
 * Stores `bytes` as `lang`'s model `version` and removes the language's
 * other versions, all in one transaction. Throws when the storage cannot
 * be written (quota, blocked storage).
 */
export async function storeModel(
  lang: OcrLang,
  version: string,
  bytes: Uint8Array<ArrayBuffer>,
): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_OCR_MODELS, 'readwrite');
  const store = tx.objectStore(STORE_OCR_MODELS);
  const done = transactionDone(tx);
  const keys = (await idbRequest(store.getAllKeys())) as [OcrLang, string][];
  for (const key of keys) {
    if (key[0] === lang && key[1] !== version) store.delete(key);
  }
  const row: ModelRow = { lang, version, bytes };
  store.put(row);
  await done;
}

/** Removes `lang`'s model `version`; a missing one is no error. */
export async function removeModel(lang: OcrLang, version: string): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_OCR_MODELS, 'readwrite');
  tx.objectStore(STORE_OCR_MODELS).delete([lang, version]);
  await transactionDone(tx);
}
