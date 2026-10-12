// Rendering: mirror wasm state, recompose, overlays and the rAF loop.

import { Canvas2DRenderer } from '../../lib/render/canvas2d-renderer';
import type { CanvasRenderer } from '../../lib/render/types';
import { WebGPURenderer } from '../../lib/render/webgpu-renderer';
import {
  type OverlayState,
  clearOverlay as clearOverlayLayer,
  overlayIsIdle,
  paintOverlays as paintOverlayLayer,
} from './overlay-painter';
import { colors, ed, live, probe, toolState, view } from './stores/editor.svelte';
import { constrainEndpoint } from './tools';

// Debug toggle (spec §4.4): `?renderer=canvas2d` forces the Canvas2D
// fallback so WebGPU can be A/B'd against it on the same build.
export const forceCanvas2d =
  typeof location !== 'undefined' &&
  new URLSearchParams(location.search).get('renderer') === 'canvas2d';

// Pick the base-layer renderer: WebGPU when available (and not forced
// off), else the universal Canvas2D fallback. WebGPURenderer.create
// resolves to null rather than throwing, so the fallback is automatic.
export async function createRenderer(target: HTMLCanvasElement): Promise<CanvasRenderer> {
  if (!forceCanvas2d) {
    const gpu = await WebGPURenderer.create(target);
    if (gpu) return gpu;
  }
  return new Canvas2DRenderer(target);
}

// Frame-time probe (F2, spec §4.4): samples kept for the rolling
// compose-cost average shown in the footer.
export const PROBE_WINDOW = 60;

// ~60 / 7 ≈ 8.5 Hz, matching Aseprite's marquee crawl rate. The
// overlay redraws once per phase step; intermediate ticks skip the
// recompose so an idle selection costs near-zero CPU.
export const MARCH_FRAMES_PER_STEP = 7;

export function syncMeta() {
  if (!ed.doc) return;
  ed.undoDepth = ed.doc.undoDepth;
  ed.redoDepth = ed.doc.redoDepth;
  ed.canvasW = ed.doc.width;
  ed.canvasH = ed.doc.height;
  ed.frameCount = ed.doc.frameCount;
  ed.currentFrame = ed.doc.currentFrame;
}

// Re-read the selection rect from the wasm side. Called after any
// event drain that may have included a `selection-changed`, and on
// doc replacement (new / open) so the overlay reflects the loaded
// sprite (always `None` today — Aseprite files do not persist
// selection, M7.8a follow-up).
export function syncSelection() {
  if (!ed.doc || !ed.doc.hasSelection) {
    ed.selection = null;
    live.selectionMaskBytes = null;
    return;
  }
  ed.selection = {
    x: ed.doc.selectionX,
    y: ed.doc.selectionY,
    w: ed.doc.selectionWidth,
    h: ed.doc.selectionHeight,
  };
  live.selectionMaskBytes = ed.doc.selectionIsRect ? null : ed.doc.selectionMask();
}

export function recompose() {
  if (!ed.doc || !live.renderer) return;
  const frame = ed.doc.compose(ed.currentFrame, 1);
  try {
    live.renderer.draw(frame);
  } finally {
    frame.free();
  }
  paintOverlays();
}

// Snapshot of everything the overlay layer paints from
// (overlay-painter.ts).
export function overlayState(): OverlayState {
  return {
    doc: ed.doc,
    tool: toolState.tool,
    color: colors.color,
    marchPhase: view.marchPhase,
    canvasW: ed.canvasW,
    canvasH: ed.canvasH,
    drag:
      live.dragStart && live.dragPreview && live.dragTool
        ? { tool: live.dragTool, start: live.dragStart, end: constrainedEndpoint() }
        : null,
    polyline:
      live.lassoPoints || toolState.polyPoints
        ? {
            points: live.lassoPoints ?? toolState.polyPoints ?? [],
            hover: toolState.polyPoints ? live.polyHover : null,
          }
        : null,
    selection: ed.selection,
    selectionMask: live.selectionMaskBytes,
    moveDelta:
      toolState.moveSelStart && toolState.moveSelPreview
        ? {
            x: toolState.moveSelPreview.x - toolState.moveSelStart.x,
            y: toolState.moveSelPreview.y - toolState.moveSelStart.y,
          }
        : { x: 0, y: 0 },
    stampTile: toolState.stampTile,
    stampHover: toolState.stampHover,
    activeSliceId: ed.activeSliceId,
  };
}

// Repaint the overlay layer on the full recompose path (the dirty fast
// path only fires when nothing would paint, per `canRecomposeDirty`).
export function paintOverlays() {
  if (view.overlay) paintOverlayLayer(view.overlay, overlayState());
}

export function clearOverlay() {
  if (view.overlay) clearOverlayLayer(view.overlay);
}

// Sub-rect blit path (M12.4). Only safe when no overlay source is live;
// `tick` checks `canRecomposeDirty()` before routing here.
export function recomposeDirty(rect: { x: number; y: number; w: number; h: number }) {
  if (!ed.doc || !live.renderer) return;
  const frame = ed.doc.composeDirty(ed.currentFrame, 1, rect.x, rect.y, rect.w, rect.h);
  try {
    live.renderer.drawDirty(frame);
  } finally {
    frame.free();
  }
  clearOverlay();
}

export function canRecomposeDirty(): boolean {
  return overlayIsIdle(overlayState());
}

