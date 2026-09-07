// Shell session state: which editor is mounted and what it opens.
//
// `seq` is the remount key — every launch bumps it so `{#key}` in
// App.svelte tears the previous editor down (freeing its wasm document)
// and mounts a fresh one that consumes `pending`. Only one editor is
// ever mounted, so the two keyboard maps never overlap.
import type { FileFormat, OpenedFile } from '../fs';
import type { EditorMode, NewDocParams } from './types';

export type ShellMode = 'start' | EditorMode;

export interface PendingLaunch {
  file: OpenedFile | null;
  create: NewDocParams | null;
}

export const session = $state({
  mode: 'start' as ShellMode,
  seq: 0,
  pending: null as PendingLaunch | null,
  /** Mirrored from the mounted editor via `EditorProps.onDirtyChange`. */
  dirty: false,
});

/** Which editor a file format belongs to. Sprites are Pixel mode;
 *  every raster format is Image mode. (A PNG opened from inside Pixel
 *  mode stays there — that path never asks the shell.) */
export function detectMode(format: FileFormat | null): EditorMode {
  return format === 'aseprite' ? 'pixel' : 'image';
}

/** Mount `mode` with a fresh session that opens `pending`. */
export function launch(mode: EditorMode, pending: PendingLaunch): void {
  session.pending = pending;
  session.dirty = false;
  session.mode = mode;
  session.seq += 1;
}
