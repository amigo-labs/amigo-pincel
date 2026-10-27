// Document edits: frames, playback, undo / redo, selection and layer commands.

import type { DropPlace } from '../../lib/components/layers/layers';
import { t } from '../../lib/i18n/index.svelte';
import {
  type OnionSettings,
  type PlayCursor,
  type PlayRange,
  type TagDirection,
  advance,
  startCursor,
} from './playback';
import { syncMeta, syncSelection } from './render';
import { colors, dialogs, ed, live, play, toolState } from './stores/editor.svelte';
import { fitView } from './view-ops';

/** A Select-menu command. */
export type SelectAction = 'all' | 'none' | 'invert' | 'expand' | 'contract';

export function swapColors() {
  [colors.color, colors.bgColor] = [colors.bgColor, colors.color];
  [colors.alpha, colors.bgAlpha] = [colors.bgAlpha, colors.alpha];
}

export function resetColors() {
  colors.color = '#000000';
  colors.alpha = 255;
  colors.bgColor = '#ffffff';
  colors.bgAlpha = 255;
}

export function selectAction(action: SelectAction, radius = 1) {
  if (!ed.doc) return;
  try {
    switch (action) {
      case 'all':
        selectAll();
        break;
      case 'none':
        deselect();
        break;
      case 'invert':
        ed.doc.invertSelection();
        break;
      case 'expand':
        ed.doc.expandSelection(radius);
        break;
      case 'contract':
        ed.doc.contractSelection(radius);
        break;
    }
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.selection'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

// wasm-bindgen classes own Rust-side allocations; freeing the prior
// `Document` before replacing it avoids leaking memory across
// repeated New / Open operations. Safe to call when `doc` is null.
export function disposeDoc() {
  if (ed.doc) {
    ed.doc.free();
    ed.doc = null;
  }
}

// Switch the viewed / edited frame. Clamped; the wasm side emits a
// dirty-canvas event that drives the recompose.
export function setFrame(next: number) {
  if (!ed.doc) return;
  const clamped = Math.max(0, Math.min(ed.frameCount - 1, next));
  if (clamped === ed.currentFrame) return;
  try {
    ed.doc.setCurrentFrame(clamped);
    ed.currentFrame = clamped;
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.frameSwitch'), error: msg });
  }
}

export function addLayer() {
  if (!ed.doc) return;
  try {
    const id = ed.doc.addLayer(`Layer ${ed.doc.layerCount + 1}`);
    ed.activeLayerId = id;
    // Route the paint tools to the freshly created layer.
    ed.doc.setActiveLayer(id);
    live.needsRecompose = true;
    syncMeta();
    ed.docRev += 1;
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.addLayer'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

/** Add an empty group on top and make it the active row. */
export function addGroup() {
  if (!ed.doc) return;
  try {
    let groups = 0;
    for (let i = 0; i < ed.doc.layerCount; i += 1) {
      if (ed.doc.layerKind(ed.doc.layerIdAt(i)) === 'group') groups += 1;
    }
    ed.activeLayerId = ed.doc.addGroup(`Group ${groups + 1}`);
    afterLayerEdit();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.addGroup'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

/** Move a layer (with its subtree) above / below another or into a group. */
export function moveLayerTo(layerId: number, targetId: number, place: DropPlace) {
  if (!ed.doc) return;
  try {
    ed.doc.moveLayerTo(layerId, targetId, place);
    afterLayerEdit();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.layerMove'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function addFrame() {
  if (!ed.doc) return;
  try {
    // 100 ms matches the default frame duration from a fresh document.
    const idx = ed.doc.addFrame(100);
    live.needsRecompose = true;
    syncMeta();
    // Step to the new frame so it's immediately viewable / paintable.
    setFrame(idx);
    ed.docRev += 1;
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.addFrame'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function removeFrame(frame: number) {
  if (!ed.doc) return;
  try {
    ed.doc.removeFrame(frame);
    live.needsRecompose = true;
    syncMeta();
    ed.docRev += 1;
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.removeFrame'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function applyOnion(next: OnionSettings) {
  play.onion = next;
  ed.doc?.setOnionSkin(next.enabled, next.back, next.forward, next.alpha, next.alpha);
}

// The range playback loops over: the chosen tag, or every frame.
export function playRange(): PlayRange {
  if (ed.doc && play.loopTag !== null && play.loopTag < ed.doc.tagCount) {
    return {
      from: ed.doc.tagFrom(play.loopTag),
      to: ed.doc.tagTo(play.loopTag),
      direction: ed.doc.tagDirection(play.loopTag) as TagDirection,
    };
  }
  return { from: 0, to: Math.max(0, ed.frameCount - 1), direction: 'forward' };
}

export function stopPlayback() {
  play.playing = false;
  if (live.playHandle !== null) {
    cancelAnimationFrame(live.playHandle);
    live.playHandle = null;
  }
}

// rAF-driven playback honouring per-frame durations (spec §3.3) and the
// loop tag's direction (§3.6). The clock is wall time, so a slow
// compose drops frames rather than slowing the animation down.
export function togglePlay() {
  if (play.playing) {
    stopPlayback();
    return;
  }
  if (!ed.doc || ed.frameCount < 2) return;
  play.playing = true;
  let cursor: PlayCursor = startCursor(playRange(), ed.currentFrame);
  setFrame(cursor.frame);
  let shownAt = performance.now();
  const step = (now: number) => {
    if (!play.playing || !ed.doc) return;
    const range = playRange();
    let duration = ed.doc.frameDuration(cursor.frame);
    // Catch up through as many frames as the elapsed time covers.
    while (now - shownAt >= duration) {
      shownAt += duration;
      cursor = advance(range, cursor);
      duration = ed.doc.frameDuration(cursor.frame);
      if (now - shownAt > 1000) shownAt = now; // tab was hidden; resync
    }
    if (cursor.frame !== ed.currentFrame) setFrame(cursor.frame);
    live.playHandle = requestAnimationFrame(step);
  };
  live.playHandle = requestAnimationFrame(step);
}

// Re-derive every piece of view / panel state that tracks the
// document after it has been replaced. Ids held from the previous
// document (stamp target, active slice / layer, tile editor) are
// meaningless against the new one and are dropped.
export function resetDocViewState() {
  stopPlayback();
  play.loopTag = null;
  applyOnion(play.onion);
  live.needsRecompose = true;
  syncMeta();
  fitView();
  syncSelection();
  ed.docRev += 1;
  toolState.stampTile = null;
  toolState.stampHover = null;
  dialogs.editingTile = null;
  ed.activeSliceId = null;
  ed.activeLayerId = null;
}

export function undo() {
  if (!ed.doc) return;
  try {
    if (ed.doc.undo()) {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
      reconcileActiveSlice();
      reconcileActiveLayerAndStamp();
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.undo'), error: msg });
  }
}

export function redo() {
  if (!ed.doc) return;
  try {
    if (ed.doc.redo()) {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
      reconcileActiveSlice();
      reconcileActiveLayerAndStamp();
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    ed.status = t('status.failed', { action: t('act.redo'), error: msg });
  }
}

// If `activeSliceId` references a slice that no longer exists
// (typical after `undo` of an `addSlice`), drop the local pointer
// so the marching-ants overlay and the panel highlight stop
// tracking a phantom id.
export function reconcileActiveSlice() {
  if (!ed.doc || ed.activeSliceId === null) return;
  const count = ed.doc.sliceCount;
  for (let i = 0; i < count; i += 1) {
    try {
      if (ed.doc.sliceIdAt(i) === ed.activeSliceId) return;
    } catch {
      // ignore
    }
  }
  ed.activeSliceId = null;
}

// Companion to `reconcileActiveSlice` for the Layers panel highlight
// and the stamp target: undo/redo can remove the referenced layer or
// tile (e.g. undoing an addTilemapLayer / addTile), leaving the local
// pointer dangling — painting would target a stale layer and the next
// stamp would throw. The wasm-side paint target already falls back
// gracefully when its layer vanishes, so only the UI state needs
// clearing.
export function reconcileActiveLayerAndStamp() {
  if (!ed.doc) return;
  if (ed.activeLayerId !== null) {
    let found = false;
    const count = ed.doc.layerCount;
    for (let i = 0; i < count; i += 1) {
      try {
        if (ed.doc.layerIdAt(i) === ed.activeLayerId) {
          found = true;
          break;
        }
      } catch {
        // ignore — treat unreadable rows as "not the active layer"
      }
    }
    if (!found) ed.activeLayerId = null;
  }
  if (toolState.stampTile) {
    try {
      if (toolState.stampTile.tileId >= ed.doc.tilesetTileCount(toolState.stampTile.tilesetId)) {
        toolState.stampTile = null;
      }
    } catch {
      toolState.stampTile = null;
    }
  }
}

// Deselect the marquee. The wasm side enqueues a `selection-changed`
// event, which the next tick drains into a repaint.
export function deselect() {
  ed.doc?.clearSelection();
}

export function selectAll() {
  ed.doc?.selectAll();
}

// Delete/Backspace: clear the pixels inside the marquee (the marquee
// stays, Aseprite-style). Recompose rides the dirty-rect event.
export function deleteSelectionPixels() {
  if (!ed.doc || !ed.selection) return;
  try {
    if (ed.doc.deleteSelection()) {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
    }
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.delete'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

// Layer commands shared by the Layers panel and the Layer menu.
export function afterLayerEdit() {
  live.needsRecompose = true;
  syncMeta();
  ed.docRev += 1;
}

export function duplicateLayer(layerId: number) {
  if (!ed.doc) return;
  try {
    const id = ed.doc.duplicateLayer(layerId);
    ed.activeLayerId = id;
    ed.doc.setActiveLayer(id);
    afterLayerEdit();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.duplicateLayer'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function mergeDownLayer(layerId: number) {
  if (!ed.doc) return;
  try {
    ed.doc.mergeDown(layerId);
    ed.activeLayerId = ed.doc.paintTargetLayer();
    afterLayerEdit();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.mergeDown'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function flattenLayers() {
  if (!ed.doc) return;
  try {
    ed.activeLayerId = ed.doc.flattenImage();
    afterLayerEdit();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.flatten'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

export function removeLayerById(layerId: number) {
  if (!ed.doc) return;
  try {
    ed.doc.removeLayer(layerId);
    if (ed.activeLayerId === layerId) ed.activeLayerId = null;
    afterLayerEdit();
  } catch (err) {
    ed.status = t('status.failed', {
      action: t('act.removeLayer'),
      error: err instanceof Error ? err.message : String(err),
    });
  }
}

// The active layer, falling back to the paint target the core picks.
export function targetLayer(): number | null {
  return ed.activeLayerId ?? ed.doc?.paintTargetLayer() ?? null;
}
