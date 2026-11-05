// What each Image-mode tab carries beside its document state
// (docs/specs/document-tabs.md): its autosave loop (AC-14), its running
// File ▸ Save (AC-17) and its registration with the shell's
// unsaved-changes prompt (AC-08, AC-10, AC-11, AC-13). Closing a tab asks
// about its unsaved changes first, then frees it; closing the last one
// releases the editor (AC-09, AC-16).
import { AutosaveSession } from '../../../lib/idb/autosave-session.svelte';
import type { SaveTarget } from '../../../lib/fs';
import type { EditorDocument } from '../../../lib/shell/types';
import { t } from '../../../lib/i18n/index.svelte';
import { tabs, type ImageTab } from '../stores/editor.svelte';
import { fileFormat, fileName, fileTarget } from './document-file';
import { saveDocument } from './save-flow';
import { activateTab, closeAllTabs, closeTab } from './tabs.svelte';
import { core } from './wasm';
import { gestureControl } from '../tools/gesture-control';

/** The shell and editor callbacks the tabs need (`EditorProps`). */
export interface TabHooks {
  registerDocument: (doc: EditorDocument) => () => void;
  confirmClose: (doc: EditorDocument, close: () => void) => void;
  activateDocument: (doc: EditorDocument) => void;
  reportError: (doc: EditorDocument, error: string) => void;
  onEmpty: () => void;
  /** A save finished: record the file as a recent under `docId`. */
  recordRecent: (target: SaveTarget, docId: string) => Promise<void>;
  /** Show an error in the status bar. */
  report: (message: string) => void;
}

interface TabExtras {
  doc: EditorDocument;
  autosave: AutosaveSession;
  running: Promise<boolean> | null;
  unregister: () => void;
}

const extras = new Map<number, TabExtras>();

let hooks: TabHooks = {
  registerDocument: () => () => {},
  confirmClose: (_doc, close) => close(),
  activateDocument: (doc) => doc.activate(),
  reportError: () => {},
  onEmpty: () => {},
  recordRecent: async () => {},
  report: () => {},
};

/** Hand the tabs the editor's callbacks (editor mount). */
export function bindTabHooks(next: TabHooks): void {
  hooks = next;
}

/** The tab has unsaved changes (the unsaved marker, app-shell AC-16). */
export function isDirty(tab: ImageTab | null = tabs.active): boolean {
  return tab !== null && tab.doc.handle !== null && tab.doc.undoDepth !== tab.doc.savedUndoDepth;
}

/** The File ▸ Save / Save As of `tab` running now, or `null`. */
export function runningSave(tab: ImageTab | null = tabs.active): Promise<boolean> | null {
  return tab === null ? null : (extras.get(tab.id)?.running ?? null);
}

/** The autosave identity of `tab` (recents are keyed by it). */
export function docIdOf(tab: ImageTab): string | null {
  return extras.get(tab.id)?.autosave.docId ?? null;
}

/** Give a freshly opened tab its autosave loop and register it with the
 *  shell. `docId` continues an identity (a recovered snapshot's); a
 *  recovered tab keeps its snapshot until it is saved. */
export function attachTab(tab: ImageTab, opts: { docId?: string; recovered?: boolean } = {}): void {
  const autosave = new AutosaveSession(
    {
      mode: 'image',
      depth: () => (tab.doc.handle === null ? null : tab.doc.undoDepth),
      savedDepth: () => tab.doc.savedUndoDepth,
      busy: () => runningSave(tab) !== null,
      name: () => fileName(tab),
      format: () => fileFormat(tab),
      encode: () => (tab.doc.handle === null ? null : core.saveProject(tab.doc.handle, 1)),
      report: (error) => hooks.report(t('img.autosaveFailed', { error })),
    },
    opts.docId,
  );
  const doc: EditorDocument = {
    name: () => tab.doc.fileName,
    dirty: () => isDirty(tab),
    location: () => fileTarget(tab),
    save: () => saveTab(tab),
    running: () => runningSave(tab),
    discard: () => autosave.finish(),
    // `activateTab` drops the gesture in flight of the tab being left.
    activate: () => activateTab(tab),
    close: () => requestCloseTab(tab),
  };
  extras.set(tab.id, { doc, autosave, running: null, unregister: hooks.registerDocument(doc) });
  autosave.start();
  if (!opts.recovered) void autosave.opened();
}

/** Make `tab` the active one through the shell, which shows this editor
 *  (the file asked for is already open there, document-tabs AC-03). */
export function focusTab(tab: ImageTab): void {
  const ex = extras.get(tab.id);
  if (ex) hooks.activateDocument(ex.doc);
  else activateTab(tab);
}

/** Saves `tab` (Save, or Save As with `forceAs`); resolves `true` once it
 *  is saved, `false` when a prompt or the picker was cancelled or the save
 *  failed (reported). One save per tab at a time. */
export function saveTab(tab: ImageTab | null = tabs.active, forceAs = false): Promise<boolean> {
  const ex = tab === null ? undefined : extras.get(tab.id);
  if (tab === null || ex === undefined || ex.running !== null || tab.doc.handle === null) {
    return Promise.resolve(false);
  }
  // Save commits the live object first, so the file has it (AC-09, AC-15).
  if (tab === tabs.active) gestureControl.commitLive();
  const run = saveInner(tab, ex, forceAs).finally(() => {
    ex.running = null;
  });
  ex.running = run;
  return run;
}

async function saveInner(tab: ImageTab, ex: TabExtras, forceAs: boolean): Promise<boolean> {
  try {
    const result = await saveDocument(forceAs, tab);
    if (result === null) return false;
    // A flattened file lost the layers: the snapshot keeps them (AC-13).
    await (result.flattened ? ex.autosave.keep() : ex.autosave.saved());
    await hooks.recordRecent(result.target, ex.autosave.docId);
    return true;
  } catch (e) {
    hooks.report(t('img.saveFailed', { error: String(e) }));
    // A tab saved in the background names its failure at once (AC-25).
    hooks.reportError(ex.doc, e instanceof Error ? e.message : String(e));
    return false;
  }
}

/** Close `tab` the way its close button does (document-tabs AC-07, AC-08):
 *  nothing while its save runs (AC-17), the unsaved-changes prompt first
 *  when it has unsaved changes. */
export function requestCloseTab(tab: ImageTab | null = tabs.active): void {
  const ex = tab === null ? undefined : extras.get(tab.id);
  if (tab === null || ex === undefined || ex.running !== null) return;
  hooks.confirmClose(ex.doc, () => closeNow(tab));
}

function closeNow(tab: ImageTab): void {
  const ex = extras.get(tab.id);
  if (ex === undefined) return;
  extras.delete(tab.id);
  ex.unregister();
  ex.autosave.close();
  if (closeTab(tab) === null) hooks.onEmpty();
}

/** The editor unmounts: every tab is unregistered, its autosave stopped
 *  (its snapshot dropped) and its core document freed (AC-16). */
export function detachAllTabs(): void {
  for (const ex of extras.values()) {
    ex.unregister();
    ex.autosave.close();
  }
  extras.clear();
  closeAllTabs();
}
