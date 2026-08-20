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

type FsPermissionMode = 'read' | 'readwrite';

interface FsHandlePermissioned {
  queryPermission?: (descriptor: {
    mode: FsPermissionMode;
  }) => Promise<PermissionState>;
  requestPermission?: (descriptor: {
    mode: FsPermissionMode;
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

async function ensurePermission(
  handle: FileSystemFileHandle,
  mode: FsPermissionMode,
): Promise<boolean> {
  const node = handle as FileSystemFileHandle & FsHandlePermissioned;
  const opts = { mode } as const;
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

/** Ask the FSA handle for read/write permission. Use before in-place
 *  saves. Returns true when granted (already or on prompt); false on
 *  denial. Safe on browsers without permission APIs — returns true
 *  (those browsers don't gate access). */
export function ensureReadWritePermission(
  handle: FileSystemFileHandle,
): Promise<boolean> {
  return ensurePermission(handle, 'readwrite');
}

/** Ask the FSA handle for read permission. Use before reading a
 *  persisted handle (e.g. opening a recent file); the user can deny
 *  write access without blocking the open. */
export function ensureReadPermission(
  handle: FileSystemFileHandle,
): Promise<boolean> {
  return ensurePermission(handle, 'read');
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
    let focusTimer: ReturnType<typeof setTimeout> | null = null;
    const onChange = async () => {
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
    };
    const onCancel = () => settle(null);
    // When the OS picker closes the browser tab regains focus.
    // Wait long enough for a near-simultaneous `change` to win the
    // race (file selection), then settle null on the assumption it
    // was a dismiss. Covers Firefox / Safari where the standard
    // `cancel` event is unreliable.
    const onFocus = () => {
      if (focusTimer !== null) return;
      focusTimer = setTimeout(() => {
        focusTimer = null;
        if (!settled) settle(null);
      }, 300);
    };
    const cleanup = () => {
      if (focusTimer !== null) {
        clearTimeout(focusTimer);
        focusTimer = null;
      }
      input.removeEventListener('change', onChange);
      input.removeEventListener('cancel', onCancel);
      window.removeEventListener('focus', onFocus);
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

    input.addEventListener('change', onChange);
    // Newer browsers fire `cancel` when the picker closes without a
    // selection. On older ones the listener is harmlessly inert and
    // the focus fallback below picks up the cancel.
    input.addEventListener('cancel', onCancel);
    // Defer the focus listener so the click that opens the picker
    // doesn't trigger it; the picker steals focus, then returns it
    // on dismiss.
    queueMicrotask(() => {
      if (settled) return;
      window.addEventListener('focus', onFocus);
    });

    input.click();
  });
}

/** Write `bytes` to disk according to `target`.
 *
 * Resolution order:
 *  1. `forceAs === false` and `target.handle` present and writable:
 *     write through the existing handle in place. Gated on
 *     `createWritable` being a function on the handle so a browser
 *     that exposes the open picker but not the writable surface
 *     still falls through to the next arm.
 *  2. FSA save picker available: prompt `showSaveFilePicker` and
 *     write to the returned handle.
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

  if (
    !forceAs &&
    target.handle &&
    typeof target.handle.createWritable === 'function'
  ) {
    if (await ensureReadWritePermission(target.handle)) {
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
