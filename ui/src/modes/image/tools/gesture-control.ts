/**
 * Cancels the in-flight gesture without committing (Escape, a tab switch).
 * Wired up by `attachTools`; a no-op while no canvas is mounted. Answers
 * whether a gesture was in progress, so Escape deselects only when none
 * was. A module of its own so the tab store can use it without importing
 * the pointer code (which imports the controller, which imports the tabs).
 */
export const gestureControl = {
  cancel: (): boolean => false,
  /** Drops a live-object handle drag in progress (Undo during the drag,
   *  docs/specs/image-live-objects-ui.md AC-11); `false` when none is. */
  cancelLiveDrag: (): boolean => false,
  /** Commits the active document's live object, if any (tab switch, save;
   *  docs/specs/image-live-objects-ui.md AC-09, AC-15). Set by `core/live`. */
  commitLive: (): void => {},
};