// Sprite-space endpoint for the in-flight drag, Shift-constrained.
export function constrainedEndpoint(): { x: number; y: number } {
  if (!live.dragStart || !live.dragPreview) return { x: 0, y: 0 };
  return live.dragTool
    ? constrainEndpoint(live.dragTool, live.dragStart, live.dragPreview, live.dragShift)
    : live.dragPreview;
}

// Fold one compose-cost sample into the rolling window and refresh
// the derived average / max. Only called when the probe is active.
export function recordComposeSample(ms: number) {
  live.composeSamples.push(ms);
  if (live.composeSamples.length > PROBE_WINDOW) live.composeSamples.shift();
  let sum = 0;
  let max = 0;
  for (const s of live.composeSamples) {
    sum += s;
    if (s > max) max = s;
  }
  probe.composeMs = sum / live.composeSamples.length;
  probe.composeMaxMs = max;
}

export function tick() {
  if (probe.on) {
    const now = performance.now();
    if (live.lastTickTs > 0) {
      const fps = 1000 / Math.max(now - live.lastTickTs, 0.0001);
      // Exponential moving average smooths the per-frame jitter so
      // the footer reading is stable enough to eyeball under load.
      probe.fpsEma = probe.fpsEma === 0 ? fps : probe.fpsEma * 0.9 + fps * 0.1;
    }
    live.lastTickTs = now;
  }
  if (ed.doc) {
    const events = ed.doc.drainEvents();
    let selectionTouched = false;
    // Aggregate dirty events: any `dirty-canvas` (or `dirty-rect`
    // whose union with prior rects we'd rather not compute) forces
    // the full recompose path; consecutive `dirty-rect`s union into
    // a single bbox so a sub-rect blit can replay them in one
    // `composeDirty` call.
    let dirtyKind: 'none' | 'rect' | 'canvas' = 'none';
    let dirtyMinX = 0;
    let dirtyMinY = 0;
    let dirtyMaxX = 0;
    let dirtyMaxY = 0;
    for (const ev of events) {
      if (ev.kind === 'selection-changed') {
        selectionTouched = true;
      } else if (ev.kind === 'dirty-canvas') {
        dirtyKind = 'canvas';
      } else if (ev.kind === 'dirty-rect' && dirtyKind !== 'canvas') {
        const x0 = ev.x;
        const y0 = ev.y;
        const x1 = ev.x + ev.width;
        const y1 = ev.y + ev.height;
        if (dirtyKind === 'none') {
          dirtyMinX = x0;
          dirtyMinY = y0;
          dirtyMaxX = x1;
          dirtyMaxY = y1;
        } else {
          if (x0 < dirtyMinX) dirtyMinX = x0;
          if (y0 < dirtyMinY) dirtyMinY = y0;
          if (x1 > dirtyMaxX) dirtyMaxX = x1;
          if (y1 > dirtyMaxY) dirtyMaxY = y1;
        }
        dirtyKind = 'rect';
      }
      ev.free();
    }
    if (dirtyKind !== 'none') live.needsRecompose = true;
    if (selectionTouched) {
      syncSelection();
      // A bare `selection-changed` event (e.g. `setSelection` /
      // `clearSelection` from a tool) needs a repaint to show or
      // hide the marquee. Force the full path so the overlay paint
      // in `recompose()` runs — the sub-rect path skips overlays.
      live.needsRecompose = true;
      if (dirtyKind === 'none') dirtyKind = 'canvas';
    }
    // Drive the marching-ants animation: when a selection is
    // active, advance `marchPhase` once every
    // `MARCH_FRAMES_PER_STEP` ticks and force a re-render so the
    // overlay redraws. With no selection the counters reset and the
    // recompose path stays idle.
    if (
      ed.selection ||
      (live.dragStart &&
        (live.dragTool === 'selection-rect' ||
          live.dragTool === 'selection-ellipse' ||
          live.dragTool === 'slice')) ||
      live.lassoPoints ||
      toolState.polyPoints ||
      toolState.moveSelStart ||
      ed.activeSliceId !== null
    ) {
      live.marchTicks += 1;
      if (live.marchTicks >= MARCH_FRAMES_PER_STEP) {
        live.marchTicks = 0;
        view.marchPhase = (view.marchPhase + 1) & 0x3;
        // The ant-phase advance changes overlay pixels only — the
        // overlay is its own canvas (M12.5a), so repaint it directly
        // instead of forcing a full sprite recompose 8.5×/sec. When a
        // real recompose is already queued this tick, it repaints the
        // overlay itself.
        if (!live.needsRecompose) paintOverlays();
      }
    } else {
      live.marchTicks = 0;
    }
    if (live.needsRecompose) {
      live.needsRecompose = false;
      const t0 = probe.on ? performance.now() : 0;
      if (dirtyKind === 'rect' && canRecomposeDirty()) {
        recomposeDirty({
          x: dirtyMinX,
          y: dirtyMinY,
          w: dirtyMaxX - dirtyMinX,
          h: dirtyMaxY - dirtyMinY,
        });
      } else {
        recompose();
      }
      if (probe.on) recordComposeSample(performance.now() - t0);
      syncMeta();
    }
  }
  live.rafHandle = requestAnimationFrame(tick);
}

// F2: frame-time probe. Reset the window on enable so a reading
// reflects only post-toggle frames.
export function toggleProbe() {
  probe.on = !probe.on;
  live.composeSamples = [];
  probe.composeMs = 0;
  probe.composeMaxMs = 0;
  probe.fpsEma = 0;
  live.lastTickTs = 0;
}
