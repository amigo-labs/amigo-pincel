// Images on the system clipboard, for New-image sizing and Edit ▸ Paste
// (spec §9.2). Two ways in: the async Clipboard API, which a menu item or
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
