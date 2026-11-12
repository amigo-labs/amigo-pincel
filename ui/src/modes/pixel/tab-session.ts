// What each Pixel-mode tab carries beside its document state
// (docs/specs/document-tabs.md): its autosave loop (AC-14), its running
// File ▸ Save (AC-17), and its registration with the shell's
// unsaved-changes prompt (AC-08, AC-10, AC-11, AC-13). Closing a tab asks
// about its unsaved changes first, then frees it; closing the last one
// releases the editor (AC-09, AC-16).
import { writeTarget } from '../../lib/fs';
import { t } from '../../lib/i18n/index.svelte';
import { AutosaveSession } from '../../lib/idb/autosave-session.svelte';
import { isIdbAvailable } from '../../lib/idb/db';
import { upsertRecent } from '../../lib/idb/recent-files';
import { listPresentRecents } from '../../lib/fs/recents';
import { hasFsAccess } from '../../lib/fs';
import { isTauri } from '../../lib/platform';
import type { EditorDocument } from '../../lib/shell/types';
import { agentFontsJson } from '../../lib/agent/fonts';
import { registerFont } from '../../lib/core';
import { generateForAgent } from '../../lib/comfyui/generation.svelte';
import { runAgentCall, runAssistantEnd } from './doc-ops';
import { pixelTarget } from './generate-target';
import { ed, file, tabs, type PixelTab } from './stores/editor.svelte';
import { activateTab, closeAllTabs, closeTab } from './tabs.svelte';

/** The shell callbacks the tabs need (`EditorProps`). */
export interface TabHooks {
  registerDocument: (doc: EditorDocument) => () => void;
  confirmClose: (doc: EditorDocument, close: () => void) => void;
  activateDocument: (doc: EditorDocument) => void;
  reportError: (doc: EditorDocument, error: string) => void;
  onEmpty: () => void;
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
};

/** Recents need something a re-open can use: an FSA handle (web) or a
 *  path (Tauri), and the IDB registry. */
export const recentsAvailable = (hasFsAccess() || isTauri()) && isIdbAvailable();

/** Hand the tabs the shell's callbacks (editor mount). */
export function bindTabHooks(next: TabHooks): void {
  hooks = next;
}

/** "Modified since last save": the tab's undo depth moved off the saved one. */
export function isTabDirty(tab: PixelTab | null = tabs.active): boolean {
  return tab !== null && tab.doc.doc !== null && tab.doc.undoDepth !== tab.doc.lastSavedUndoDepth;
}

/** The File ▸ Save / Save As of `tab` running now, or `null`. */
export function runningSave(tab: PixelTab | null = tabs.active): Promise<boolean> | null {
  return tab === null ? null : (extras.get(tab.id)?.running ?? null);
}

/** Give a freshly opened tab its autosave loop and register it with the
 *  shell. `docId` continues an identity (a recent's id, a recovered
 *  snapshot's); a recovered tab keeps its snapshot until it is saved. */
export function attachTab(tab: PixelTab, opts: { docId?: string; recovered?: boolean } = {}): void {
  const autosave = new AutosaveSession(
    {
      mode: 'pixel',
      depth: () => tab.doc.doc?.undoDepth ?? null,
      savedDepth: () => tab.doc.lastSavedUndoDepth,
      busy: () => file.busy || runningSave(tab) !== null,
      name: () => tab.file.saveTarget.name,
      encode: () => (tab.doc.doc ? new Uint8Array(tab.doc.doc.saveAseprite()) : null),
      report: (error) => {
        ed.status = t('status.failed', { action: t('act.autosave'), error });
      },
    },
    opts.docId,
  );
  const doc: EditorDocument = {
    name: () => tab.file.saveTarget.name,
    dirty: () => isTabDirty(tab),
    location: () => tab.file.saveTarget,
    save: () => saveTab(tab),
    running: () => runningSave(tab),
    discard: () => autosave.finish(),
    activate: () => activateTab(tab),
    close: () => requestCloseTab(tab),
    saveAs: () => saveTab(tab, true),
    agentCall: (docId, name, argsJson, file, fonts) => {
      const fontsJson = fonts ? agentFontsJson(fonts, (b, i) => registerFont(b, i)) : null;
      // The shell activates the tab before a call that may change it;
      // `doc.list` reads background tabs in place, without switching the
      // editor behind the shell's back. Their view refreshes when shown.
      if (tabs.active === tab) return runAgentCall(docId, name, argsJson, file, fontsJson);
      const d = tab.doc.doc;
      if (d === null) return JSON.stringify({ ok: false, error: 'no document is open' });
      return d.agentCall(docId, name, argsJson, file?.path, file?.bytes, fontsJson);
    },
    assistantBegin: () => tab.doc.doc?.assistantBegin(),
    assistantEnd: () => {
      if (tabs.active === tab) return runAssistantEnd();
      return tab.doc.doc?.assistantEnd() ?? false;
    },
    undoKinds: () => tab.doc.doc?.undoKinds() ?? [],
    generateImage: (args, models, signal) => generateForAgent(pixelTarget(tab), args, models, signal),
  };
  extras.set(tab.id, { doc, autosave, running: null, unregister: hooks.registerDocument(doc) });
  autosave.start();
  if (!opts.recovered) void autosave.opened();
}

