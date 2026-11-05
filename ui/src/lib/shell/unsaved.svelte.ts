// The one unsaved-changes prompt (docs/specs/unsaved-changes-prompt.md,
// extended to several tabs by docs/specs/document-tabs.md). Opening a
// document never replaces another (tabs of both modes), so the prompt
// guards closing: the desktop app goes through `guardUnsaved`, one tab
// through `guardDocument`. With one unsaved document the prompt asks Save /
// Don't Save / Cancel about it, with several it lists them and offers
// Save All / Don't Save / Cancel. While the prompt (or a save it started)
// is open, further actions are ignored (unsaved-changes-prompt AC-08); a
// session-wide action waits for running saves first (document-tabs AC-17).
import { activateDocument, openDocuments } from './session.svelte';
import type { EditorDocument } from './types';

/** What the prompt shows; `null` while it is closed. One name asks about
 *  that document, several list them all. */
export const unsaved = $state({
  prompt: null as { names: string[] } | null,
});

/** The user's answer to the prompt (`save` is Save All for a list). */
export type UnsavedAnswer = 'save' | 'dontSave' | 'cancel';

/** The question awaiting an answer; kept while a save from it runs. */
let pending: { action: () => void; docs: EditorDocument[] } | null = null;
/** A session-wide action is waiting for running saves to finish. */
let waiting = false;

/** Some open document has unsaved changes (app-shell AC-15, the desktop
 *  close prompt; document-tabs AC-13). */
export function hasUnsaved(): boolean {
  return openDocuments().some((d) => d.dirty());
}

/** The File ▸ Save / Save As runs that are running now. */
function runningSaves(): Promise<boolean>[] {
  return openDocuments()
    .map((d) => d.running()).filter((p): p is Promise<boolean> => p !== null);
}

/** The prompt is open, a save it started still runs, or an action waits
 *  for a running save. */
export function promptBusy(): boolean {
  return pending !== null || waiting;
}

/** Drop every open document's autosave snapshot (closing without saving). */
export async function discardDocuments(): Promise<void> {
  await Promise.all(openDocuments().map((d) => d.discard()));
}

function ask(action: () => void, dirty: EditorDocument[]): void {
  pending = { action, docs: dirty };
  const only = dirty.length === 1 ? dirty[0] : undefined;
  if (only) activateDocument(only);
  unsaved.prompt = { names: dirty.map((d) => d.name()) };
}

/** Run `action` — it closes the app — at once when nothing is unsaved, otherwise once the prompt allows it.
 *  Running saves are awaited first (document-tabs AC-17). Ignored while
 *  the prompt is busy (unsaved-changes-prompt AC-08). */
export async function guardUnsaved(action: () => void): Promise<void> {
  if (promptBusy()) return;
  const running = runningSaves();
  if (running.length > 0) {
    waiting = true;
    try {
      await Promise.allSettled(running);
    } finally {
      waiting = false;
    }
    // A save started meanwhile is waited for too.
    if (runningSaves().length > 0) return guardUnsaved(action);
  }
  const dirty = openDocuments().filter((d) => d.dirty());
  if (dirty.length === 0) {
    action();
    return;
  }
  ask(action, dirty);
}

/** Run `close` (closing `doc`'s tab) at once when `doc` is clean,
 *  otherwise once the prompt about `doc` allows it (document-tabs AC-08).
 *  Ignored while the prompt is busy or `doc` is being saved (AC-17). */
export function guardDocument(doc: EditorDocument, close: () => void): void {
  if (promptBusy() || doc.running() !== null) return;
  if (!doc.dirty()) {
    close();
    return;
  }
  ask(close, [doc]);
}

/** Save the listed documents one after another in tab order, each tab
 *  active while it saves; stops at the first one that is cancelled or
 *  fails (document-tabs AC-11). */
async function saveAll(list: EditorDocument[]): Promise<boolean> {
  for (const doc of list) {
    if (!openDocuments().includes(doc) || !doc.dirty()) continue;
    activateDocument(doc);
    let ok: boolean;
    try {
      ok = await doc.save();
    } catch (err) {
      console.error('save from the unsaved-changes prompt failed', err);
      ok = false;
    }
    if (!ok) return false;
  }
  return true;
}

/** Answer the open prompt: Save runs the action only once every save
 *  succeeded (unsaved-changes-prompt AC-03, AC-04; document-tabs AC-11),
 *  Don't Save runs it as is (AC-05), Cancel drops it (AC-06). */
export async function answerUnsaved(answer: UnsavedAnswer): Promise<void> {
  const question = pending;
  if (question === null || unsaved.prompt === null) return;
  unsaved.prompt = null;
  const proceed = answer === 'dontSave' || (answer === 'save' && (await saveAll(question.docs)));
  pending = null;
  if (proceed) question.action();
}
