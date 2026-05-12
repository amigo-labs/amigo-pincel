// Key/value preferences store, backed by the `prefs` object store.
//
// Values pass through the structured-clone algorithm, so anything
// IDB-serializable is allowed (primitives, arrays, plain objects,
// Uint8Array, …). Callers are expected to provide their own value
// shape — this module is intentionally untyped beyond `unknown`.
//
// CLAUDE.md §9 forbids localStorage / sessionStorage in the UI, so
// session prefs (last foreground color, zoom, panel visibility…) live
// here.

import { STORE_PREFS, idbRequest, openDb, transactionDone } from './db';

/** Read a pref by key. Returns `undefined` if the key is unknown. */
export async function getPref(key: string): Promise<unknown> {
  const db = await openDb();
  const tx = db.transaction(STORE_PREFS, 'readonly');
  const store = tx.objectStore(STORE_PREFS);
  const row = (await idbRequest(store.get(key))) as
    | { key: string; value: unknown }
    | undefined;
  return row?.value;
}

/** Write a pref by key. Overwrites any prior value. */
export async function setPref(key: string, value: unknown): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_PREFS, 'readwrite');
  tx.objectStore(STORE_PREFS).put({ key, value });
  await transactionDone(tx);
}

/** Drop a pref by key. No-op if the key is unknown. */
export async function removePref(key: string): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_PREFS, 'readwrite');
  tx.objectStore(STORE_PREFS).delete(key);
  await transactionDone(tx);
}
