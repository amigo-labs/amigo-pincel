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

/** One open document (a tab) of the mounted editor, as the shell's
 *  unsaved-changes prompt and the desktop close guard see it
 *  (docs/specs/unsaved-changes-prompt.md, docs/specs/document-tabs.md). */
export interface EditorDocument {
  /** The name the top bar shows, with its extension. */
  name(): string;
  /** The document has unsaved changes (the top bar's unsaved marker). */
  dirty(): boolean;
  /** File ▸ Save, with every picker and question it asks; resolves `true`
   *  once the document is saved, `false` when it was cancelled or failed
   *  (the failure is reported as File ▸ Save reports it). */
  save(): Promise<boolean>;
  /** The File ▸ Save / Save As running now, or `null`; it resolves like
   *  `save()` once that save ends. */
  running(): Promise<boolean> | null;
  /** Drop the document's autosave snapshot and stop autosaving: the app
   *  is about to close without saving. */
  discard(): Promise<void>;
  /** Make this document's tab the active one (the prompt asks about it,
   *  or Save All saves it). */
  activate(): void;
}

/** The mounted editor's tabs, which the shell routes same-mode documents
 *  to instead of remounting (docs/specs/document-tabs.md AC-02, AC-03). */
export interface EditorSession {
  /** Open an already-read file as a new tab, or activate the tab that
   *  holds the same file; resolves `false` when it could not be opened
   *  (the editor reports why, the active tab stays). */
  open(file: OpenedFile): Promise<boolean>;
  /** Create a blank document as a new tab. */
  create(params: NewDocParams): void;
}

/** Props every editor component accepts. The shell mounts exactly one
 *  editor at a time and remounts it per session, so `initialFile` /
 *  `initialNew` (the first tab) are consumed once, right after the
 *  editor's core loaded; later same-mode documents arrive through the
 *  registered `EditorSession`. */
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
   *  re-routes it (replacing the session after its unsaved-changes guard). */
  onOpenForeign: (file: OpenedFile) => void;
  /** Open a document the editor was handed (Paste as New Image): the shell
   *  routes it like Open… — a tab in this mode, else a new session. */
  onOpenFile: (file: OpenedFile) => void;
  /** Run `close` (closing `doc`'s tab) once the shell's unsaved-changes
   *  prompt about that one document allows it (document-tabs AC-08). */
  confirmClose: (doc: EditorDocument, close: () => void) => void;
  /** Hand the shell one of the editor's documents (each tab, in opening
   *  order); returns the unregister function the editor calls when the tab
   *  closes or the editor unmounts. */
  registerDocument: (doc: EditorDocument) => () => void;
  /** Hand the shell the editor's tabs on mount; returns the unregister
   *  function the editor calls on unmount. */
  registerSession: (session: EditorSession) => () => void;
  /** The last tab was closed, or the first document could not be opened
   *  (`error` names why): the shell shows the start screen (document-tabs
   *  AC-09, edge cases). */
  onEmpty: (error?: string) => void;
  /** Re-open a recent through the shell, which reads the file (FSA handle
   *  or Tauri path), runs the unsaved-changes guard and routes it to the
   *  mode the format belongs to. */
  onOpenRecent: (recent: RecentFile) => void;
  /** Native-menu (Tauri) items the editor serves; `null` on unmount. */
  registerMenuHandlers: (handlers: EditorMenuHandlers | null) => void;
}
