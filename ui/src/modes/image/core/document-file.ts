// The file an Image-mode document belongs to, and how Save writes it
// (docs/specs/save-original-format.md, spec §15 2026-10-29): an opened
// PNG / JPEG / WebP / BMP / TIFF is written back in its own format, an
// `.ora` or a new document as OpenRaster; GIF and PSD (never written) go
// to Save As. Each tab owns its file (docs/specs/document-tabs.md); the
// functions act on the active tab unless a tab is passed, so a save that
// outlives a tab switch still writes the tab it started in.
import { core } from './wasm';
import { editor, tabs, type ImageTab } from '../stores/editor.svelte';
import {
  writeTarget,
  type FileFormat,
  type ImageSaveFormat,
  type SaveTarget,
} from '../../../lib/fs';

/** Formats Image-mode Save writes. */
export type DocSaveFormat = 'ora' | ImageSaveFormat;

/** Formats an Image-mode document can come from; `null` when unknown. */
export type DocFormat = DocSaveFormat | 'gif' | 'psd';

/** The document's file: where Save writes, in which format, and the
 *  per-document answers that later Saves reuse. */
export interface DocumentFile extends SaveTarget {
  format: DocFormat | null;
  /** JPEG quality asked on the first JPEG Save; reused afterwards. */
  jpegQuality: number | null;
  /** "Don't ask again" was checked in the flatten prompt. */
  flattenWithoutAsking: boolean;
}

const DEFAULT_STEM = 'image';

/** Stand-in while no tab is open. */
const idleFile: DocumentFile = blankFile();

/** The file of `tab`, or of the active tab. */
function fileOf(tab: ImageTab | null = tabs.active): DocumentFile {
  return tab?.file ?? idleFile;
}

/** A new, never-saved document's file: Save asks for `image.ora`. */
export function blankFile(): DocumentFile {
  return {
    name: `${DEFAULT_STEM}.ora`,
    handle: null,
    path: null,
    format: 'ora',
    jpegQuality: null,
    flattenWithoutAsking: false,
  };
}

/** Replaces `tab`'s file and mirrors its name to the tab's document state. */
function setFile(tab: ImageTab | null, next: DocumentFile): void {
  if (tab === null) return;
  tab.file = next;
  tab.doc.fileName = next.name;
}

/** The Image-mode format of an opened file's `FileFormat`. */
export function docFormat(format: FileFormat | null): DocFormat | null {
  return format === null || format === 'aseprite' ? null : format;
}

/** `true` for the formats Save can write. */
export function isWritable(format: DocFormat | null): format is DocSaveFormat {
  return format !== null && format !== 'gif' && format !== 'psd';
}

/** The file of a document that came from `name` in `format`; `target`
 *  keeps its FSA handle / Tauri path so Save writes it in place. */
export function openedFile(
  name: string,
  format: DocFormat | null,
  target: Pick<SaveTarget, 'handle' | 'path'>,
): DocumentFile {
  return {
    name,
    handle: target.handle,
    path: target.path,
    format,
    jpegQuality: null,
    flattenWithoutAsking: false,
  };
}

/** The file name the editor title shows and Save writes to. */
export function fileName(tab: ImageTab | null = tabs.active): string {
  return fileOf(tab).name;
}

/** Where `tab`'s file lives (FSA handle / Tauri path), for recents and the
 *  same-file check (document-tabs AC-03). */
export function fileTarget(tab: ImageTab | null = tabs.active): SaveTarget {
  const { name, handle, path } = fileOf(tab);
  return { name, handle, path };
}

/** The file name without its extension (seeds export names). */
export function fileStem(tab: ImageTab | null = tabs.active): string {
  return fileOf(tab).name.replace(/\.[^.]+$/, '') || DEFAULT_STEM;
}

/** The document's own format, as detected at open (autosave stores it
 *  so a recovered document keeps saving back to it). */
export function fileFormat(tab: ImageTab | null = tabs.active): DocFormat | null {
  return fileOf(tab).format;
}

const DOC_FORMATS: readonly DocFormat[] = ['ora', 'png', 'jpeg', 'webp', 'bmp', 'tiff', 'gif', 'psd'];

/** Narrows a stored format string back to a `DocFormat`; `null` when it
 *  is missing or unknown. */
export function parseDocFormat(value: string | undefined): DocFormat | null {
  return DOC_FORMATS.find((f) => f === value) ?? null;
}

