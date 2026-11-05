// Contract between the app shell (src/App.svelte) and the two editors
// (src/modes/pixel/PixelEditor.svelte, src/modes/image/ImageEditor.svelte).
// See docs/specs/pincel.md §1.1 "Two modes" and §9.2.
import type { TabBarProps } from '../components/layout/TabBar.svelte';
import type { FileLocation, OpenedFile } from '../fs';
import type { AutosaveSnapshotMeta } from '../idb/autosave';
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

/** One open document (a tab) of an editor, as the shell's tab bar, its
 *  unsaved-changes prompt and the desktop close guard see it
 *  (docs/specs/unsaved-changes-prompt.md, docs/specs/document-tabs.md). */
export interface EditorDocument {
  /** The name the top bar shows, with its extension. */
  name(): string;
  /** The document has unsaved changes (the top bar's unsaved marker). */
  dirty(): boolean;
  /** Where the document is saved (FSA handle or desktop path), for the
   *  same-file check (document-tabs AC-03); both `null` without a file. */
  location(): FileLocation;
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
  /** Make this document its editor's active tab. The shell calls it when
   *  it activates the tab (`activateDocument` makes the shell do so). */
  activate(): void;
  /** Close the tab the way its close button does: nothing while its save
   *  runs, the unsaved-changes prompt first when it has unsaved changes
   *  (document-tabs AC-07, AC-08, AC-17). */
  close(): void;
}

/** What an editor does with the documents the shell routes to it
 *  (docs/specs/document-tabs.md AC-02, AC-03, AC-15). Each resolves once
 *  the document is open as a new, active tab — or the tab holding the
 *  same file is active. */
export interface EditorSession {
  /** Open an already-read file; resolves `null` once open, or the error
   *  that kept it from opening (no tab opens, the active tab stays). */
  open(file: OpenedFile): Promise<string | null>;
  /** Create a blank document; resolves like `open`. */
  create(params: NewDocParams): Promise<string | null>;
  /** Open a recovered autosave snapshot as an unsaved tab that continues
   *  under `meta.docId`; rejects when it could not be opened. */
  recover(bytes: Uint8Array, meta: AutosaveSnapshotMeta): Promise<void>;
}

/** Props every editor component accepts. The shell mounts at most one
 *  editor per mode, when the first document of that mode opens, and
 *  releases it with its last tab (document-tabs AC-16, AC-22). Documents
 *  arrive through the registered `EditorSession`. */
export interface EditorProps {
  /** The active tab is this editor's: it is shown and receives keyboard
   *  shortcuts, menu commands, pointer input and clipboard events. While
   *  `false` the editor stays mounted, hidden, and reacts to none of them
   *  (document-tabs AC-21). */
  active: boolean;
  /** The shell's tab bar (tabs of both modes), drawn by the editor frame. */
  tabBar: TabBarProps;
  /** Next / previous tab shortcut: the shell steps across both modes. */
  onStepTab: (step: 1 | -1) => void;
  /** The editor's "New" affordance: the shell owns the New dialog. */
  onRequestNew: () => void;
  /** The editor's "Open…" affordance: the shell picks the file and routes
   *  it to the right mode. (Pixel mode keeps its own Open for sprites and
   *  PNGs and only hands foreign formats over via `onOpenForeign`.) */
  onRequestOpen: () => void;
  /** The editor read a file that belongs to the other mode; the shell
   *  routes it by its format. */
  onOpenForeign: (file: OpenedFile) => void;
  /** Open a document the editor read or was handed (its own Open…, Paste
   *  as New Image): the shell activates a tab holding the same file, else
   *  opens it as a tab of `mode` (default: the mode its format belongs
   *  to). */
  onOpenFile: (file: OpenedFile, mode?: EditorMode) => void;
  /** Run `close` (closing `doc`'s tab) once the shell's unsaved-changes
   *  prompt about that one document allows it (document-tabs AC-08). */
  confirmClose: (doc: EditorDocument, close: () => void) => void;
  /** Hand the shell a newly opened tab; it goes after the last tab and
   *  becomes active. Returns the function the editor calls when the tab
   *  closes or the editor unmounts. */
  registerDocument: (doc: EditorDocument) => () => void;
  /** The editor activated one of its tabs itself (the file it was asked to
   *  open is already open there): the shell makes that tab active. */
  activateDocument: (doc: EditorDocument) => void;
  /** A tab's save failed: the shell shows `message` as a notice when that
   *  tab is not the active one (document-tabs AC-25). */
  reportError: (doc: EditorDocument, message: string) => void;
  /** Hand the shell the editor's session on mount; returns the unregister
   *  function the editor calls on unmount. */
  registerSession: (session: EditorSession) => () => void;
  /** The editor's last tab was closed: the shell releases the editor, and
   *  shows the start screen when no tab of either mode is left
   *  (document-tabs AC-09). */
  onEmpty: () => void;
  /** Re-open a recent through the shell, which reads the file (FSA handle
   *  or Tauri path) and routes it to the mode the format belongs to. */
  onOpenRecent: (recent: RecentFile) => void;
  /** Native-menu (Tauri) items the editor serves; `null` on unmount. The
   *  shell forwards the menu only to the active tab's editor. */
  registerMenuHandlers: (handlers: EditorMenuHandlers | null) => void;
}
