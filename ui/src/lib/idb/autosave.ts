// Autosave snapshots, backed by two object stores:
//
//   * `autosave_snapshots` — `{docId, ts, bytes}`. The heavy byte
//                            payload; only read when the user clicks
//                            Recover.
//   * `autosave_meta`      — `{docId, ts, name, byteLength}`. Walked
//                            on every boot by the recovery probe.
//
// Splitting metadata from bytes (v2 schema) keeps the boot read cost
// O(meta) instead of O(bytes) — a single snapshot of a 1024×1024
// sprite can run multi-MB, and reading every doc's bytes on every
// page load before the user even decides whether to recover is the
// kind of cost that surfaces as a long blank screen.
//
// One snapshot per document. `writeSnapshot` deletes any prior rows
// for the same `docId` in both stores via `IDBKeyRange.bound` before
// the `put` — both requests are queued sequentially on the same
// readwrite transaction, so IDB executes them in order. (The earlier
// cursor-walk approach allowed the put to race ahead of the cursor's
// delete requests and either left the prior row in place or had the
// cursor observe and delete the new row.)

import {
  STORE_AUTOSAVE,
  STORE_AUTOSAVE_META,
  idbRequest,
  openDb,
  transactionDone,
} from './db';

export interface AutosaveSnapshotMeta {
  /** Per-document UUID, owned by App.svelte. */
  docId: string;
  /** Wall-clock ms at write time. */
  ts: number;
  /** Display name carried over from the source file (or
   *  `pincel.aseprite` for never-saved documents). */
  name: string;
  /** Byte size of the `.aseprite` payload — surfaced in the
   *  recovery dialog so the user can spot anomalies. */
  byteLength: number;
}

export interface AutosaveSnapshot extends AutosaveSnapshotMeta {
  /** Encoded `.aseprite` bytes. Stored as `Uint8Array`; IDB's
   *  structured clone preserves the typed-array view. */
  bytes: Uint8Array;
}

/** Write a snapshot for `docId`. Drops any prior snapshots for the
 *  same id in both stores inside the same readwrite transaction, then
 *  puts the new bytes + meta rows. Returns the stored snapshot. */
export async function writeSnapshot(
  docId: string,
  name: string,
  bytes: Uint8Array,
): Promise<AutosaveSnapshot> {
  const db = await openDb();
  const ts = Date.now();
  const snapshot: AutosaveSnapshot = {
    docId,
    ts,
    name,
    byteLength: bytes.length,
    bytes,
  };
  const meta: AutosaveSnapshotMeta = {
    docId,
    ts,
    name,
    byteLength: bytes.length,
  };
  const tx = db.transaction(
    [STORE_AUTOSAVE, STORE_AUTOSAVE_META],
    'readwrite',
  );
  const bytesStore = tx.objectStore(STORE_AUTOSAVE);
  const metaStore = tx.objectStore(STORE_AUTOSAVE_META);
  const range = IDBKeyRange.bound([docId, -Infinity], [docId, Infinity]);
  // Order matters — IDB processes requests in the order they were
  // queued on the transaction. Deletes are queued before the puts so
  // any prior rows for this doc are gone before the new ones land.
  bytesStore.delete(range);
  metaStore.delete(range);
  // The bytes row intentionally skips `name` (it lives in the meta
  // row) so the heavy payload store doesn't carry redundant fields.
  bytesStore.put({ docId, ts, bytes });
  metaStore.put(meta);
  await transactionDone(tx);
  return snapshot;
}

/** Read the bytes row for `docId`, or `undefined` if none. Used by
 *  the recovery flow once the user clicks Recover — until then the
 *  bytes payload stays on disk. */
export async function latestSnapshot(
  docId: string,
): Promise<AutosaveSnapshot | undefined> {
  const db = await openDb();
  const tx = db.transaction(
    [STORE_AUTOSAVE, STORE_AUTOSAVE_META],
    'readonly',
  );
  const bytesStore = tx.objectStore(STORE_AUTOSAVE);
  const metaStore = tx.objectStore(STORE_AUTOSAVE_META);
  const range = IDBKeyRange.bound([docId, -Infinity], [docId, Infinity]);
  // Walk both stores in descending `ts` order; resolve the first hit
  // from each. The meta lookup carries the display name; the bytes
  // lookup carries the payload.
  const meta = await firstCursorValue<AutosaveSnapshotMeta>(
    metaStore.openCursor(range, 'prev'),
  );
  const bytesRow = await firstCursorValue<{
    docId: string;
    ts: number;
    bytes: Uint8Array;
  }>(bytesStore.openCursor(range, 'prev'));
  if (!meta || !bytesRow) return undefined;
  return {
    docId: meta.docId,
    ts: meta.ts,
    name: meta.name,
    byteLength: meta.byteLength,
    bytes: bytesRow.bytes,
  };
}

/** List the latest metadata row per known doc, newest first. Read
 *  on boot to populate the recovery dialog — never reads bytes. */
export async function listLatestSnapshots(): Promise<AutosaveSnapshotMeta[]> {
  const db = await openDb();
  const tx = db.transaction(STORE_AUTOSAVE_META, 'readonly');
  const store = tx.objectStore(STORE_AUTOSAVE_META);
  const all = (await idbRequest(store.getAll())) as AutosaveSnapshotMeta[];
  // The single-row-per-doc invariant is maintained at write time,
  // but defensively dedupe in case a partial write left an older
  // row behind. Latest `ts` wins.
  const byDoc = new Map<string, AutosaveSnapshotMeta>();
  for (const s of all) {
    const prior = byDoc.get(s.docId);
    if (!prior || s.ts > prior.ts) byDoc.set(s.docId, s);
  }
  return [...byDoc.values()].sort((a, b) => b.ts - a.ts);
}

/** Drop every row (bytes + meta) for `docId`. No-op if none exist. */
export async function removeSnapshots(docId: string): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(
    [STORE_AUTOSAVE, STORE_AUTOSAVE_META],
    'readwrite',
  );
  const range = IDBKeyRange.bound([docId, -Infinity], [docId, Infinity]);
  tx.objectStore(STORE_AUTOSAVE).delete(range);
  tx.objectStore(STORE_AUTOSAVE_META).delete(range);
  await transactionDone(tx);
}

function firstCursorValue<T>(
  req: IDBRequest<IDBCursorWithValue | null>,
): Promise<T | undefined> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => {
      const cursor = req.result;
      resolve(cursor ? (cursor.value as T) : undefined);
    };
    req.onerror = () =>
      reject(req.error ?? new Error('IDB cursor failed'));
  });
}