/** The format Save writes back to; `null` when it has no encoder. */
export function saveFormat(tab: ImageTab | null = tabs.active): DocSaveFormat | null {
  const format = fileOf(tab).format;
  return isWritable(format) ? format : null;
}

/** Short name of the document's own format for messages (`GIF`), from the
 *  detected format or else the file extension. */
export function formatLabel(tab: ImageTab | null = tabs.active): string {
  const file = fileOf(tab);
  const ext = /\.([^.]+)$/.exec(file.name)?.[1];
  return (file.format ?? ext ?? '?').toUpperCase();
}

/** The JPEG quality remembered for this document, if any. */
export function rememberedJpegQuality(tab: ImageTab | null = tabs.active): number | null {
  return fileOf(tab).jpegQuality;
}

/** Whether later Saves flatten without the prompt ("Don't ask again"). */
export function flattensWithoutAsking(tab: ImageTab | null = tabs.active): boolean {
  return fileOf(tab).flattenWithoutAsking;
}

/** Remembers "Don't ask again" for this document. */
export function flattenWithoutAsking(tab: ImageTab | null = tabs.active): void {
  if (tab) tab.file.flattenWithoutAsking = true;
}

/** `true` when a flat image would lose something the document has: more
 *  than one layer, a hidden layer (left out of the composite), or a
 *  layer's opacity or blend mode. */
export function flatteningLoses(tab: ImageTab | null = tabs.active): boolean {
  const layers = tab?.doc.layers ?? editor.layers;
  return (
    layers.length > 1 ||
    layers.some((l) => !l.visible || l.opacity < 1 || l.blend_mode !== 'normal')
  );
}

/** Marks the document as saved: it reads clean until its undo depth moves
 *  again. */
export function markSaved(tab: ImageTab | null = tabs.active): void {
  if (tab) tab.doc.savedUndoDepth = tab.doc.undoDepth;
}

/** Encodes the document in `format`: the layered OpenRaster project, or
 *  the flattened composite. `quality` (1–100) applies to JPEG only. */
function encode(handle: number, format: DocSaveFormat, quality: number): Uint8Array {
  switch (format) {
    case 'ora':
      return core.saveProject(handle, 6);
    case 'png':
      return core.exportPng(handle, 6);
    case 'jpeg':
      return core.exportJpeg(handle, Math.min(100, Math.max(1, Math.round(quality))));
    case 'webp':
      return core.exportWebp(handle);
    case 'bmp':
      return core.exportBmp(handle);
    case 'tiff':
      return core.exportTiff(handle);
  }
}

const EXTENSION: Record<DocSaveFormat, string> = {
  ora: 'ora',
  png: 'png',
  jpeg: 'jpg',
  webp: 'webp',
  bmp: 'bmp',
  tiff: 'tiff',
};

/** Writes the document as `format`: in place when it is the file's own
 *  format and `forceAs` is off, otherwise through the save picker / dialog /
 *  download under `<stem>.<ext>`. On success the file becomes the save
 *  target (Save As semantics) and the document reads saved; resolves `null`
 *  when the user cancelled the picker. */
export async function writeDocument(
  tab: ImageTab,
  format: DocSaveFormat,
  opts: { forceAs?: boolean; quality?: number | undefined } = {},
): Promise<SaveTarget | null> {
  const handle = tab.doc.handle;
  if (handle === null) return null;
  const file = tab.file;
  const quality = opts.quality ?? file.jpegQuality ?? 90;
  const same = format === file.format;
  const forceAs = (opts.forceAs ?? false) || !same;
  const target: SaveTarget = same
    ? { name: file.name, handle: file.handle, path: file.path }
    : { name: `${fileStem(tab)}.${EXTENSION[format]}`, handle: null, path: null };
  // Copy into a fresh ArrayBuffer so the file layer owns standalone memory.
  const bytes = new Uint8Array(encode(handle, format, quality));
  const depth = tab.doc.undoDepth;
  const written = await writeTarget(bytes, target, { forceAs, format });
  if (written === null) return null;
  setFile(tab, {
    name: written.name,
    handle: written.handle,
    path: written.path,
    format,
    jpegQuality: format === 'jpeg' ? quality : tab.file.jpegQuality,
    flattenWithoutAsking: tab.file.flattenWithoutAsking,
  });
  // The state that was written is the saved one, even if the user kept
  // editing while the file was written.
  tab.doc.savedUndoDepth = depth;
  return written;
}
