// Image-mode Save / Save As (docs/specs/save-original-format.md): picks the
// format, asks what the format needs (JPEG quality once per document), then
// writes through `document-file`. Prompts are answered by `SaveDialogs`.
import { editor, savePrompt, type SavePrompt } from '../stores/editor.svelte';
import { rememberedJpegQuality, saveFormat, writeDocument } from './document-file';
import type { SaveTarget } from '../../../lib/fs';

/** What a Save wrote. */
export interface SaveResult {
  target: SaveTarget;
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
  const format = own ?? 'ora';
  let quality: number | undefined;
  if (format === 'jpeg') {
    const q =
      rememberedJpegQuality() ??
      (await ask<number | null>((resolve) => ({ kind: 'jpeg', resolve })));
    if (q === null) return null;
    quality = q;
  }
  const target = await writeDocument(format, { forceAs: forceAs || own === null, quality });
  return target === null ? null : { target };
}
