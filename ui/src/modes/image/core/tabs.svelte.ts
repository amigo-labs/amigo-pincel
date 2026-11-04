// Image-mode tabs (docs/specs/document-tabs.md): every open document is a
// core handle with its own tab state (`stores/editor.svelte.ts` `tabs`).
// Opening adds a tab after the last one and activates it; closing frees
// the handle (AC-16) and activates the right-hand neighbour, or the left
// one when the closed tab was the last (AC-07).
import { core } from './wasm';
import {
  docInit,
  editor,
  selectionPreview,
  shapePreview,
  tabs,
  viewInit,
  type ImageTab,
} from '../stores/editor.svelte';
import type { DocumentFile } from './document-file';

let nextId = 1;

/** Opens `handle` (an already created / decoded core document) as a new
 *  tab after the last one and makes it active. */
export function openTab(handle: number, file: DocumentFile): ImageTab {
  const tab: ImageTab = $state({
    id: nextId++,
    doc: { ...docInit(), handle, fileName: file.name },
    view: viewInit(),
    file,
  });
  tabs.list.push(tab);
  activateTab(tab);
  return tab;
}

/** Makes `tab` the active one: the canvas, panels, menus and status bar
 *  follow it (AC-04); transient previews of the previous tab are dropped. */
export function activateTab(tab: ImageTab): void {
  if (tabs.active === tab) return;
  tabs.active = tab;
  editor.previewComposite = null;
  editor.notice = null;
  selectionPreview.value = null;
  shapePreview.value = null;
  editor.revision += 1;
}

/** The tab with `id`, if it is open. */
export function tabById(id: number): ImageTab | null {
  return tabs.list.find((t) => t.id === id) ?? null;
}

/** Closes `tab` and frees its core document; the right-hand neighbour
 *  becomes active (the left one for the last tab). Returns the tab now
 *  active, `null` when none is left. */
export function closeTab(tab: ImageTab): ImageTab | null {
  const at = tabs.list.indexOf(tab);
  if (at < 0) return tabs.active;
  tabs.list.splice(at, 1);
  if (tab.doc.handle !== null) core.closeDocument(tab.doc.handle);
  tab.doc.handle = null;
  if (tabs.active === tab) {
    const next = tabs.list[at] ?? tabs.list[at - 1] ?? null;
    if (next) activateTab(next);
    else tabs.active = null;
  }
  return tabs.active;
}

/** Closes every tab (the editor unmounts): no core document stays open. */
export function closeAllTabs(): void {
  for (const tab of [...tabs.list]) {
    if (tab.doc.handle !== null) core.closeDocument(tab.doc.handle);
    tab.doc.handle = null;
  }
  tabs.list.length = 0;
  tabs.active = null;
}

/** The tab `step` places from the active one, wrapping around (AC-04). */
export function neighbourTab(step: 1 | -1): ImageTab | null {
  const n = tabs.list.length;
  if (n === 0 || tabs.active === null) return null;
  const at = tabs.list.indexOf(tabs.active);
  return tabs.list[(at + step + n) % n] ?? null;
}
