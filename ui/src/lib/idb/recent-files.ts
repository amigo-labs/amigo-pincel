// Recent-files registry, backed by the `recent_files` object store.
//
// Entries pin a `FileSystemFileHandle` when available so a recent
// click can re-open the on-disk file without re-prompting. Non-FSA
// browsers persist `handle: null` and the caller is expected to skip
// the "click to re-open" affordance — see App.svelte and the
// `hasFsAccess()` gate in `../fs/index.ts`.
//
// Capped at `MAX_RECENTS` entries; oldest-opened entries are evicted
// inside the insert transaction, so the registry never grows past the
// cap.

import {
  STORE_RECENT_FILES,
  idbRequest,
  openDb,
  transactionDone,
} from './db';

/** Per-entry cap. 8 is plenty for a recents menu. */
export const MAX_RECENTS = 8;

export interface RecentFile {
  /** Per-document UUID assigned by App.svelte. Stable across saves of
   *  the same document; new on every `New` / `Open`. */
  id: string;
  /** Display name. For FSA opens this is `handle.name` at open time;
   *  for fallback opens it's the original `File.name`. */
  name: string;
  /** Persistable FSA handle, or `null` on fallback browsers. */
  handle: FileSystemFileHandle | null;
  /** Wall-clock ms when this entry was first inserted. */
  addedAt: number;
  /** Wall-clock ms of the most recent open / save that touched the
   *  on-disk file. Drives LRU eviction and the menu order. */
  openedAt: number;
}

export interface RecentFileInput {
  id: string;
  name: string;
  handle: FileSystemFileHandle | null;
}

/** Insert or refresh a recent-files entry. Preserves any prior
 *  `addedAt` for the same id; bumps `openedAt` to `Date.now()`.
 *  Evicts the oldest entries inside the same transaction so the
 *  store never holds more than `MAX_RECENTS`. Returns the stored
 *  record. */
export async function upsertRecent(
  input: RecentFileInput,
): Promise<RecentFile> {
  const db = await openDb();
  const now = Date.now();
  const prior = await readEntry(db, input.id);
  const entry: RecentFile = {
    id: input.id,
    name: input.name,
    handle: input.handle,
    addedAt: prior?.addedAt ?? now,
    openedAt: now,
  };
  const tx = db.transaction(STORE_RECENT_FILES, 'readwrite');
  const store = tx.objectStore(STORE_RECENT_FILES);
  store.put(entry);
  const keys: string[] = [];
  const cursorReq = store.index('by_openedAt').openKeyCursor(null, 'next');
  cursorReq.onsuccess = () => {
    const cursor = cursorReq.result;
    if (cursor) {
      keys.push(cursor.primaryKey as string);
      cursor.continue();
      return;
    }
    const overflow = keys.length - MAX_RECENTS;
    for (let i = 0; i < overflow; i += 1) {
      const k = keys[i];
      if (k !== undefined) store.delete(k);
    }
  };
  await transactionDone(tx);
  return entry;
}

/** List recents, newest-opened first. Capped at `MAX_RECENTS`. */
export async function listRecents(): Promise<RecentFile[]> {
  const db = await openDb();
  const tx = db.transaction(STORE_RECENT_FILES, 'readonly');
  const store = tx.objectStore(STORE_RECENT_FILES);
  const all = (await idbRequest(store.getAll())) as RecentFile[];
  return all
    .slice()
    .sort((a, b) => b.openedAt - a.openedAt)
    .slice(0, MAX_RECENTS);
}

/** Drop a single entry by id. No-op if the id is unknown. */
export async function removeRecent(id: string): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_RECENT_FILES, 'readwrite');
  tx.objectStore(STORE_RECENT_FILES).delete(id);
  await transactionDone(tx);
}

/** Drop all recents. */
export async function clearRecents(): Promise<void> {
  const db = await openDb();
  const tx = db.transaction(STORE_RECENT_FILES, 'readwrite');
  tx.objectStore(STORE_RECENT_FILES).clear();
  await transactionDone(tx);
}

async function readEntry(
  db: IDBDatabase,
  id: string,
): Promise<RecentFile | undefined> {
  const tx = db.transaction(STORE_RECENT_FILES, 'readonly');
  const store = tx.objectStore(STORE_RECENT_FILES);
  return (await idbRequest(store.get(id))) as RecentFile | undefined;
}
