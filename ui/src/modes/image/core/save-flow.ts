// Image-mode Save / Save As (docs/specs/save-original-format.md): picks the
// format (Save As, or a format without an encoder, asks), asks what the
// format needs (flatten, JPEG quality once per document), then writes
// through `document-file`. At most one prompt precedes the save picker:
// Save As asks format, quality and the merge confirmation together, and
// Save asks flatten and quality together (AC-10, AC-17). Prompts are
// answered by `SaveDialogs`.
import {
  editor,
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
import { t } from '../../../lib/i18n/index.svelte';

/** Largest ICO image edge, px (the core's `ICO_MAX_SIDE`). */
const ICO_MAX_SIDE = 256;
/** Smallest JPEG XL image edge, px (the core's `JXL_MIN_SIDE`). */
const JXL_MIN_SIDE = 2;

/** The notice when `format` cannot hold the document at its size, else
 *  `null`: an ICO holds at most 256 × 256 px
 *  (docs/specs/image-formats-more.md AC-08), a JPEG XL needs at least
 *  2 × 2 px (docs/specs/image-formats-more-modern.md AC-16). */
function sizeNotice(tab: ImageTab, format: DocSaveFormat): string | null {
  const { width, height } = tab.doc;
  if (format === 'ico' && (width > ICO_MAX_SIDE || height > ICO_MAX_SIDE)) {
    return t('save.icoTooLarge');
  }
  if (format === 'jxl' && (width < JXL_MIN_SIDE || height < JXL_MIN_SIDE)) {
    return t('save.jxlTooSmall');
  }
  return null;
}

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
  // Save of a document its own format cannot hold at this size (an ICO
  // past 256 px, a JPEG XL under 2 px) goes to Save As, with a notice
  // (image-formats-more AC-08, image-formats-more-modern AC-16).
  const own = saveFormat(tab);
  const notice = forceAs || own === null ? null : sizeNotice(tab, own);
  if (notice !== null) editor.notice = notice;
  if (forceAs || own === null || notice !== null) return saveAs(tab, own, notice !== null);
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
async function saveAs(
  tab: ImageTab,
  own: DocSaveFormat | null,
  sizeBlocked = false,
): Promise<SaveResult | null> {
  const layered = tab.doc.layers.length > 1 || fileFormat(tab) === 'psd';
  const loses = flatteningLoses(tab);
  // A format that only cannot hold this size is writable: no "cannot be
  // written back", and PNG instead of it preselected (AC-08, AC-16).
  const choice = await ask<SaveAsChoice | null>((resolve) => ({
    kind: 'format',
    preselect: layered ? 'ora' : sizeBlocked ? 'png' : (own ?? 'png'),
    unwritable: own === null ? formatLabel(tab) : null,
    quality: rememberedJpegQuality(tab) ?? 90,
    confirmMerge: loses && !flattensWithoutAsking(tab),
    resolve,
  }));
  if (choice === null) return null;
  const { format } = choice;
  const notice = sizeNotice(tab, format);
  if (notice !== null) {
    // Nothing is written; Save As can run again with another format.
    editor.notice = notice;
    return null;
  }
  const quality = format === 'jpeg' ? choice.quality : undefined;
  const target = await writeDocument(tab, format, { forceAs: true, quality });
  return target === null ? null : { target, flattened: format !== 'ora' && loses };
}
