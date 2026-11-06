// Image-mode tabs (docs/specs/document-tabs.md): every open document is a
// core handle with its own tab state (`stores/editor.svelte.ts` `tabs`).
// Opening adds a tab after the last one and activates it; closing frees
// the handle (AC-16) and activates the right-hand neighbour, or the left
// one when the closed tab was the last (AC-07).
import { core } from './wasm';
import {
  docInit,
  editor,
  live,
  selectionPreview,
  shapePreview,
  tabs,
  viewInit,
  type ImageTab,
} from '../stores/editor.svelte';
import type { DocumentFile } from './document-file';
import { gestureControl } from '../tools/gesture-control';
import { dropCloneSource } from '../tools/clone-source.svelte';

let nextId = 1;

/** The tabs whose core documents are still open. A plain array, not the
 *  reactive list: the editor's unmount reads it from an effect teardown,
 *  where a `$state` read can still return the list before the last close
 *  and would close a closed tab's document twice. */
const allocated: ImageTab[] = [];

/** Close `tab`'s core document, once. */
function release(tab: ImageTab): void {
  const at = allocated.indexOf(tab);
  if (at < 0) return;
  allocated.splice(at, 1);
  dropCloneSource(tab.id);
  if (tab.doc.handle !== null) core.closeDocument(tab.doc.handle);
  tab.doc.handle = null;
}

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
  allocated.push(tab);
  activateTab(tab);
  return tab;
}

/** Makes `tab` the active one: the canvas, panels, menus and status bar
 *  follow it (AC-04); a gesture in flight and the transient previews of
 *  the previous tab are dropped, also for a newly opened tab. */
export function activateTab(tab: ImageTab): void {
  if (tabs.active === tab) return;
  gestureControl.cancel();
  // The live object is committed in the tab it belongs to (AC-09).
  gestureControl.commitLive();
  tabs.active = tab;
  // Only the active tab can have a live object, and switching committed it.
  live.object = null;
  live.cursor = null;
  editor.previewComposite = null;
  editor.notice = null;
  selectionPreview.value = null;
  shapePreview.value = null;
  editor.revision += 1;
}

/** Closes `tab` and frees its core document; the right-hand neighbour
 *  becomes active (the left one for the last tab). Returns the tab now
 *  active, `null` when none is left. */
export function closeTab(tab: ImageTab): ImageTab | null {
  const at = tabs.list.indexOf(tab);
  if (at < 0) return tabs.active;
  tabs.list.splice(at, 1);
  release(tab);
  if (tabs.active === tab) {
    const next = tabs.list[at] ?? tabs.list[at - 1] ?? null;
    if (next) activateTab(next);
    else tabs.active = null;
  }
  return tabs.active;
}

/** Closes every tab (the editor unmounts): no core document stays open. */
export function closeAllTabs(): void {
  for (const tab of [...allocated]) release(tab);
  tabs.list.length = 0;
  tabs.active = null;
}
