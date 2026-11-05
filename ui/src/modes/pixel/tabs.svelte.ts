// Pixel-mode tabs (docs/specs/document-tabs.md): every open document is a
// wasm `Document` with its own tab state (`stores/editor.svelte.ts` `tabs`).
// Opening adds a tab after the last one and activates it; switching keeps
// each tab's view, selection, current layer, frame and onion skin (AC-05);
// closing frees the document (AC-16) and activates the right-hand
// neighbour, or the left one when the closed tab was the last (AC-07).
import type { Document } from '../../lib/core';
import type { SaveTarget } from '../../lib/fs';
import { applyOnion, stopPlayback } from './doc-ops';
import { cancelPolygon, onWindowBlur } from './pointer';
import { syncMeta, syncSelection } from './render';
import { dialogs, ed, live, play, tabInit, tabs, toolState, type PixelTab } from './stores/editor.svelte';
import { fitView } from './view-ops';

let nextId = 1;

/** The tabs whose documents are still allocated. A plain array, not the
 *  reactive list: the editor's unmount reads it from an effect teardown,
 *  where a `$state` read can still return the list before the last close
 *  and would free a closed tab's document twice. */
const allocated: PixelTab[] = [];

/** Opens `doc` as a new tab after the last one, saved to `saveTarget`,
 *  and makes it active, fitted to the stage. */
export function openTab(doc: Document, saveTarget: SaveTarget): PixelTab {
  const tab: PixelTab = $state(tabInit(nextId++, doc, saveTarget));
  tabs.list.push(tab);
  allocated.push(tab);
  show(tab);
  // A fresh document starts with the default onion skin and a fitted view.
  applyOnion(play.onion);
  fitView();
  return tab;
}

/** Makes `tab` the active one (AC-04): the canvas, panels, timeline,
 *  palette and status bar follow it; tool and colours stay (AC-05). */
export function activateTab(tab: PixelTab): void {
  if (tabs.active !== tab) show(tab);
}

function show(tab: PixelTab): void {
  // Gestures and sub-modes in flight belong to the tab being left.
  stopPlayback();
  onWindowBlur();
  cancelPolygon();
  toolState.stampTile = null;
  toolState.stampHover = null;
  dialogs.editingTile = null;
  tabs.active = tab;
  live.needsRecompose = true;
  syncMeta();
  syncSelection();
  ed.docRev += 1;
}

/** Closes `tab` and frees its document; the right-hand neighbour becomes
 *  active (the left one for the last tab). Returns the tab now active,
 *  `null` when none is left. */
export function closeTab(tab: PixelTab): PixelTab | null {
  const at = tabs.list.indexOf(tab);
  if (at < 0) return tabs.active;
  tabs.list.splice(at, 1);
  if (tabs.active === tab) {
    const next = tabs.list[at] ?? tabs.list[at - 1] ?? null;
    if (next) show(next);
    else {
      stopPlayback();
      tabs.active = null;
    }
  }
  free(tab);
  return tabs.active;
}

/** Closes every tab (the editor unmounts): no document stays allocated. */
export function closeAllTabs(): void {
  stopPlayback();
  const all = [...allocated];
  tabs.list.length = 0;
  tabs.active = null;
  for (const tab of all) free(tab);
}

function free(tab: PixelTab): void {
  const at = allocated.indexOf(tab);
  if (at < 0) return;
  allocated.splice(at, 1);
  tab.doc.doc?.free();
  tab.doc.doc = null;
}