/** Make `tab` the active one through the shell, which shows this editor
 *  (the file asked for is already open there, document-tabs AC-03). */
export function focusTab(tab: PixelTab): void {
  const ex = extras.get(tab.id);
  if (ex) hooks.activateDocument(ex.doc);
  else activateTab(tab);
}

/** Saves `tab`; resolves `true` once it is saved, `false` when the picker
 *  was dismissed (no message, unsaved-changes-prompt AC-12) or the save
 *  failed (reported in the status bar). One save per tab at a time; the
 *  tab is fixed when the save starts, so switching tabs meanwhile does not
 *  change what is written (document-tabs AC-17). */
export function saveTab(tab: PixelTab | null = tabs.active, forceAs = false): Promise<boolean> {
  const ex = tab === null ? undefined : extras.get(tab.id);
  if (tab === null || ex === undefined || ex.running !== null || tab.doc.doc === null || file.busy) {
    return Promise.resolve(false);
  }
  const run = saveInner(tab, ex, forceAs).finally(() => {
    ex.running = null;
  });
  ex.running = run;
  return run;
}

async function saveInner(tab: PixelTab, ex: TabExtras, forceAs: boolean): Promise<boolean> {
  const doc = tab.doc.doc;
  if (doc === null) return false;
  try {
    const bytes = new Uint8Array(doc.saveAseprite());
    const depth = doc.undoDepth;
    const next = await writeTarget(bytes, tab.file.saveTarget, { forceAs });
    if (next === null) return false;
    tab.file.saveTarget = next;
    tab.doc.lastSavedUndoDepth = depth;
    ed.status = t('px.saved', { bytes: bytes.length, name: next.name });
    await ex.autosave.saved();
    await recordRecent(tab);
    return true;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.save'), error: msg });
    // A tab saved in the background names its failure at once (AC-25).
    hooks.reportError(ex.doc, msg);
    return false;
  }
}

// Persist the tab's save target to the recent-files registry and refresh
// the in-memory list. No-op when the registry isn't available (non-FSA
// browsers / IDB disabled) or the target doesn't carry a handle or path (a
// download-only save, or a New document that hasn't been written yet).
export async function recordRecent(tab: PixelTab | null = tabs.active): Promise<void> {
  const ex = tab === null ? undefined : extras.get(tab.id);
  if (!recentsAvailable || tab === null || ex === undefined) return;
  const target = tab.file.saveTarget;
  if (!target.handle && !target.path) return;
  try {
    await upsertRecent({
      id: ex.autosave.docId,
      name: target.name,
      handle: target.handle,
      path: target.path,
      mode: 'pixel',
    });
    file.recents = await listPresentRecents();
  } catch (err) {
    // Persistence is best-effort; surface to the status line but don't
    // block the save / open the user already completed.
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('px.recentsFailed', { status: ed.status, error: msg });
  }
}

/** Close `tab` the way its close button does (document-tabs AC-07, AC-08):
 *  nothing while its save runs (AC-17), the unsaved-changes prompt first
 *  when it has unsaved changes. */
export function requestCloseTab(tab: PixelTab | null = tabs.active): void {
  const ex = tab === null ? undefined : extras.get(tab.id);
  if (tab === null || ex === undefined || ex.running !== null) return;
  hooks.confirmClose(ex.doc, () => closeNow(tab));
}

function closeNow(tab: PixelTab): void {
  const ex = extras.get(tab.id);
  if (ex === undefined) return;
  extras.delete(tab.id);
  ex.unregister();
  ex.autosave.close();
  if (closeTab(tab) === null) hooks.onEmpty();
}

/** The editor unmounts: every tab is unregistered, its autosave stopped
 *  (its snapshot dropped) and its document freed (AC-16). */
export function detachAllTabs(): void {
  for (const ex of extras.values()) {
    ex.unregister();
    ex.autosave.close();
  }
  extras.clear();
  closeAllTabs();
}
