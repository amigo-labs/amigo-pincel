// Images on the system clipboard, for New-image sizing and Edit ▸ Copy /
// Cut / Paste (spec §9.2, §5.4). Two ways in: the async Clipboard API, which a menu item or
// an opening dialog can call, and the `paste` event that Ctrl+V / ⌘V
// fires, which needs no permission. Sizes come from `createImageBitmap`,
// never from a canvas readback (CLAUDE.md §9).

/**
 * The first image on the clipboard, or `null` (no image, no API, denied).
 *
 * With `onlyIfAllowed`, reads only where the browser can say up front that
 * reading won't be refused: Chromium answers `clipboard-read` through the
 * Permissions API; Firefox, Safari and WebKitGTK don't, and would show a
 * paste prompt on every unprompted read, so they are skipped.
 */
export async function clipboardImage(opts: { onlyIfAllowed?: boolean } = {}): Promise<Blob | null> {
  if (typeof navigator === 'undefined' || !navigator.clipboard?.read) return null;
  try {
    if (opts.onlyIfAllowed) {
      const status = await navigator.permissions.query({
        name: 'clipboard-read' as PermissionName,
      });
      if (status.state === 'denied') return null;
    }
    for (const item of await navigator.clipboard.read()) {
      const type = item.types.find((t) => t.startsWith('image/'));
      if (type) return await item.getType(type);
    }
  } catch {
    // Unsupported permission name, refused read or an empty clipboard.
  }
  return null;
}

/** The image a `paste` event carries, or `null`. */
export function pasteEventImage(e: ClipboardEvent): File | null {
  for (const file of e.clipboardData?.files ?? []) {
    if (file.type.startsWith('image/')) return file;
  }
  return null;
}

/** Pixel size of an encoded image, or `null` when the browser can't decode it. */
export async function imageSize(blob: Blob): Promise<{ width: number; height: number } | null> {
  try {
    const bitmap = await createImageBitmap(blob);
    const size = { width: bitmap.width, height: bitmap.height };
    bitmap.close();
    return size;
  } catch {
    return null;
  }
}

// ---- Copy / Cut / Paste (spec §5.4, Decision Log 2026-10-29) -------------

/**
 * Writes a PNG to the system clipboard. Call it from the menu click or the
 * keydown that asked for the copy: both are user activation. `false` when
 * the browser has no async Clipboard API or refuses the write.
 */
export async function writeClipboardImage(png: Uint8Array): Promise<boolean> {
  if (
    typeof navigator === 'undefined' ||
    !navigator.clipboard?.write ||
    typeof ClipboardItem === 'undefined'
  ) {
    return false;
  }
  try {
    const blob = new Blob([png.slice()], { type: 'image/png' });
    await navigator.clipboard.write([new ClipboardItem({ 'image/png': blob })]);
    return true;
  } catch {
    return false;
  }
}

/** The last image Pincel copied, kept in-app beside the system clipboard. */
export interface CopyRecord {
  png: Uint8Array;
  /** Canvas position the pixels came from (paste-in-place origin). */
  x: number;
  y: number;
  /** `docTag` of the document copied from. */
  source: string;
  /** `imageKey` of `png`: size plus a hash of the decoded pixels. */
  key: string | null;
  /** The system clipboard refused the PNG. */
  writeFailed: boolean;
}

let lastCopy: CopyRecord | null = null;

/**
 * Records a copy and puts it on the system clipboard. The in-app record
 * survives a refused write, so a following Paste in Pincel still works.
 * Resolves to whether the system write succeeded.
 */
export async function storeCopy(copy: Omit<CopyRecord, 'writeFailed'>): Promise<boolean> {
  const record: CopyRecord = { ...copy, writeFailed: false };
  lastCopy = record;
  const ok = await writeClipboardImage(copy.png);
  if (!ok) record.writeFailed = true;
  return ok;
}

/** What a paste uses: encoded bytes, and the copy origin for a paste-in-place. */
export interface PasteSource {
  bytes: Uint8Array;
  origin: { x: number; y: number } | null;
}

/**
 * Picks what a paste into document `target` uses: the in-app copy when the
 * last system write failed or the system clipboard holds no image;
 * otherwise the system image, which counts as Pincel's own copy when its
 * `keyOf` (decoded size + pixel hash, never the bytes: Chromium re-encodes
 * clipboard PNGs) equals the record's. Pincel's own copy pasted into the
 * document it came from carries its origin; anything else `null`.
 */
export async function resolvePaste(
  system: Blob | null,
  target: string,
  keyOf: (bytes: Uint8Array) => string | null,
): Promise<PasteSource | null> {
  const rec = lastCopy;
  const inPlace = (r: CopyRecord) => (r.source === target ? { x: r.x, y: r.y } : null);
  if (rec && (rec.writeFailed || !system)) return { bytes: rec.png, origin: inPlace(rec) };
  if (!system) return null;
  const bytes = new Uint8Array(await system.arrayBuffer());
  const own = rec !== null && rec.key !== null && keyOf(bytes) === rec.key;
  return { bytes, origin: own && rec ? inPlace(rec) : null };
}

const tags = new WeakMap<object, string>();
let nextTag = 0;

/** A stable id for a document object (`prefix:n`): the in-app copy's source tag. */
export function docTag(doc: object, prefix: string): string {
  let tag = tags.get(doc);
  if (!tag) {
    tag = `${prefix}:${nextTag++}`;
    tags.set(doc, tag);
  }
  return tag;
}

/**
 * Where a pasted image without a copy origin lands: the canvas position at
 * the top-left of the visible viewport (`view`), never negative and, on an
 * axis where the image fits the canvas, shifted so it lies fully inside.
 */
export function pastePosition(
  view: { x: number; y: number },
  image: { width: number; height: number },
  canvas: { width: number; height: number },
): { x: number; y: number } {
  const axis = (v: number, size: number, extent: number): number => {
    const p = Math.max(0, Math.floor(Number.isFinite(v) ? v : 0));
    return size <= extent ? Math.min(p, extent - size) : p;
  };
  return {
    x: axis(view.x, image.width, canvas.width),
    y: axis(view.y, image.height, canvas.height),
  };
}
