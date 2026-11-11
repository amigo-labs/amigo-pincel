// Plain (non-reactive) lookups of the font sources (`source.svelte.ts`):
// where each listed face's bytes come from, and the reads of this session
// (docs/specs/system-fonts.md AC-05). Nothing here drives the UI, so the
// collections stay plain.

/** What `queryLocalFonts()` hands out per face. */
export interface LocalFontData {
  readonly family: string;
  readonly style: string;
  readonly fullName: string;
  readonly postscriptName: string;
  blob(): Promise<Blob>;
}

/** A face's bytes and its index inside them. */
export interface FaceBytes {
  readonly bytes: Uint8Array;
  readonly index: number;
}

/** Local Font Access data by face key. */
export const localData = new Map<string, LocalFontData>();
/** Desktop file and face index by face key. */
export const desktopFaces = new Map<string, { file: number; index: number }>();
/** Added file id and face index by face key. */
export const addedFaces = new Map<string, { fileId: string; index: number }>();
/** Face reads of this session, by face key. */
export const faceReads = new Map<string, Promise<FaceBytes>>();
/** File reads of this session: one per desktop or added file. */
export const fileReads = new Map<string, Promise<Uint8Array>>();
/** CSS family aliases of families rendered from bytes, by family. */
export const cssAliases = new Map<string, string>();

/** A read cached for the session; a failed one is retried next time. */
export function cached<T>(
  cache: Map<string, Promise<T>>,
  key: string,
  read: () => Promise<T>,
): Promise<T> {
  let p = cache.get(key);
  if (!p) {
    const started = read();
    cache.set(key, started);
    started.catch(() => {
      if (cache.get(key) === started) cache.delete(key);
    });
    p = started;
  }
  return p;
}
