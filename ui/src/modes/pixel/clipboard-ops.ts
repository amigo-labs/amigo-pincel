// Edit ▸ Cut / Copy / Copy Merged / Paste (spec §5.4, Decision Log
// 2026-10-29). The core turns pixels into PNG bytes and back; the shared
// `lib/clipboard` writes the system clipboard and keeps the in-app record.

import { Document } from '../../lib/core';
import {
  clipboardImage,
  docTag,
  imageSize,
  newImageBytes,
  pasteEventImage,
  pastePosition,
  pastedImageFile,
  resolvePaste,
  storeCopy,
} from '../../lib/clipboard';
import { openInShell } from './file-ops';
import { isTypingTarget } from '../../lib/shell/shortcuts';
import { t } from '../../lib/i18n/index.svelte';
import { MAX_DOC_SIZE, dialogs, ed, live, view } from './stores/editor.svelte';
import { syncMeta, syncSelection } from './render';

/** A pasted image larger than the canvas, awaiting Expand / Keep / Cancel. */
export interface PasteDialogState {
  doc: Document;
  bytes: Uint8Array;
  x: number;
  y: number;
  width: number;
  height: number;
  /** The canvas can grow to cover the image (Pixel sprites cap at 4096). */
  canExpand: boolean;
}

const source = (doc: Document): string => docTag(doc, 'pixel');

const keyOf = (bytes: Uint8Array): string | null => {
  try {
    return Document.imageKey(bytes);
  } catch {
    return null;
  }
};

const message = (err: unknown): string => (err instanceof Error ? err.message : String(err));

/**
 * Copy / Copy Merged / Cut: PNG to the system clipboard (this runs inside the
 * click or keydown, i.e. with user activation) and to the in-app record.
 * Cut on a group or tilemap layer is refused before anything changes.
 */
export function runCopy(kind: 'copy' | 'merged' | 'cut'): void {
  const doc = ed.doc;
  if (!doc) return;
  if (kind === 'cut') {
    const id = ed.activeLayerId ?? doc.paintTargetLayer();
    const layerKind = doc.layerKind(id);
    if (layerKind === 'group' || layerKind === 'tilemap') {
      ed.status = t('cut.imageLayersOnly');
      return;
    }
  }
  let copied;
  try {
    copied = kind === 'cut' ? doc.cutSelection() : doc.copySelection(kind === 'merged');
  } catch (err) {
    ed.status = t('copy.failed', { error: message(err) });
    return;
  }
  const png = new Uint8Array(copied.png());
  const record = { png, x: copied.x, y: copied.y, source: source(doc), key: keyOf(png) };
  copied.free();
  if (kind === 'cut') {
    live.needsRecompose = true;
    syncMeta();
    ed.docRev += 1;
  }
  void storeCopy(record);
}

/** Canvas position shown at the top-left of the visible viewport. */
function viewportOrigin(): { x: number; y: number } {
  const z = view.zoom;
  return {
    x: ((ed.canvasW * z - view.stageW) / 2 - view.panX) / z,
    y: ((ed.canvasH * z - view.stageH) / 2 - view.panY) / z,
  };
}

/**
 * Paste into the current document: Pincel's own copy back into its source
 * document lands at its origin, anything else at the viewport's top-left.
 * An image larger than the canvas opens the expand dialog first.
 */
export async function beginPaste(system: Blob | null): Promise<void> {
  const doc = ed.doc;
  if (!doc) return;
  const src = await resolvePaste(system, source(doc), keyOf);
  if (ed.doc !== doc) return;
  if (!src) {
    ed.status = t('paste.noImage');
    return;
  }
  const size = await imageSize(new Blob([src.bytes.slice()], { type: 'image/png' }));
  if (ed.doc !== doc) return;
  const canvas = { width: ed.canvasW, height: ed.canvasH };
  const at =
    src.origin ?? (size ? pastePosition(viewportOrigin(), size, canvas) : { x: 0, y: 0 });
  if (!size || (size.width <= canvas.width && size.height <= canvas.height)) {
    runPaste(doc, src.bytes, at.x, at.y, false);
    return;
  }
  dialogs.paste = {
    doc,
    bytes: src.bytes,
    x: at.x,
    y: at.y,
    width: size.width,
    height: size.height,
    canExpand: at.x + size.width <= MAX_DOC_SIZE && at.y + size.height <= MAX_DOC_SIZE,
  };
}

/** Runs the paste as one undo step; `expand` grows the canvas to cover it. */
export function runPaste(doc: Document, bytes: Uint8Array, x: number, y: number, expand: boolean): void {
  dialogs.paste = null;
  if (ed.doc !== doc) return;
  try {
    ed.activeLayerId = doc.pasteLayer(bytes, x, y, expand);
    live.needsRecompose = true;
    syncMeta();
    syncSelection();
    ed.docRev += 1;
    ed.status = t('px.pasted');
  } catch (err) {
    ed.status = t('paste.failed', { error: message(err) });
  }
}

/** Edit ▸ Paste: reads the system clipboard through the async API. */
export async function pasteFromClipboard(): Promise<void> {
  const doc = ed.doc;
  if (!doc) return;
  const blob = await clipboardImage();
  if (ed.doc !== doc) return;
  await beginPaste(blob);
}

/**
 * Edit ▸ Paste as New Image (Ctrl/⌘+Alt+V): the image Paste would use opens
 * as a new document; without one, a notice and no guard (copy-cut-paste
 * AC-23, AC-25).
 */
export async function pasteAsNewImage(): Promise<void> {
  const system = await clipboardImage();
  const bytes = await newImageBytes(system);
  if (!bytes) {
    ed.status = t('paste.noImage');
    return;
  }
  openInShell(pastedImageFile(bytes, system?.type ?? 'image/png'));
}

/** Ctrl+V / ⌘V: the browser's `paste` event, which needs no permission. */
export function onPaste(e: ClipboardEvent, blocked: boolean): void {
  if (!ed.doc || blocked || isTypingTarget(e.target)) return;
  // A paste inside any dialog (shell or editor) is not for the canvas.
  if (e.target instanceof Element && e.target.closest('[role="dialog"], [role="alertdialog"]')) {
    return;
  }
  e.preventDefault();
  void beginPaste(pasteEventImage(e));
}
