// File operations: open, recents, PNG / sheet / animation export. Every
// opened document becomes a tab (docs/specs/document-tabs.md); saving and
// autosave live per tab in `tab-session.ts`.

import { invoke } from '@tauri-apps/api/core';
import { Document, type Vectorizer } from '../../lib/core';
import type { SvgTrace } from '../../lib/components/SvgExportDialog.svelte';
import {
  type FileLocation,
  type OpenedFile,
  type SaveTarget,
  ensureReadPermission,
  formatFromName,
  hasFsAccess,
  isPngBytes,
  mimeFromName,
  pickAndOpen,
  sameFile,
  saveExport,
  sniffFormat,
} from '../../lib/fs';
import { t } from '../../lib/i18n/index.svelte';
import { isIdbAvailable } from '../../lib/idb/db';
import type { RecentFile } from '../../lib/idb/recent-files';
import { hasNativeMenu, isTauri } from '../../lib/platform';
import type { NewDocParams } from '../../lib/shell/types';
import type {
  AnimationExportOptions,
  AnimationSummary,
} from './components/AnimationExportDialog.svelte';
import type { SpriteSheetOptions } from './components/SpriteSheetDialog.svelte';
import { afterImageCommand } from './image-ops';
import { DEFAULT_FILE_NAME, MAX_DOC_SIZE, dialogs, ed, file, tabs } from './stores/editor.svelte';
import { attachTab, recentsAvailable, recordRecent } from './tab-session';
import { activateTab, openTab } from './tabs.svelte';

export { recentsAvailable };

/** Shell callbacks this module serves (`EditorProps`). */
const shell: {
  requestNew: () => void;
  openForeign: (opened: OpenedFile) => void;
  openFile: (opened: OpenedFile) => void;
} = {
  requestNew: () => {},
  openForeign: () => {},
  openFile: () => {},
};

/** Hand a document to the shell, which routes it by format. */
export function openInShell(opened: OpenedFile): void {
  shell.openFile(opened);
}

/** Hand the shell's callbacks to the file operations (editor mount). */
export function bindShell(next: typeof shell): void {
  Object.assign(shell, next);
}

// Stable across the session; drives Save / Save As button labels.
export const fsAccessAvailable = hasFsAccess();

export const tauriHost = isTauri();

/** True only where the OS draws the menu bar (macOS desktop app), which
 *  is then the owner of the `NATIVE_MENU_ACTIONS` chords (spec §9.4). */
export const nativeMenuHost = hasNativeMenu();

export const idbAvailable = isIdbAvailable();

// The New dialog lives in the shell (it offers both editor modes); Create
// in Pixel mode comes back as `createTab`.
export function openNewDialog() {
  if (file.busy) return;
  shell.requestNew();
}

/** A blank sprite as a new tab (File ▸ New… in Pixel mode, or the
 *  editor's first document). */
export function createTab(params: Pick<NewDocParams, 'width' | 'height'>): void {
  const w = Math.max(1, Math.min(MAX_DOC_SIZE, Math.floor(params.width)));
  const h = Math.max(1, Math.min(MAX_DOC_SIZE, Math.floor(params.height)));
  const tab = openTab(new Document(w, h), { name: DEFAULT_FILE_NAME, handle: null, path: null });
  tab.doc.lastSavedUndoDepth = tab.doc.undoDepth;
  attachTab(tab);
  ed.status = t('px.ready');
}

export async function openDoc() {
  if (file.busy) return;
  file.busy = true;
  try {
    await openDocInner();
  } finally {
    file.busy = false;
  }
}

export async function openDocInner() {
  let opened;
  try {
    opened = await pickAndOpen();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.open'), error: msg });
    return;
  }
  if (!opened) return;
  // Anything that is neither a sprite nor a PNG belongs to the Image
  // mode; the shell re-routes it (unknown bytes fall through so the
  // codec error below names the problem).
  const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
  if (format !== null && format !== 'aseprite' && format !== 'png') {
    shell.openForeign(opened);
    return;
  }
  await openOpened(opened);
}

/** Activates the open tab whose file is `location` (document-tabs AC-03);
 *  `false` when no tab holds it. */
async function activateSameFile(location: FileLocation): Promise<boolean> {
  for (const tab of tabs.list) {
    if (await sameFile(tab.file.saveTarget, location)) {
      activateTab(tab);
      return true;
    }
  }
  return false;
}

