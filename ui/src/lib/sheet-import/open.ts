// File ▸ Import Sprite Sheet… (docs/specs/sprite-sheet-import.md): decode
// the picked PNG for the dialog, and cut it into the `.aseprite` file the
// shell opens as a new Pixel-mode tab. Loaded on demand by the shell, so
// the start bundle stays free of the Pixel core.

import { loadCore, SpriteSheet } from '../core';
import type { OpenedFile } from '../fs';
import { t } from '../i18n/index.svelte';
import { documentName, rememberSheetDefaults, type GridValues } from './values';

export { default as SheetImportDialog } from './SpriteSheetImportDialog.svelte';

/** What the dialog's Import hands back. */
export interface SheetImportValues {
  grid: GridValues;
  /** `0xRRGGBBAA`, or `undefined` with "Make background transparent" off. */
  background: number | undefined;
  duration: number;
  rowsAsTags: boolean;
}

/** `opened` decoded as a sprite sheet, or the notice why it cannot be (AC-13). */
export async function readSheet(opened: OpenedFile): Promise<SpriteSheet | string> {
  try {
    await loadCore();
    return SpriteSheet.decode(opened.bytes);
  } catch (err) {
    if (err === 'too-large') return t('sheetImport.tooLarge', { name: opened.name });
    // A wasm trap is an allocation the core could not handle as an error.
    if (err === 'out-of-memory' || err instanceof WebAssembly.RuntimeError) {
      return t('sheetImport.outOfMemory', { name: opened.name });
    }
    return t('sheetImport.unreadable', { name: opened.name });
  }
}

/**
 * The new document for `sheet` cut along `values`, as an unsaved
 * `.aseprite` file named after `fileName` (AC-04, AC-15), or the notice
 * when memory ran out (AC-13). Remembers the values for the next dialog
 * (AC-10).
 */
export function buildSheetFile(
  sheet: SpriteSheet,
  fileName: string,
  values: SheetImportValues,
): OpenedFile | string {
  try {
    const doc = sheet.build(
      Uint32Array.from(values.grid),
      values.background,
      values.duration,
      values.rowsAsTags,
    );
    let bytes: Uint8Array;
    try {
      bytes = new Uint8Array(doc.saveAseprite());
    } finally {
      doc.free();
    }
    rememberSheetDefaults({
      grid: values.grid,
      duration: values.duration,
      rowsAsTags: values.rowsAsTags,
    });
    return {
      name: documentName(fileName),
      bytes,
      mime: 'application/x-aseprite',
      handle: null,
      path: null,
    };
  } catch (err) {
    if (err === 'out-of-memory' || err instanceof WebAssembly.RuntimeError) {
      return t('sheetImport.outOfMemory', { name: fileName });
    }
    return t('sheetImport.unreadable', { name: fileName });
  }
}

/** Free the decoded sheet once the dialog closes. */
export function releaseSheet(sheet: SpriteSheet): void {
  try {
    sheet.free();
  } catch {
    // A sheet left borrowed by a trap cannot be freed; its memory is lost
    // with the trap anyway.
  }
}
