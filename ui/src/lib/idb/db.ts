// IndexedDB substrate for Pincel.
//
// One database (`pincel`) with three object stores:
//
//   * `prefs`              — primitive key/value (foreground color,
//                             zoom, panel visibility…).
//   * `recent_files`       — open-recents registry. Each entry pins a
//                             `FileSystemFileHandle` on FSA-capable
//                             browsers so a click can re-open the
//                             on-disk file without a fresh picker.
//   * `autosave_snapshots` — autosave bytes keyed by `(docId, ts)`.
//                             Schema is created here; the timer +
//                             recovery dialog land in M10.3.
//
// Direct IDB rather than the `idb` npm wrapper — CLAUDE.md §9 forbids
// new runtime deps without explicit approval, and the request /
// transaction surface we touch is small enough to wrap inline.

const DB_NAME = 'pincel';
const DB_VERSION = 1;

export const STORE_PREFS = 'prefs';
export const STORE_RECENT_FILES = 'recent_files';
export const STORE_AUTOSAVE = 'autosave_snapshots';

let openPromise: Promise<IDBDatabase> | null = null;

/** True when `indexedDB` is reachable on the global object. False in
 *  SSR contexts or browsers with storage disabled. */
export function isIdbAvailable(): boolean {
  return typeof indexedDB !== 'undefined';
}

/** Open (and lazily upgrade) the Pincel IDB database. Idempotent and
 *  safe to await from multiple call sites — the underlying open
 *  request runs at most once per page load. */
export function openDb(): Promise<IDBDatabase> {
  if (!openPromise) {
    openPromise = new Promise<IDBDatabase>((resolve, reject) => {
      if (!isIdbAvailable()) {
        reject(new Error('IndexedDB is not available in this environment'));
        return;
      }
      const req = indexedDB.open(DB_NAME, DB_VERSION);
      req.onupgradeneeded = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains(STORE_PREFS)) {
          db.createObjectStore(STORE_PREFS, { keyPath: 'key' });
        }
        if (!db.objectStoreNames.contains(STORE_RECENT_FILES)) {
          const recents = db.createObjectStore(STORE_RECENT_FILES, {
            keyPath: 'id',
          });
          recents.createIndex('by_openedAt', 'openedAt');
        }
        if (!db.objectStoreNames.contains(STORE_AUTOSAVE)) {
          db.createObjectStore(STORE_AUTOSAVE, {
            keyPath: ['docId', 'ts'],
          });
        }
      };
      req.onsuccess = () => {
        const db = req.result;
        // Close the connection on a foreign version-change so a
        // future schema upgrade (DB_VERSION bump) in another tab
        // isn't blocked by this tab's stale handle. Clearing
        // `openPromise` lets the next call reopen at the new
        // version. The reopen itself is on-demand — there's no
        // active autosave or recents read here that needs to
        // resume.
        db.onversionchange = () => {
          db.close();
          openPromise = null;
        };
        resolve(db);
      };
      req.onerror = () => reject(req.error ?? new Error('IDB open failed'));
      req.onblocked = () =>
        reject(new Error('IDB open blocked by another tab'));
    });
    // If the open fails, clear the cache so a subsequent call retries
    // instead of repeating the same rejection forever.
    openPromise.catch(() => {
      openPromise = null;
    });
  }
  return openPromise;
}

/** Promise-wrap a single IDBRequest. Rejects with the request's error
 *  if one is set, otherwise a generic Error. */
export function idbRequest<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error ?? new Error('IDB request failed'));
  });
}

/** Promise-wrap a transaction's `complete` / `abort` / `error`
 *  lifecycle. Resolves once the transaction commits. */
export function transactionDone(tx: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error ?? new Error('IDB transaction failed'));
    tx.onabort = () =>
      reject(tx.error ?? new Error('IDB transaction aborted'));
  });
}
