// Image-mode Save / Save As (docs/specs/save-original-format.md): picks the
// format (Save As, or a format without an encoder, asks), asks what the
// format needs (flatten, JPEG quality once per document), then writes
// through `document-file`. At most one prompt precedes the save picker:
// Save As asks format, quality and the merge confirmation together, and
// Save asks flatten and quality together (AC-10, AC-17). Prompts are
// answered by `SaveDialogs`.
import {
  savePrompt,
  tabs,
  type FlattenChoice,
  type ImageTab,
  type SaveAsChoice,
  type SavePrompt,
} from '../stores/editor.svelte';
import {
  flattenWithoutAsking,
  flatteningLoses,
  flattensWithoutAsking,
  fileFormat,
  formatLabel,
  rememberedJpegQuality,
  saveFormat,
  writeDocument,
  type DocSaveFormat,
} from './document-file';
import type { SaveTarget } from '../../../lib/fs';

/** What a Save wrote. */
export interface SaveResult {
  target: SaveTarget;
  /** The file is a flattened copy of a document whose layers it lost;
   *  autosave keeps its layered snapshot (AC-13). */
  flattened: boolean;
}

/** Shows a prompt and resolves with its answer once the dialog calls back. */
function ask<T>(make: (resolve: (value: T) => void) => SavePrompt): Promise<T> {
  return new Promise((resolve) => {
    savePrompt.value = make((value) => {
      savePrompt.value = null;
      resolve(value);
    });
  });
}

/** Saves `tab`'s document (Save, or Save As with `forceAs`). Resolves
 *  `null` when nothing was written: no document, or the user cancelled a
 *  prompt or the picker — the document then stays unsaved. The tab is
 *  fixed when the save starts, so a tab switch meanwhile does not change
 *  what is written (document-tabs AC-17). */
export async function saveDocument(
  forceAs = false,
  tab: ImageTab | null = tabs.active,
): Promise<SaveResult | null> {
  if (tab === null || tab.doc.handle === null) return null;
  const own = saveFormat(tab);
  if (forceAs || own === null) return saveAs(tab, own);
  const format = own;
  const flattened = format !== 'ora' && flatteningLoses(tab);
  let quality = format === 'jpeg' ? (rememberedJpegQuality(tab) ?? undefined) : undefined;
  const askQuality = format === 'jpeg' && quality === undefined;
  if (flattened && !flattensWithoutAsking(tab)) {
    const choice = await ask<FlattenChoice | null>((resolve) => ({
      kind: 'flatten',
      askQuality,
      resolve,
    }));
    if (choice === null) return null;
    if (choice.action === 'ora') {
      const target = await writeDocument(tab, 'ora', { forceAs: true });
      return target === null ? null : { target, flattened: false };
    }
    if (choice.dontAsk) flattenWithoutAsking(tab);
    quality = choice.quality ?? quality;
  }
  if (format === 'jpeg' && quality === undefined) {
    const q = await ask<number | null>((resolve) => ({ kind: 'jpeg', resolve }));
    if (q === null) return null;
    quality = q;
  }
  const target = await writeDocument(tab, format, { forceAs, quality });
  return target === null ? null : { target, flattened };
}

/** Save As, or Save of a format Pincel cannot write back (GIF, PSD): one
 *  dialog picks the format, its JPEG quality and confirms merging, then
 *  only the save picker follows (AC-10, AC-15, AC-16). A layered document,
 *  and any opened PSD, defaults to `.ora`, which keeps its layers
 *  (docs/specs/psd-import.md AC-12). */
async function saveAs(tab: ImageTab, own: DocSaveFormat | null): Promise<SaveResult | null> {
  const layered = tab.doc.layers.length > 1 || fileFormat(tab) === 'psd';
  const loses = flatteningLoses(tab);
  const choice = await ask<SaveAsChoice | null>((resolve) => ({
    kind: 'format',
    preselect: layered ? 'ora' : (own ?? 'png'),
    unwritable: own === null ? formatLabel(tab) : null,
    quality: rememberedJpegQuality(tab) ?? 90,
    confirmMerge: loses && !flattensWithoutAsking(tab),
    resolve,
  }));
  if (choice === null) return null;
  const { format } = choice;
  const quality = format === 'jpeg' ? choice.quality : undefined;
  const target = await writeDocument(tab, format, { forceAs: true, quality });
  return target === null ? null : { target, flattened: format !== 'ora' && loses };
}
