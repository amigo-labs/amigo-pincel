// The Magic Wand link (docs/specs/image-editing.md AC-33, AC-34): after a
// wand click that changed the selection, every change of Tolerance,
// Contiguous or Sample recomputes that click — same point, same combine mode,
// combined with the selection from before the click. In Image mode the click
// is an undoable command, so a recompute undoes the linked command (back to
// the selection before the click) and applies the wand again with the new
// settings: the click and all its updates stay one undo step, and Redo after
// an Undo restores the selection of the last settings.
//
// The link keeps the document handle and `editor.revision` its click (or last
// recompute) left. Every command, Undo, Redo and layer switch bumps the
// revision (`syncInfo`), so any other change makes the link stale, and a stale
// link is dropped on its next use. Choosing another tool drops it at once
// (`dropWandLink`).

import type { SelectionMode } from '../core/wasm';
import { deselect, selectWand, selectionMask, undo } from '../core/controller';
import { editor } from '../stores/editor.svelte';

interface WandLink {
  readonly handle: number;
  /** Canvas point of the click; null for a click off the canvas. */
  readonly point: readonly [number, number] | null;
  readonly mode: SelectionMode;
  revision: number;
}

let link: WandLink | null = null;

function sameMask(a: Uint8ClampedArray | null, b: Uint8ClampedArray | null): boolean {
  if (a === null || b === null) return a === b;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function apply(point: readonly [number, number] | null, mode: SelectionMode): void {
  if (point) selectWand(point[0], point[1], mode);
  else deselect();
}

/**
 * A Magic Wand click: on the canvas at `point`, or off it (`null`), where
 * replace mode clears a present selection (AC-30) and other modes do nothing.
 * Starts a link when the selection changed (AC-34).
 */
export function wandClick(point: readonly [number, number] | null, mode: SelectionMode): void {
  const handle = editor.handle;
  if (handle === null) return;
  if (point === null && !(mode === 'replace' && editor.hasSelection)) {
    link = null;
    return;
  }
  const before = selectionMask();
  apply(point, mode);
  link = sameMask(before, selectionMask())
    ? null
    : { handle, point, mode, revision: editor.revision };
}

/** The Magic Wand settings changed (AC-33): recompute the linked click. */
export function relinkWand(): void {
  const l = link;
  if (l === null) return;
  if (editor.handle !== l.handle || editor.revision !== l.revision) {
    link = null;
    return;
  }
  undo();
  apply(l.point, l.mode);
  l.revision = editor.revision;
}

/** Drops the link (another tool was chosen, AC-34). */
export function dropWandLink(): void {
  link = null;
}
