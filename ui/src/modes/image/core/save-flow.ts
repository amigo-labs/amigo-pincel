// Image-mode Save / Save As (docs/specs/save-original-format.md): picks the
// format (Save As, or a format without an encoder, asks), asks what the
// format needs (flatten, JPEG quality once per document), then writes
// through `document-file`. Prompts are answered by
// `SaveDialogs`.
import {
  editor,
  savePrompt,
  type FlattenChoice,
  type SavePrompt,
} from '../stores/editor.svelte';
import {
  flattenWithoutAsking,
  flatteningLoses,
  flattensWithoutAsking,
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

/** Saves the document (Save, or Save As with `forceAs`). Resolves `null`
 *  when nothing was written: no document, or the user cancelled a prompt
 *  or the picker — the document then stays unsaved. */
export async function saveDocument(forceAs = false): Promise<SaveResult | null> {
  if (editor.handle === null) return null;
  const own = saveFormat();
  let format: DocSaveFormat;
  if (forceAs || own === null) {
    // Save As, or a format Pincel cannot write back (GIF): pick the format.
    // A layered document defaults to `.ora`, which keeps its layers.
    const picked = await ask<DocSaveFormat | null>((resolve) => ({
      kind: 'format',
      preselect: editor.layers.length > 1 ? 'ora' : (own ?? 'png'),
      unwritable: own === null ? formatLabel() : null,
      resolve,
    }));
    if (picked === null) return null;
    format = picked;
    forceAs = true;
  } else {
    format = own;
  }
  const flattened = format !== 'ora' && flatteningLoses();
  if (flattened && !flattensWithoutAsking()) {
    const choice = await ask<FlattenChoice | null>((resolve) => ({ kind: 'flatten', resolve }));
    if (choice === null) return null;
    if (choice.action === 'ora') {
      const target = await writeDocument('ora', { forceAs: true });
      return target === null ? null : { target, flattened: false };
    }
    if (choice.dontAsk) flattenWithoutAsking();
  }
  let quality: number | undefined;
  if (format === 'jpeg') {
    const q =
      rememberedJpegQuality() ??
      (await ask<number | null>((resolve) => ({ kind: 'jpeg', resolve })));
    if (q === null) return null;
    quality = q;
  }
  const target = await writeDocument(format, { forceAs, quality });
  return target === null ? null : { target, flattened };
}
