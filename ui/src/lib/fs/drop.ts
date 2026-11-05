// File drops onto the window, from either host (Decision Log 2026-10-30):
// the browser's HTML5 `dragover` / `drop` events carry `File`s; the
// desktop app reports drops through Tauri's native drag-drop event with
// file paths, read by `read_file_bytes`, so a dropped document keeps its
// path for Save and Open Recent. The start screen and both editors get
// the same `DroppedFile` list (docs/specs/file-access.md AC-15, AC-20 –
// AC-29).
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { isTauri } from '../platform';
import { droppedHandles, mimeFromName, type OpenedFile } from './index';

/** One dropped file: read, or the reason it could not be read. */
export interface DroppedFile {
  /** The file's name (the last path component in the desktop app). */
  name: string;
  /** The file's bytes, `null` when reading failed. */
  file: OpenedFile | null;
  /** Why reading failed; `null` when `file` is set. */
  error: string | null;
}

/** What the window does with a file drag (`watchFileDrops`). */
export interface FileDropHandlers {
  /** A file drag entered or moved over the window (`true`), or left it or
   *  ended (`false`). Answer whether a drop would be taken now. */
  hover: (active: boolean) => boolean;
  /** Files were dropped while `hover` last answered `true`. */
  drop: (files: DroppedFile[]) => void;
}

const errText = (err: unknown): string => (err instanceof Error ? err.message : String(err));

/** True when the drag carries files (not only text or a link). */
function carriesFiles(e: DragEvent): boolean {
  return e.dataTransfer?.types.includes('Files') ?? false;
}

/** Reads `File`s dropped in the browser, each with its File System Access
 *  handle where the browser gave one (`handles`, in drop order): the handle
 *  makes it the same file as an open tab's (document-tabs AC-03). A folder
 *  or an unreadable file comes back with its error. */
export async function readDroppedFiles(
  files: readonly File[],
  handles: Promise<(FileSystemFileHandle | null)[]> = Promise.resolve([]),
): Promise<DroppedFile[]> {
  const found = await handles.catch((): (FileSystemFileHandle | null)[] => []);
  return Promise.all(
    files.map(async (f, i): Promise<DroppedFile> => {
      try {
        const bytes = new Uint8Array(await f.arrayBuffer());
        const handle = found[i] ?? null;
        return {
          name: f.name,
          file: { name: f.name, bytes, mime: f.type, handle, path: null },
          error: null,
        };
      } catch (err) {
        return { name: f.name, file: null, error: errText(err) };
      }
    }),
  );
}

/** Reads files dropped onto the desktop app by their paths. */
export async function readDroppedPaths(paths: readonly string[]): Promise<DroppedFile[]> {
  return Promise.all(
    paths.map(async (path): Promise<DroppedFile> => {
      const name = path.replace(/^.*[/\\]/, '');
      try {
        const raw = await invoke<number[] | ArrayBuffer>('read_file_bytes', { path });
        const bytes = raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
        return {
          name,
          file: { name, bytes, mime: mimeFromName(name), handle: null, path },
          error: null,
        };
      } catch (err) {
        return { name, file: null, error: errText(err) };
      }
    }),
  );
}

/**
 * Watches file drags over the whole window and hands dropped files to
 * `handlers`. A file drag is always kept from the browser's default (which
 * would navigate away to the file); drags of text or links pass through.
 * Returns the function that stops watching.
 */
export function watchFileDrops(handlers: FileDropHandlers): () => void {
  return isTauri() ? watchTauri(handlers) : watchBrowser(handlers);
}

function watchBrowser(h: FileDropHandlers): () => void {
  const onOver = (e: DragEvent) => {
    if (!carriesFiles(e)) return;
    e.preventDefault();
    const taken = h.hover(true);
    if (e.dataTransfer) e.dataTransfer.dropEffect = taken ? 'copy' : 'none';
  };
  // Moving between elements fires `dragleave` with the entered element as
  // its related target; leaving the window has none.
  const onLeave = (e: DragEvent) => {
    if (carriesFiles(e) && e.relatedTarget === null) h.hover(false);
  };
  const onDrop = (e: DragEvent) => {
    if (!carriesFiles(e)) return;
    e.preventDefault();
    const files = Array.from(e.dataTransfer?.files ?? []);
    // Ask before `hover(false)`: it reports the state the drop landed in.
    const taken = h.hover(true);
    h.hover(false);
    if (!taken || files.length === 0) return;
    // Read the handles now: the drop's items are gone after this event.
    void readDroppedFiles(files, droppedHandles(e.dataTransfer)).then(h.drop);
  };
  const onEnd = () => h.hover(false);
  window.addEventListener('dragover', onOver);
  window.addEventListener('dragleave', onLeave);
  window.addEventListener('drop', onDrop);
  window.addEventListener('dragend', onEnd);
  return () => {
    window.removeEventListener('dragover', onOver);
    window.removeEventListener('dragleave', onLeave);
    window.removeEventListener('drop', onDrop);
    window.removeEventListener('dragend', onEnd);
  };
}

function watchTauri(h: FileDropHandlers): () => void {
  let stopped = false;
  let unlisten: (() => void) | null = null;
  // `getCurrentWebview()` throws without the webview's metadata (an old
  // or partial bridge): the window then takes no drops, nothing else breaks.
  let listening: Promise<() => void>;
  try {
    listening = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === 'enter' || p.type === 'over') {
        h.hover(true);
      } else if (p.type === 'leave') {
        h.hover(false);
      } else {
        const taken = h.hover(true);
        h.hover(false);
        if (taken && p.paths.length > 0) void readDroppedPaths(p.paths).then(h.drop);
      }
    });
  } catch (err) {
    console.error('onDragDropEvent failed', err);
    return () => {};
  }
  listening
    .then((fn) => {
      if (stopped) fn();
      else unlisten = fn;
    })
    .catch((err: unknown) => {
      console.error('onDragDropEvent failed', err);
    });
  return () => {
    stopped = true;
    unlisten?.();
  };
}
