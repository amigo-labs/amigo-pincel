// History panel state of the active Pixel tab (docs/specs/history-panel.md).
import { sameHistory } from '../../lib/history/names';
import { ed } from './stores/editor.svelte';

/** Re-reads the step list for the History panel when it changed: called
 *  with every recompose, so the panel moves with the canvas
 *  (history-panel AC-08). A stroke's merges leave the revision alone. */
export function syncHistory() {
  const doc = ed.doc;
  if (!doc || doc.historyRevision === ed.historyRev) return;
  ed.historyRev = doc.historyRevision;
  const next = { undo: doc.undoKinds(), redo: doc.redoKinds(), dropped: doc.historyDropped };
  if (!sameHistory(next, ed.history)) ed.history = next;
}
