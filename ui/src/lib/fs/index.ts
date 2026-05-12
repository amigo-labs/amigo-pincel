// Browser file-system adapter for Pincel's open / save flow.
//
// Two implementations behind one surface:
//   * Chromium-based browsers (window.showOpenFilePicker exists) use
//     the File System Access API. `pickAndOpen` returns a
//     FileSystemFileHandle that subsequent saves can write through
//     in place.
//   * Everywhere else (Firefox, Safari) we fall back to <input
//     type="file"> for open and Blob + anchor download for save.
//
// See docs/specs/pincel.md §10.2.

interface FilePickerAcceptType {
  description?: string;
  accept: Record<string, string[]>;
}

interface OpenFilePickerOptions {
  multiple?: boolean;
  excludeAcceptAllOption?: boolean;
  types?: FilePickerAcceptType[];
}

interface SaveFilePickerOptions {
  suggestedName?: string;
  excludeAcceptAllOption?: boolean;
  types?: FilePickerAcceptType[];
}

interface FsAccessWindow {
  showOpenFilePicker?: (
    opts?: OpenFilePickerOptions,
  ) => Promise<FileSystemFileHandle[]>;
  showSaveFilePicker?: (
    opts?: SaveFilePickerOptions,
  ) => Promise<FileSystemFileHandle>;
}

interface FsHandlePermissioned {
  queryPermission?: (descriptor: {
    mode: 'readwrite';
  }) => Promise<PermissionState>;
  requestPermission?: (descriptor: {
    mode: 'readwrite';
  }) => Promise<PermissionState>;
}

const ASEPRITE_TYPES: FilePickerAcceptType[] = [
  {
    description: 'Aseprite sprite',
    accept: { 'application/octet-stream': ['.aseprite', '.ase'] },
  },
];

export interface OpenedFile {
  name: string;
  bytes: Uint8Array;
  /** Present only on FSA-capable browsers. Lets a later save write
   *  back to the same on-disk file without prompting the user. */
  handle: FileSystemFileHandle | null;
}

export interface SaveTarget {
  /** Display / suggested-name string. Always set; defaults to e.g.
   *  `pincel.aseprite` for a never-saved document. */
  name: string;
  /** When non-null, the next save writes here in place; otherwise the
   *  next save prompts a picker (FSA) or downloads (fallback). */
  handle: FileSystemFileHandle | null;
}

/** True when the current browser exposes `window.showOpenFilePicker`,
 *  i.e. supports in-place save. Used to drive UI labels. */
export function hasFsAccess(): boolean {
  if (typeof window === 'undefined') return false;
  return (
    typeof (window as Window & FsAccessWindow).showOpenFilePicker ===
    'function'
  );
}

function fsAccess(): FsAccessWindow {
  return window as Window & FsAccessWindow;
}

function isUserCancel(err: unknown): boolean {
  return err instanceof DOMException && err.name === 'AbortError';
}

async function ensureReadWrite(
  handle: FileSystemFileHandle,
): Promise<boolean> {
  const node = handle as FileSystemFileHandle & FsHandlePermissioned;
  const opts = { mode: 'readwrite' } as const;
  if (node.queryPermission) {
    const state = await node.queryPermission(opts);
    if (state === 'granted') return true;
  }
  if (node.requestPermission) {
    const state = await node.requestPermission(opts);
    return state === 'granted';
  }
  return true;
}

/** Prompt the user to pick a sprite file. Returns `null` on cancel. */
export async function pickAndOpen(): Promise<OpenedFile | null> {
  const fs = fsAccess();
  if (fs.showOpenFilePicker) {
    try {
      const handles = await fs.showOpenFilePicker({
        types: ASEPRITE_TYPES,
        multiple: false,
      });
      const handle = handles[0];
      if (!handle) return null;
      const file = await handle.getFile();
      const bytes = new Uint8Array(await file.arrayBuffer());
      return { name: file.name, bytes, handle };
    } catch (err) {
      if (isUserCancel(err)) return null;
      throw err;
    }
  }
  return openViaInput();
}

function openViaInput(): Promise<OpenedFile | null> {
  return new Promise((resolve, reject) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.aseprite,.ase';
    input.style.display = 'none';
    document.body.appendChild(input);

    let settled = false;
    const cleanup = () => {
      input.parentNode?.removeChild(input);
    };
    const settle = (val: OpenedFile | null) => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(val);
    };
    const fail = (err: unknown) => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(err);
    };

    input.addEventListener('change', async () => {
      const file = input.files?.[0];
      if (!file) {
        settle(null);
        return;
      }
      try {
        const bytes = new Uint8Array(await file.arrayBuffer());
        settle({ name: file.name, bytes, handle: null });
      } catch (err) {
        fail(err);
      }
    });
    // Newer browsers fire `cancel` when the picker closes without a
    // selection. On older ones the listener is harmlessly inert.
    input.addEventListener('cancel', () => settle(null));

    input.click();
  });
}

/** Write `bytes` to disk according to `target`.
 *
 * Resolution order:
 *  1. `forceAs === false` and `target.handle` present and writable:
 *     write through the existing handle in place.
 *  2. FSA available: prompt `showSaveFilePicker` and write to the
 *     returned handle.
 *  3. Fallback: trigger a Blob + anchor download named `target.name`.
 *
 * Returns the (possibly updated) target so the caller can persist the
 * fresh handle for subsequent in-place saves. On user cancel of the
 * save-as picker, returns the original target unchanged. */
export async function saveBytes(
  // ArrayBuffer-backed (not ArrayBufferLike): the FSA `write()` and
  // `Blob` ctors reject SharedArrayBuffer-compatible views under
  // current lib.dom typings. Callers copy through `new Uint8Array(…)`
  // before invoking, which narrows the buffer type.
  bytes: Uint8Array<ArrayBuffer>,
  target: SaveTarget,
  opts: { forceAs?: boolean } = {},
): Promise<SaveTarget> {
  const fs = fsAccess();
  const forceAs = opts.forceAs ?? false;

  if (!forceAs && target.handle && fs.showSaveFilePicker) {
    if (await ensureReadWrite(target.handle)) {
      await writeHandle(target.handle, bytes);
      return target;
    }
  }
  if (fs.showSaveFilePicker) {
    try {
      const handle = await fs.showSaveFilePicker({
        suggestedName: target.name,
        types: ASEPRITE_TYPES,
      });
      await writeHandle(handle, bytes);
      return { name: handle.name, handle };
    } catch (err) {
      if (isUserCancel(err)) return target;
      throw err;
    }
  }
  saveViaDownload(target.name, bytes);
  return target;
}

async function writeHandle(
  handle: FileSystemFileHandle,
  bytes: Uint8Array<ArrayBuffer>,
): Promise<void> {
  const writable = await handle.createWritable();
  try {
    await writable.write(bytes);
  } finally {
    await writable.close();
  }
}

function saveViaDownload(name: string, bytes: Uint8Array<ArrayBuffer>): void {
  const blob = new Blob([bytes], {
    type: 'application/octet-stream',
  });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  // Defer the revoke: some browsers cancel the download if the blob
  // URL is revoked synchronously after `.click()`.
  setTimeout(() => URL.revokeObjectURL(url), 0);
}
