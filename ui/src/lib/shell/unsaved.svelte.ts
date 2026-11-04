// The one unsaved-changes prompt (docs/specs/unsaved-changes-prompt.md).
// Every action that would replace the open document — and closing the
// desktop app — goes through `guardUnsaved`: with unsaved changes it asks
// Save / Don't Save / Cancel first. While the prompt (or a save it started)
// is open, or a File ▸ Save runs, further actions are ignored (AC-08).
import type { EditorDocument } from './types';

/** What the prompt shows; `null` while it is closed. */
export const unsaved = $state({
  prompt: null as { name: string } | null,
});

/** The user's answer to the prompt. */
export type UnsavedAnswer = 'save' | 'dontSave' | 'cancel';

let current: EditorDocument | null = null;
/** The action awaiting an answer; kept while a save from the prompt runs. */
let pending: (() => void) | null = null;

/** The mounted editor hands over its document; the returned function
 *  unregisters it (a no-op once another editor registered). */
export function registerDocument(doc: EditorDocument): () => void {
  current = doc;
  return () => {
    if (current === doc) current = null;
  };
}

/** The open document has unsaved changes. */
export function hasUnsaved(): boolean {
  return current?.dirty() ?? false;
}

/** The File ▸ Save running now, or `null`. */
export function runningSave(): Promise<boolean> | null {
  return current?.running() ?? null;
}

/** The prompt is open, or a save it started still runs. */
export function promptBusy(): boolean {
  return pending !== null;
}

/** Drop the open document's autosave snapshot (closing without saving). */
export async function discardDocument(): Promise<void> {
  await current?.discard();
}

/** Run `action` at once when nothing is unsaved, otherwise once the prompt
 *  allows it. Ignored while the prompt is busy or a save runs (AC-08). */
export function guardUnsaved(action: () => void): void {
  if (promptBusy() || runningSave() !== null) return;
  if (current === null || !current.dirty()) {
    action();
    return;
  }
  pending = action;
  unsaved.prompt = { name: current.name() };
}

/** Answer the open prompt: Save runs the action only once the save
 *  succeeded (AC-03, AC-04), Don't Save runs it as is (AC-05), Cancel
 *  drops it (AC-06). */
export async function answerUnsaved(answer: UnsavedAnswer): Promise<void> {
  const action = pending;
  if (action === null || unsaved.prompt === null) return;
  unsaved.prompt = null;
  let proceed = answer === 'dontSave';
  if (answer === 'save') {
    try {
      proceed = (await current?.save()) ?? false;
    } catch (err) {
      console.error('save from the unsaved-changes prompt failed', err);
      proceed = false;
    }
  }
  pending = null;
  if (proceed) action();
}
