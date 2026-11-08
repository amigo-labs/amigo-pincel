// History panel rows (docs/specs/history-panel.md): the start entry and one
// name per step kind the cores report. Rune-free, so a Node-side Playwright
// spec can test it without the reactive i18n module.

import { en, type MessageKey } from '../i18n/en';

/** How a document began: names the start entry (AC-03). */
export type HistoryOrigin = 'new' | 'open' | 'paste' | 'recovered' | 'start';

/** What the History panel shows of a document's undo history. */
export interface HistorySnapshot {
  /** Kind ids of the undo steps, oldest first. */
  readonly undo: readonly string[];
  /** Kind ids of the redo steps, in the order Redo restores them. */
  readonly redo: readonly string[];
  /** A step was dropped from the bottom (step limit, memory budget). */
  readonly dropped: boolean;
}

/** An empty history: a document nothing was done to. */
export const EMPTY_HISTORY: HistorySnapshot = { undo: [], redo: [], dropped: false };

/** Translates a message key with optional `{name}` parameters. */
export type Translate = (key: MessageKey, params?: Record<string, string | number>) => string;

function isKey(key: string): key is MessageKey {
  return key in en;
}

/** The name of a step of kind `kind` (AC-04): `history.<kind>`, or the effect
 *  menu's name for `effect.<name>`; "Unknown step <kind>" for a kind the UI
 *  has no name for (AC-17). */
export function historyStepName(kind: string, tr: Translate): string {
  const key = kind.startsWith('effect.') ? kind : `history.${kind}`;
  return isKey(key) ? tr(key) : tr('historyPanel.unknown', { kind });
}

/** The start entry's name (AC-03): how the document began, or "Oldest
 *  available state" once a step was dropped. */
export function historyStartName(origin: HistoryOrigin, dropped: boolean, tr: Translate): string {
  if (dropped) return tr('historyPanel.start.oldest');
  return tr(`historyPanel.start.${origin}`);
}

/** `true` when two snapshots list the same steps. */
export function sameHistory(a: HistorySnapshot, b: HistorySnapshot): boolean {
  return (
    a.dropped === b.dropped &&
    a.undo.length === b.undo.length &&
    a.redo.length === b.redo.length &&
    a.undo.every((k, i) => k === b.undo[i]) &&
    a.redo.every((k, i) => k === b.redo[i])
  );
}
