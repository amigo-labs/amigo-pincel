// File operations: open, save, recents, autosave, PNG / sheet / animation
// export.

import { invoke } from '@tauri-apps/api/core';
import { Document, type Vectorizer } from '../../lib/core';
import type { SvgTrace } from '../../lib/components/SvgExportDialog.svelte';
import {
  type OpenedFile,
  type SaveTarget,
  ensureReadPermission,
  formatFromName,
  hasFsAccess,
  isPngBytes,
  mimeFromName,
  pickAndOpen,
  saveExport,
  sniffFormat,
  writeTarget,
} from '../../lib/fs';
import { t } from '../../lib/i18n/index.svelte';
import { AutosaveSession } from '../../lib/idb/autosave-session.svelte';
import { isIdbAvailable } from '../../lib/idb/db';
import { type RecentFile, listRecents, upsertRecent } from '../../lib/idb/recent-files';
import { hasNativeMenu, isTauri } from '../../lib/platform';
import type {
  AnimationExportOptions,
  AnimationSummary,
} from './components/AnimationExportDialog.svelte';
import type { SpriteSheetOptions } from './components/SpriteSheetDialog.svelte';
import { disposeDoc, resetDocViewState } from './doc-ops';
import { rememberExportScale } from './export-scale';
import { afterImageCommand } from './image-ops';
import { dialogs, ed, file } from './stores/editor.svelte';

/** Shell callbacks this module serves (`EditorProps`). */
const shell: {
  requestNew: () => void;
  openForeign: (opened: OpenedFile) => void;
  openFile: (opened: OpenedFile) => void;
  notice: (message: string) => void;
} = {
  requestNew: () => {},
  openForeign: () => {},
  openFile: () => {},
  notice: () => {},
};

/** Hand a document to the shell, which guards unsaved changes and opens it. */
export function openInShell(opened: OpenedFile): void {
  shell.openFile(opened);
}

/** Hand the shell's callbacks to the file operations (editor mount). */
export function bindShell(next: typeof shell): void {
  Object.assign(shell, next);
}

/** "Modified since last save": the undo depth moved off the saved one. */
export function isDocDirty(): boolean {
  return ed.doc !== null && ed.undoDepth !== ed.lastSavedUndoDepth;
}

// Stable across the session; drives Save / Save As button labels.
export const fsAccessAvailable = hasFsAccess();

export const tauriHost = isTauri();

/** True only where the OS draws the menu bar (macOS desktop app), which
 *  is then the owner of the `NATIVE_MENU_ACTIONS` chords (spec §9.4). */
export const nativeMenuHost = hasNativeMenu();

export const idbAvailable = isIdbAvailable();

export const recentsAvailable = (fsAccessAvailable || tauriHost) && idbAvailable;

// Run `action` immediately when the document is clean; otherwise park
// it behind the inline "discard unsaved changes?" confirm dialog.
export function guardUnsaved(action: () => void) {
  if (!isDocDirty()) {
    action();
    return;
  }
  dialogs.confirm = {
    message: t('px.discardNamed', { name: file.saveTarget.name }),
    run: action,
  };
}

export function confirmDiscard() {
  const pending = dialogs.confirm;
  dialogs.confirm = null;
  pending?.run();
}

// The New dialog lives in the shell (it offers both editor modes and
// runs the unsaved-changes guard from the mirrored dirty state).
export function openNewDialog() {
  if (file.busy) return;
  shell.requestNew();
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
    // A file that cannot be read gets the shell's notice (file-access AC-16).
    shell.notice(t('shell.openFailed', { error: err instanceof Error ? err.message : String(err) }));
    return;
  }
  if (!opened) return;
  // Anything that is neither a sprite nor a PNG belongs to the Image
  // mode, and an unknown file gets the shell's notice naming it
  // (app-shell AC-11, AC-12): the shell routes both and leaves the open
  // document alone when nothing can open the file.
  const format = sniffFormat(opened.bytes) ?? formatFromName(opened.name);
  if (format !== 'aseprite' && format !== 'png') {
    shell.openForeign(opened);
    return;
  }
  await openOpened(opened);
}

// Replace the current document with the already-read `opened` file
// (Open…, or the shell's initial file on mount).
export async function openOpened(opened: OpenedFile) {
  try {
    // A PNG opens as a fresh single-layer sprite; its save target is a
    // sibling .aseprite (never the PNG itself), so the first Save asks.
    const png = isPngBytes(opened.bytes);
    const next = png ? Document.openPng(opened.bytes) : Document.openAseprite(opened.bytes);
    disposeDoc();
    ed.doc = next;
    resetDocViewState();
    file.saveTarget = png
      ? { name: opened.name.replace(/\.png$/i, '') + '.aseprite', handle: null, path: null }
      : {
          name: opened.name,
          handle: opened.handle,
          path: opened.path,
        };
    ed.lastSavedUndoDepth = ed.doc.undoDepth;
    ed.status = t('px.opened', { name: opened.name, w: ed.doc.width, h: ed.doc.height });
    await autosave.opened();
    await recordRecent();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.open'), error: msg });
  }
}

