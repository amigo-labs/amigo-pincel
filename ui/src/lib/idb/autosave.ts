// Autosave snapshots, backed by the `autosave_snapshots` object store.
//
// One snapshot per document. `writeSnapshot` always evicts prior
// snapshots for the same `docId` inside the same transaction so the
// store holds at most one row per known doc — recovery only ever cares
// about the latest one, and the bytes payload makes keeping a history
// expensive. On a successful save through the FSA / download adapter
// the snapshot for the current `docId` is dropped via
// `removeSnapshots`; the presence of a snapshot for any known doc on
// boot is the signal "this doc has unsaved changes from a prior
// session" that the recovery dialog acts on.
//
// Composite keyPath `[docId, ts]` is created in `db.ts` — keeping `ts`
// in the key would let us extend to a small ring buffer later without
// a schema migration.

import {
  STORE_AUTOSAVE,
  idbRequest,
  openDb,
  transactionDone,
} from './db';

export interface AutosaveSnapshot {
  /** Per-document UUID, owned by App.svelte. */
  docId: string;
  /** Wall-clock ms at write time. */
  ts: number;
  /** Display name carried over from the source file (or
   *  `pincel.aseprite` for never-saved documents). */
  name: string;
  /** Encoded `.aseprite` bytes. Stored as `Uint8Array`; IDB's
   *  structured clone preserves the typed-array view. */
  bytes: Uint8Array;
}

/** Write a snapshot for `docId`. Drops any prior snapshots for the
 *  same id inside the same transaction so the store keeps at most one
 *  row per doc. Returns the stored snapshot. */
export async function writeSnapshot(
  docId: string,
  name: string,
  bytes: Uint8Array,
): Promise<AutosaveSnapshot> {
  const db = await openDb();
  const snapshot: AutosaveSnapshot = {
    docId,
    ts: Date.now(),
    name,
    bytes,
  };
  const tx = db.transaction(STORE_AUTOSAVE, 'readwrite');
  const store = tx.objectStore(STORE_AUTOSAVE);
  // Walk the composite primary key range `[docId, -∞]..[docId, +∞]`
  // and delete any prior rows for this doc before writing the new
  // one. Using `IDBKeyRange.bound` on the composite key keeps the
  // scan tight to a single doc's prefix.
  const range = IDBKeyRange.bound(
    [docId, -Infinity],
    [docId, Infinity],
  );
  const cursorReq = store.openCursor(range);
  cursorReq.onsuccess = () => {
    const cursor = cursorReq.result;
    if (cursor) {
      cursor.delete();
      cursor.continue();
    }
  };
  store.put(snapshot);
  await transactionDone(tx);
  return snapshot;
}

/** Read the latest snapshot for `docId`, or `undefined` if none. */
export async function latestSnapshot(
  docId: string,
): Promise<AutosaveSnapshot | undefined> {
  const db = await openDb();
  const tx = db.transaction(STORE_AUTOSAVE, 'readonly');
  const store = tx.objectStore(STORE_AUTOSAVE);
  const range = IDBKeyRange.bound([docId, -Infinity], [docId, Infinity]);
  // Walk in descending `ts` order and resolve the first hit.
  const cursorReq = store.openCursor(range, 'prev');
  return new Promise((resolve, reject) => {
    cursorReq.onsuccess = () => {
      const cursor = cursorReq.result;
      resolve(cursor ? (cursor.value as AutosaveSnapshot) : undefined);
    };
    cursorReq.onerror = () =>
      reject(cursorReq.error ?? new Error('IDB cursor failed'));
  });
}

/** List the latest snapshot per known doc, newest first. Used to
 *  populate the recovery dialog on boot. */
export async function listLatestSnapshots(): Promise<AutosaveSnapshot[]> {
  const db = await openDb();
  const tx = db.transaction(STORE_AUTOSAVE, 'readonly');
  const store = tx.objectStore(STORE_AUTOSAVE);
  const all = (await idbRequest(store.getAll())) as AutosaveSnapshot[];
  // The single-snapshot-per-doc invariant is maintained at write
  // time, but defensively dedupe in case a partial write left an
  // older row behind. Latest `ts` wins.
  const byDoc = new Map<string, AutosaveSnapshot>();
  for (const s of all) {
    const prior = byDoc.get(s.docId);
    if (!prior || s.ts > prior.ts) byDoc.set(s.docId, s);
  }
  return [...byDoc.values()].sort((a, b) => b.ts - a.ts);
}

/** Drop every snapshot for `docId`. No-op if none exist. */
export async function removeSnapshots(docId: string): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_AUTOSAVE, 'readwrite');
  const store = tx.objectStore(STORE_AUTOSAVE);
  const range = IDBKeyRange.bound([docId, -Infinity], [docId, Infinity]);
  const cursorReq = store.openCursor(range);
  cursorReq.onsuccess = () => {
    const cursor = cursorReq.result;
    if (cursor) {
      cursor.delete();
      cursor.continue();
    }
  };
  await transactionDone(tx);
}
