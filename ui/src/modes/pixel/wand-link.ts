// The Magic Wand link (docs/specs/pixel-selection-move.md AC-25, AC-26): after
// a wand click that changed the selection, committing a new Tolerance,
// Contiguous or Sample setting recomputes that click — same point, same
// combine mode, combined with the selection from before the click. Selection
// changes stay outside the undo stack (AC-13), so the link keeps the
// selection from before the click and restores it before recomputing.
//
// The link holds a token of the document state right after its click. Any
// other change (pixels, layers, canvas, undo / redo, frame, active layer,
// another selection change, another document) makes the token stale, and a
// stale link is dropped on its next use. Choosing another tool drops it at
// once (`dropWandLink`).

import type { Document } from '../../lib/core';
import type { SelectionMode } from './stores/editor.svelte';
import { ed, toolState } from './stores/editor.svelte';

interface WandLink {
  readonly doc: Document;
  readonly x: number;
  readonly y: number;
  readonly mode: SelectionMode;
  /** The selection before the click (empty: none). */
  readonly before: Uint8Array;
  /** The selection the link last produced, and the state it left. */
  after: Uint8Array;
  undo: number;
  redo: number;
  rev: number;
  readonly frame: number;
  readonly layer: number | null;
}

let link: WandLink | null = null;

/** The tolerance as a wand click uses it, or null for an empty / non-numeric field. */
function tolerance(): number | null {
  const v: unknown = toolState.wandTolerance;
  if (typeof v !== 'number' || !Number.isFinite(v)) return null;
  return Math.max(0, Math.min(255, Math.round(v)));
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

function wand(doc: Document, x: number, y: number, tol: number, mode: SelectionMode): void {
  doc.selectWand(x, y, tol, toolState.wandContiguous, toolState.wandSample, mode);
}

function stamp(l: WandLink, doc: Document): void {
  l.after = doc.selectionMask();
  l.undo = doc.undoDepth;
  l.redo = doc.redoDepth;
  l.rev = ed.docRev;
}

/** True while nothing but this link changed the document since its click. */
function current(l: WandLink): boolean {
  const doc = ed.doc;
  return (
    doc === l.doc &&
    doc.undoDepth === l.undo &&
    doc.redoDepth === l.redo &&
    ed.docRev === l.rev &&
    ed.currentFrame === l.frame &&
    ed.activeLayerId === l.layer &&
    sameBytes(doc.selectionMask(), l.after)
  );
}

/**
 * A Magic Wand click at sprite `(x, y)`: applies the wand and starts a link
 * when the selection changed (AC-26); a click that changes nothing drops any
 * link. Throws what `selectWand` throws.
 */
export function wandClick(doc: Document, x: number, y: number, mode: SelectionMode): void {
  const before = doc.selectionMask();
  wand(doc, x, y, tolerance() ?? 0, mode);
  const after = doc.selectionMask();
  if (sameBytes(before, after)) {
    link = null;
    return;
  }
  const next: WandLink = {
    doc,
    x,
    y,
    mode,
    before,
    after,
    undo: 0,
    redo: 0,
    rev: 0,
    frame: ed.currentFrame,
    layer: ed.activeLayerId,
  };
  stamp(next, doc);
  link = next;
}

/**
 * The Magic Wand settings were committed (AC-25): recompute the linked click
 * with them. Without a current link, or with an empty / non-numeric
 * tolerance, nothing changes.
 */
export function relinkWand(): void {
  const l = link;
  const doc = ed.doc;
  if (l === null || doc === null) return;
  if (!current(l)) {
    link = null;
    return;
  }
  const tol = tolerance();
  if (tol === null) return;
  doc.setSelectionMask(l.before);
  wand(doc, l.x, l.y, tol, l.mode);
  stamp(l, doc);
}

/** Drops the link (another tool was chosen, AC-26). */
export function dropWandLink(): void {
  link = null;
}