export async function save(opts: { forceAs?: boolean } = {}) {
  if (!ed.doc || file.busy) return;
  const forceAs = opts.forceAs ?? false;
  file.busy = true;
  try {
    const bytes = new Uint8Array(ed.doc.saveAseprite());
    const next = await writeTarget(bytes, file.saveTarget, { forceAs });
    // A cancelled save picker or dialog wrote nothing: still unsaved.
    if (next === null) return;
    file.saveTarget = next;
    ed.lastSavedUndoDepth = ed.doc.undoDepth;
    ed.status = t('px.saved', { bytes: bytes.length, name: next.name });
    await autosave.saved();
    await recordRecent();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.save'), error: msg });
  } finally {
    file.busy = false;
  }
}

// Autosave and recovery, shared with Image mode
// (lib/idb/autosave-session): `.aseprite` snapshots tagged `pixel`.
/** The editor's autosave session; recreated per mount by `initAutosave`. */
export let autosave: AutosaveSession;

/** Start a fresh autosave session (editor mount). */
export function initAutosave(): void {
  autosave = new AutosaveSession({
    mode: 'pixel',
    depth: () => ed.doc?.undoDepth ?? null,
    savedDepth: () => ed.lastSavedUndoDepth,
    busy: () => file.busy,
    name: () => file.saveTarget.name,
    encode: () => (ed.doc ? new Uint8Array(ed.doc.saveAseprite()) : null),
    open: async (bytes, meta) => {
      const next = Document.openAseprite(bytes);
      disposeDoc();
      ed.doc = next;
      resetDocViewState();
      file.saveTarget = { name: meta.name, handle: null, path: null };
      // Recovered work is not on disk yet: it reads as unsaved.
      ed.lastSavedUndoDepth = -1;
      ed.status = t('px.recovered', { name: meta.name, w: ed.doc.width, h: ed.doc.height });
    },
    report: (action, error) => {
      const act =
        action === 'autosave'
          ? 'act.autosave'
          : action === 'recover'
            ? 'act.recover'
            : 'act.discard';
      ed.status = t('status.failed', { action: t(act), error });
    },
  });
}

// Persist the current `saveTarget` to the recent-files registry and
// refresh the in-memory list. No-op when the registry isn't
// available (non-FSA browsers / IDB disabled) or the current target
// doesn't carry a handle (a download-only save, or a New document
// that hasn't been written yet).
export async function recordRecent() {
  if (!recentsAvailable) return;
  // A recent needs something a future re-open can use: an FSA handle
  // (web) or a path (Tauri).
  if (!file.saveTarget.handle && !file.saveTarget.path) return;
  try {
    await upsertRecent({
      id: autosave.docId,
      name: file.saveTarget.name,
      handle: file.saveTarget.handle,
      path: file.saveTarget.path,
      mode: 'pixel',
    });
    file.recents = await listRecents();
  } catch (err) {
    // Persistence is best-effort; surface to the status line but
    // don't block the save / open the user already completed.
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('px.recentsFailed', { status: ed.status, error: msg });
  }
}

export async function openRecent(r: RecentFile) {
  if (file.busy) return;
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
      // for write via `writeTarget` → `ensureReadWritePermission`.
      if (!(await ensureReadPermission(r.handle))) {
        shell.notice(t('shell.permissionDenied', { name: r.name }));
        return;
      }
      const file = await r.handle.getFile();
      bytes = new Uint8Array(await file.arrayBuffer());
      name = file.name;
      nextTarget = { name: file.name, handle: r.handle, path: null };
    } else {
      shell.notice(t('shell.noHandle', { name: r.name }));
      return;
    }
    // The registry also lists Image-mode files: anything that is not a
    // sprite goes to the shell, which routes it like any other open
    // (app-shell AC-09, AC-10).
    const format = sniffFormat(bytes) ?? formatFromName(name);
    if (format !== null && format !== 'aseprite') {
      shell.openForeign({
        name,
        bytes,
        mime: mimeFromName(name),
        handle: nextTarget.handle,
        path: nextTarget.path,
      });
      return;
    }
    const next = Document.openAseprite(bytes);
    disposeDoc();
    ed.doc = next;
    resetDocViewState();
    file.saveTarget = nextTarget;
    ed.lastSavedUndoDepth = ed.doc.undoDepth;
    ed.status = t('px.opened', { name, w: ed.doc.width, h: ed.doc.height });
    // Keep the recent's id so re-opens count as the same document.
    await autosave.opened(r.id);
    await recordRecent();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    shell.notice(t('shell.recentFailed', { name: r.name, error: msg }));
  } finally {
    file.busy = false;
  }
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

// File ▸ Export ▸ PNG (current frame) asks for the scale first
// (docs/specs/scaled-export.md).
export function openPngDialog() {
  if (ed.doc && !file.busy) dialogs.png = true;
}

// Export the current frame's visible composite as a PNG (spec §7.3),
// upscaled by `scale` (1–10).
export async function exportPng(scale: number) {
  dialogs.png = false;
  if (!ed.doc || file.busy) return;
  rememberExportScale(scale);
  file.busy = true;
  try {
    const bytes = new Uint8Array(ed.doc.exportPng(ed.currentFrame, scale));
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
  rememberExportScale(options.scale);
  file.busy = true;
  try {
    const base =
      file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '') +
      (options.tag ? `-${options.tag}` : '');
    const atlas = ed.doc.exportAtlas(
      options.columns,
      options.tag ?? undefined,
      `${base}.png`,
      options.scale,
    );
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
