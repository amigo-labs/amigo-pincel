// Font files added through "Add font file…" (docs/specs/system-fonts.md
// AC-20, AC-24), kept across sessions. A face row holds the names and
// attributes the font list needs, so listing reads no font bytes (AC-05);
// the bytes sit in their own row per file.

import { STORE_FONT_FACES, STORE_FONT_FILES, idbRequest, openDb, transactionDone } from './db';

/** One stored face of an added file. */
export interface StoredFace {
  /** `family` + NUL + `style`: a newer face with the same names replaces
   *  the older one (AC-24). */
  readonly key: string;
  readonly family: string;
  readonly style: string;
  readonly weight: number;
  readonly italic: boolean;
  readonly stretch: number;
  readonly postscript: string;
  /** Face index inside the file. */
  readonly index: number;
  /** The `font_files` row holding the bytes. */
  readonly fileId: string;
}

/** The key of a face with `family` and `style`. */
export function faceKey(family: string, style: string): string {
  return `${family}\u0000${style}`;
}

/** Every stored face. */
export async function storedFaces(): Promise<StoredFace[]> {
  const db = await openDb();
  const tx = db.transaction(STORE_FONT_FACES, 'readonly');
  return (await idbRequest(tx.objectStore(STORE_FONT_FACES).getAll())) as StoredFace[];
}

/** The bytes of the stored file `fileId`, or `null` when it is gone. */
export async function storedFontFile(fileId: string): Promise<Uint8Array | null> {
  const db = await openDb();
  const tx = db.transaction(STORE_FONT_FILES, 'readonly');
  const row = (await idbRequest(tx.objectStore(STORE_FONT_FILES).get(fileId))) as
    { id: string; bytes: Uint8Array } | undefined;
  return row?.bytes ?? null;
}

/**
 * Stores one file's bytes and its faces in one transaction; faces replace
 * stored faces with the same key, and a file left without faces is dropped.
 */
export async function storeFontFile(
  fileId: string,
  name: string,
  bytes: Uint8Array,
  faces: readonly StoredFace[],
): Promise<void> {
  const db = await openDb();
  const tx = db.transaction([STORE_FONT_FACES, STORE_FONT_FILES], 'readwrite');
  const faceStore = tx.objectStore(STORE_FONT_FACES);
  const fileStore = tx.objectStore(STORE_FONT_FILES);
  fileStore.put({ id: fileId, name, bytes });
  for (const f of faces) faceStore.put(f);
  const all = (await idbRequest(faceStore.getAll())) as StoredFace[];
  const used = new Set(all.map((f) => f.fileId));
  const ids = (await idbRequest(fileStore.getAllKeys())) as string[];
  for (const id of ids) if (!used.has(id)) fileStore.delete(id);
  await transactionDone(tx);
}
