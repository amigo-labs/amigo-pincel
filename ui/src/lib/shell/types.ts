// Contract between the app shell (src/App.svelte) and the two editors
// (src/modes/pixel/PixelEditor.svelte, src/modes/image/ImageEditor.svelte).
// See docs/specs/pincel.md §1.1 "Two modes" and §9.2.
import type { OpenedFile } from '../fs';
import type { RecentFile } from '../idb/recent-files';
import type { EditorMenuHandlers } from '../menu';

/** Which editor a document lives in. */
export type EditorMode = 'pixel' | 'image';

/** Parameters of a fresh document created from the New dialog. */
export interface NewDocParams {
  mode: EditorMode;
  width: number;
  height: number;
}

/** Props every editor component accepts. The shell mounts exactly one
 *  editor at a time and remounts it per session, so `initialFile` /
 *  `initialNew` are consumed once, right after the editor's core loaded. */
export interface EditorProps {
  /** File to open on mount (bytes already read), or `null`. */
  initialFile: OpenedFile | null;
  /** Blank document to create on mount, or `null`. Exactly one of
   *  `initialFile` / `initialNew` is non-null. */
  initialNew: NewDocParams | null;
  /** The editor's "New" affordance: the shell owns the New dialog. */
  onRequestNew: () => void;
  /** The editor's "Open…" affordance: the shell picks the file and routes
   *  it to the right mode. (Pixel mode keeps its own Open for sprites and
   *  PNGs and only hands foreign formats over via `onOpenForeign`.) */
  onRequestOpen: () => void;
  /** The editor opened a file that belongs to the other mode; the shell
   *  re-routes it (after its own unsaved-changes guard). */
  onOpenForeign: (file: OpenedFile) => void;
  /** Open a document the editor was handed (Paste as New Image): the shell
   *  runs its unsaved-changes guard, then routes it like Open…. */
  onOpenFile: (file: OpenedFile) => void;
  /** Mirror of the editor's dirty state, for the shell's guards. */
  onDirtyChange: (dirty: boolean) => void;
  /** Re-open a recent through the shell, which reads the file (FSA handle
   *  or Tauri path), runs the unsaved-changes guard and routes it to the
   *  mode the format belongs to. */
  onOpenRecent: (recent: RecentFile) => void;
  /** Native-menu (Tauri) items the editor serves; `null` on unmount. */
  registerMenuHandlers: (handlers: EditorMenuHandlers | null) => void;
}