// Open the already-read `opened` file as a new tab (Open…, a file the
// shell routes here, or the shell's initial file on mount), or activate
// the tab that holds it. `false` when it could not be opened; the active
// tab stays.
export async function openOpened(opened: OpenedFile): Promise<boolean> {
  try {
    if (await activateSameFile(opened)) return true;
    // A PNG opens as a fresh single-layer sprite; its save target is a
    // sibling .aseprite (never the PNG itself), so the first Save asks.
    const png = isPngBytes(opened.bytes);
    const next = png ? Document.openPng(opened.bytes) : Document.openAseprite(opened.bytes);
    const tab = openTab(
      next,
      png
        ? { name: opened.name.replace(/\.png$/i, '') + '.aseprite', handle: null, path: null }
        : { name: opened.name, handle: opened.handle, path: opened.path },
    );
    tab.doc.lastSavedUndoDepth = next.undoDepth;
    attachTab(tab);
    ed.status = t('px.opened', { name: opened.name, w: next.width, h: next.height });
    await recordRecent(tab);
    return true;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.open'), error: msg });
    return false;
  }
}

// File ▸ Open Recent: the file is read and recognized first. A sprite
// opens as a tab (or activates the tab that holds it); another mode's file
// goes to the shell, which recognizes it and asks before replacing the
// session (unsaved-changes-prompt AC-01, AC-11).
export async function openRecent(r: RecentFile) {
  if (file.busy) return;
  if (await activateSameFile(r)) return;
  const read = await readRecent(r);
  if (read === null) return;
  if ((sniffFormat(read.bytes) ?? formatFromName(read.name)) !== 'aseprite') {
    shell.openFile({
      name: read.name,
      bytes: read.bytes,
      mime: mimeFromName(read.name),
      handle: read.target.handle,
      path: read.target.path,
    });
    return;
  }
  await applyRecent(r, read);
}

interface ReadRecent {
  name: string;
  bytes: Uint8Array;
  target: SaveTarget;
}

async function readRecent(r: RecentFile): Promise<ReadRecent | null> {
  file.busy = true;
  try {
    let name: string;
    let bytes: Uint8Array;
    let nextTarget: SaveTarget;
    if (tauriHost && r.path) {
      const raw = await invoke<number[] | ArrayBuffer>('read_file_bytes', {
        path: r.path,
      });
      bytes = raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
      name = r.name;
      nextTarget = { name: r.name, handle: null, path: r.path };
    } else if (r.handle) {
      // Read access is enough to open. The next save will prompt
      // for write via `saveBytes` → `ensureReadWritePermission`.
      if (!(await ensureReadPermission(r.handle))) {
        ed.status = t('px.recentDenied', { name: r.name });
        return null;
      }
      const file = await r.handle.getFile();
      bytes = new Uint8Array(await file.arrayBuffer());
      name = file.name;
      nextTarget = { name: file.name, handle: r.handle, path: null };
    } else {
      ed.status = t('px.recentNoHandle', { name: r.name });
      return null;
    }
    return { name, bytes, target: nextTarget };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('px.recentFailed', { name: r.name, error: msg });
    return null;
  } finally {
    file.busy = false;
  }
}

async function applyRecent(r: RecentFile, read: ReadRecent): Promise<void> {
  let next: Document;
  try {
    next = Document.openAseprite(read.bytes);
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('px.recentFailed', { name: r.name, error: msg });
    return;
  }
  const tab = openTab(next, read.target);
  tab.doc.lastSavedUndoDepth = next.undoDepth;
  // Keep the recent's id so re-opens count as the same document.
  attachTab(tab, { docId: r.id });
  ed.status = t('px.opened', { name: read.name, w: next.width, h: next.height });
  await recordRecent(tab);
}

/** Open a recovered snapshot as a new, unsaved tab that continues under
 *  the snapshot's id (document-tabs AC-15). */
export function openRecovered(bytes: Uint8Array, name: string, docId: string): void {
  const next = Document.openAseprite(bytes);
  const tab = openTab(next, { name, handle: null, path: null });
  // Recovered work is not on disk yet: it reads as unsaved.
  tab.doc.lastSavedUndoDepth = -1;
  attachTab(tab, { docId, recovered: true });
  ed.status = t('px.recovered', { name, w: next.width, h: next.height });
}

// Import a PNG as a new layer on top of the current document.
export async function importPngLayer() {
  if (!ed.doc || file.busy) return;
  file.busy = true;
  try {
    const opened = await pickAndOpen();
    if (!opened || !ed.doc) return;
    if (!isPngBytes(opened.bytes)) {
      ed.status = t('px.notPng');
      return;
    }
    const name = opened.name.replace(/\.png$/i, '');
    const id = ed.doc.importPngAsLayer(opened.bytes, name);
    ed.activeLayerId = id;
    afterImageCommand(t('px.imported', { name: opened.name }));
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('menu.importPng'),
      error: err instanceof Error ? err.message : String(err),
    });
  } finally {
    file.busy = false;
  }
}

// Export the current frame's visible composite as a PNG (spec §7.3).
export async function exportPng() {
  if (!ed.doc || file.busy) return;
  file.busy = true;
  try {
    const bytes = new Uint8Array(ed.doc.exportPng(ed.currentFrame));
    const base = file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '');
    const suffix = ed.frameCount > 1 ? `-${ed.currentFrame + 1}` : '';
    if (await saveExport(bytes, `${base}${suffix}.png`)) {
      ed.status = t('px.exportedPng', { bytes: bytes.length });
    }
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.export'),
      error: err instanceof Error ? err.message : String(err),
    });
  } finally {
    file.busy = false;
  }
}

// File ▸ Export ▸ SVG… (docs/specs/vectorize-dialog.md). The vectorizer
// lives while the dialog is open, so re-tracing reuses its caches. It is
// tied to the document and frame it was made from: a document replaced
// while the dialog is open (native File ▸ New / Open) gets a fresh one.
let vectorizer: { source: Document; frame: number; tracer: Vectorizer } | null = null;

function dropVectorizer() {
  vectorizer?.tracer.free();
  vectorizer = null;
}

export function openSvgDialog() {
  if (!ed.doc || file.busy) return;
  dropVectorizer();
  dialogs.svg = true;
}

export function closeSvgDialog() {
  dialogs.svg = false;
  dropVectorizer();
}

/** Traces the current frame with the dialog's mode and option vector. */
export function traceSvg(mode: number, params: number[]): SvgTrace {
  if (!ed.doc) return { error: 'no document' };
  try {
    if (vectorizer?.source !== ed.doc || vectorizer.frame !== ed.currentFrame) {
      dropVectorizer();
      vectorizer = { source: ed.doc, frame: ed.currentFrame, tracer: ed.doc.vectorizer(ed.currentFrame) };
    }
    const r = vectorizer.tracer.trace(mode, new Float64Array(params));
    try {
      return { svg: r.svg, layers: r.layers, outlines: r.outlines, threshold: r.threshold };
    } finally {
      r.free();
    }
  } catch (err) {
    return { error: err instanceof Error ? err.message : String(err) };
  }
}

export async function exportSvg(svg: string) {
  closeSvgDialog();
  if (!ed.doc || file.busy) return;
  file.busy = true;
  try {
    const bytes = new Uint8Array(new TextEncoder().encode(svg));
    const base = file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '');
    if (await saveExport(bytes, `${base}.svg`, 'svg')) ed.status = t('px.exportedSvg', { bytes: bytes.length });
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.export'),
      error: err instanceof Error ? err.message : String(err),
    });
  } finally {
    file.busy = false;
  }
}

export function openAnimationDialog() {
  if (ed.doc && !file.busy) dialogs.animation = true;
}

/** The core's view of the frames `tag` selects; `null` when the core
 *  refuses the sprite (the export then reports the error). */
export function describeAnimation(tag: string | null): AnimationSummary | null {
  if (!ed.doc) return null;
  try {
    const info = ed.doc.animationInfo(tag ?? undefined);
    try {
      return {
        frameCount: info.frameCount,
        defaultLoops: info.defaultLoops,
        gifApproximated: info.gifApproximated,
        semiTransparent: info.semiTransparent,
      };
    } finally {
      info.free();
    }
  } catch {
    return null;
  }
}

// Export the animation as APNG (`.png`) or GIF (spec §7.5).
export async function exportAnimation(options: AnimationExportOptions) {
  dialogs.animation = false;
  if (!ed.doc || file.busy) return;
  file.busy = true;
  try {
    const bytes = new Uint8Array(
      ed.doc.exportAnimation(options.format, options.tag ?? undefined, options.scale, options.loops),
    );
    const base = file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '');
    const saved =
      options.format === 'gif'
        ? await saveExport(bytes, `${base}.gif`, 'gif')
        : await saveExport(bytes, `${base}.png`);
    if (saved) ed.status = t('px.exportedAnimation', { bytes: bytes.length });
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.animationExport'),
      error: err instanceof Error ? err.message : String(err),
    });
  } finally {
    file.busy = false;
  }
}

export function openSheetDialog() {
  if (ed.doc && !file.busy) dialogs.sheet = true;
}

export async function exportSheet(options: SpriteSheetOptions) {
  dialogs.sheet = false;
  if (!ed.doc || file.busy) return;
  file.busy = true;
  try {
    const base =
      file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '') +
      (options.tag ? `-${options.tag}` : '');
    const atlas = ed.doc.exportAtlas(options.columns, options.tag ?? undefined, `${base}.png`);
    let png: Uint8Array<ArrayBuffer>;
    let data: string;
    try {
      png = new Uint8Array(atlas.png);
      data = atlas.data;
    } finally {
      atlas.free();
    }
    if (!(await saveExport(png, `${base}.png`))) return;
    if (options.data) {
      const json = new TextEncoder().encode(data);
      await saveExport(new Uint8Array(json), `${base}.json`, 'json');
    }
    ed.status = t('px.exportedSheet', { bytes: png.length });
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.sheetExport'),
      error: err instanceof Error ? err.message : String(err),
    });
  } finally {
    file.busy = false;
  }
}
