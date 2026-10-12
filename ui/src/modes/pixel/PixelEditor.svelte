<script lang="ts">
  import { onMount } from 'svelte';
  import { Document, loadCore } from '../../lib/core';
  import RecoveryDialog from '../../lib/components/RecoveryDialog.svelte';
  import TileEditor from './components/TileEditor.svelte';
  import TilesetPanel from './components/TilesetPanel.svelte';
  import SlicesPanel from './components/SlicesPanel.svelte';
  import LayersPanel from './components/LayersPanel.svelte';
  import TimelinePanel from './components/TimelinePanel.svelte';
  import {
    clearOverlay as clearOverlayLayer,
    overlayIsIdle,
    paintOverlays as paintOverlayLayer,
    type OverlayState,
  } from './overlay-painter';
  import {
    constrainEndpoint,
    isDragShapeTool,
    isShapeTool,
    linePoints,
    toolGroups,
    type Tool,
  } from './tools';
  import type { PinchFrame } from '../../lib/view/pinch';
  import SpriteSheetDialog, { type SpriteSheetOptions } from './components/SpriteSheetDialog.svelte';
  import {
    advance,
    startCursor,
    type OnionSettings,
    type PlayCursor,
    type PlayRange,
    type TagDirection,
  } from './playback';
  import PalettePanel from './components/PalettePanel.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import EffectDialog from '../../lib/components/EffectDialog.svelte';
  import ColorPair from '../../lib/components/ColorPair.svelte';
  import EditorFrame from '../../lib/components/layout/EditorFrame.svelte';
  import Panel from '../../lib/components/layout/Panel.svelte';
  import IconButton from '../../lib/components/layout/IconButton.svelte';
  import ToolRail from '../../lib/components/layout/ToolRail.svelte';
  import ToolVariants from '../../lib/components/layout/ToolVariants.svelte';
  import ZoomControl from '../../lib/components/layout/ZoomControl.svelte';
  import { cycleGroup, groupOf } from '../../lib/components/layout/tools';
  import {
    NATIVE_MENU_ACTIONS,
    actionFor,
    hint,
    isTypingTarget,
    toolGroupForKey,
    type ShortcutAction,
  } from '../../lib/shell/shortcuts';
  import { pixelMenus, type TransformOp } from './menus';
  import TextDialog, { type TextParams } from './components/TextDialog.svelte';
  import { loadDefaultFont } from '../../lib/text/font';
  import ResizeDialog, { type ResizeResult } from './components/ResizeDialog.svelte';
  import type { EffectDef } from '../../lib/effects/catalog';
  import {
    ensureReadPermission,
    formatFromName,
    hasFsAccess,
    pickAndOpen,
    isPngBytes,
    saveExport,
    saveBytes,
    sniffFormat,
    type OpenedFile,
    type SaveTarget,
  } from '../../lib/fs';
  import type { EditorProps } from '../../lib/shell/types';
  import { getPref, setPref } from '../../lib/idb/prefs';
  import { syncRecentMenu } from '../../lib/menu';
  import { isTauri } from '../../lib/platform';
  import { isIdbAvailable } from '../../lib/idb/db';
  import { AutosaveSession } from '../../lib/idb/autosave-session.svelte';
  import {
    listRecents,
    upsertRecent,
    type RecentFile,
  } from '../../lib/idb/recent-files';
  import { Canvas2DRenderer } from '../../lib/render/canvas2d-renderer';
  import type { CanvasRenderer } from '../../lib/render/types';
  import { WebGPURenderer } from '../../lib/render/webgpu-renderer';
  import { fitZoom } from '../../lib/view/fit';
  import { packColor, unpackColor } from '../../lib/color';
  import { t } from '../../lib/i18n/index.svelte';
  import { effectLabel } from '../../lib/effects/labels';
  import { altHeld, shiftHeld } from '../../lib/shell/modifiers.svelte';
  import RenameDialog from '../../lib/components/RenameDialog.svelte';
  import {
    colors,
    dialogs,
    ed,
    file,
    live,
    play,
    probe,
    resetPixelStore,
    toolState,
    view,
    type SelectionMode,
  } from './stores/editor.svelte';

  type ImageAction =
    | `layer:${TransformOp}`
    | `canvas:${TransformOp}`
    | 'canvas:resize'
    | 'canvas:scale'
    | 'canvas:crop';
  type SelectAction = 'all' | 'none' | 'invert' | 'expand' | 'contract';

  // Shell contract (src/App.svelte mounts one editor per session): the
  // document to open / create on mount, and the callbacks the shell
  // serves — New dialog, foreign-format re-routing, dirty mirror, and
  // the native-menu items this editor handles.
  let {
    initialFile,
    initialNew,
    onRequestNew,
    onOpenForeign,
    onDirtyChange,
    registerMenuHandlers,
  }: EditorProps = $props();

  // Module-level store: start every mount from a fresh state.
  resetPixelStore();

  // The wasm `Document` is the source of truth for sprite state
  // (CLAUDE.md §9 — "canvas-as-source-of-truth" anti-pattern). The UI
  // holds an opaque handle, paints with `applyTool`, and re-renders by
  // calling `compose()` and blitting the resulting `ComposeFrame`.
  const MIN_ZOOM = 1;
  const MAX_ZOOM = 64;
  // CSS px of breathing room left on each edge when auto-fitting a
  // sprite to the viewport (see `fitView`).
  const FIT_MARGIN = 24;

  // Release version stamped in by the release workflow; `dev` locally.
  const appVersion = import.meta.env.VITE_APP_VERSION ?? 'dev';
  // Debug toggle (spec §4.4): `?renderer=canvas2d` forces the Canvas2D
  // fallback so WebGPU can be A/B'd against it on the same build.
  const forceCanvas2d =
    typeof location !== 'undefined' &&
    new URLSearchParams(location.search).get('renderer') === 'canvas2d';

  // Pick the base-layer renderer: WebGPU when available (and not forced
  // off), else the universal Canvas2D fallback. WebGPURenderer.create
  // resolves to null rather than throwing, so the fallback is automatic.
  async function createRenderer(target: HTMLCanvasElement): Promise<CanvasRenderer> {
    if (!forceCanvas2d) {
      const gpu = await WebGPURenderer.create(target);
      if (gpu) return gpu;
    }
    return new Canvas2DRenderer(target);
  }

  // Frame-time probe (F2, spec §4.4): samples kept for the rolling
  // compose-cost average shown in the footer.
  const PROBE_WINDOW = 60;
  // Stable across the session; drives Save / Save As button labels.
  const fsAccessAvailable = hasFsAccess();
  // Canvas size cap for new / resized documents (the shell's New
  // dialog shares it).
  const MAX_DOC_SIZE = 4096;

  const tauriHost = isTauri();
  const idbAvailable = isIdbAvailable();
  const recentsAvailable = (fsAccessAvailable || tauriHost) && idbAvailable;
  // "Modified since last save" — drives the footer dot, the tab title,
  // the beforeunload warning, and the New / Open confirm guard.
  const isDirty = $derived(ed.doc !== null && ed.undoDepth !== ed.lastSavedUndoDepth);
  // Last-used foreground color, persisted across sessions (prefs IDB
  // store). Loaded once on mount; written on every change thereafter.
  const COLOR_PREF = 'lastColor';
  const BG_COLOR_PREF = 'lastBgColor';

  function selectionModeFor(e: { shiftKey: boolean; altKey: boolean }): SelectionMode {
    const shift = shiftHeld(e);
    const alt = altHeld(e);
    if (shift && alt) return 'intersect';
    if (shift) return 'add';
    if (alt) return 'subtract';
    return 'replace';
  }
  // ~60 / 7 ≈ 8.5 Hz, matching Aseprite's marquee crawl rate. The
  // overlay redraws once per phase step; intermediate ticks skip the
  // recompose so an idle selection costs near-zero CPU.
  const MARCH_FRAMES_PER_STEP = 7;

  function syncMeta() {
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
  function syncSelection() {
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

  function recompose() {
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
  function overlayState(): OverlayState {
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
          ? { points: live.lassoPoints ?? toolState.polyPoints ?? [], hover: toolState.polyPoints ? live.polyHover : null }
          : null,
      selection: ed.selection,
      selectionMask: live.selectionMaskBytes,
      moveDelta:
        toolState.moveSelStart && toolState.moveSelPreview
          ? { x: toolState.moveSelPreview.x - toolState.moveSelStart.x, y: toolState.moveSelPreview.y - toolState.moveSelStart.y }
          : { x: 0, y: 0 },
      stampTile: toolState.stampTile,
      stampHover: toolState.stampHover,
      activeSliceId: ed.activeSliceId,
    };
  }

  // Repaint the overlay layer on the full recompose path (the dirty fast
  // path only fires when nothing would paint, per `canRecomposeDirty`).
  function paintOverlays() {
    if (view.overlay) paintOverlayLayer(view.overlay, overlayState());
  }

  function clearOverlay() {
    if (view.overlay) clearOverlayLayer(view.overlay);
  }

  // Sub-rect blit path (M12.4). Only safe when no overlay source is live;
  // `tick` checks `canRecomposeDirty()` before routing here.
  function recomposeDirty(rect: { x: number; y: number; w: number; h: number }) {
    if (!ed.doc || !live.renderer) return;
    const frame = ed.doc.composeDirty(ed.currentFrame, 1, rect.x, rect.y, rect.w, rect.h);
    try {
      live.renderer.drawDirty(frame);
    } finally {
      frame.free();
    }
    clearOverlay();
  }

  function canRecomposeDirty(): boolean {
    return overlayIsIdle(overlayState());
  }

  // Sprite-space endpoint for the in-flight drag, Shift-constrained.
  function constrainedEndpoint(): { x: number; y: number } {
    if (!live.dragStart || !live.dragPreview) return { x: 0, y: 0 };
    return live.dragTool ? constrainEndpoint(live.dragTool, live.dragStart, live.dragPreview, live.dragShift) : live.dragPreview;
  }

  // Clamp + apply a new zoom level. Pan offset stays in CSS pixels,
  // so the canvas's flex-centered position keeps the sprite center
  // anchored across zoom changes; further pan offsets shift uniformly
  // from there.
  // Tools the right mouse button drives with the background color.
  const BG_TOOLS: ReadonlySet<Tool> = new Set<Tool>([
    'pencil',
    'bucket',
    'eyedropper',
    'line',
    'rectangle',
    'rectangle-fill',
    'ellipse',
    'ellipse-fill',
    'rounded-rect',
    'polygon-shape',
  ]);

  /** Packed RGBA of the color the current gesture paints with. */
  function strokeColor(): number {
    return live.useBg ? packColor(colors.bgColor, colors.bgAlpha) : packColor(colors.color, colors.alpha);
  }

  /** Packed RGBA of a filled shape's interior: the other color. */
  function shapeFillColor(): number {
    return live.useBg ? packColor(colors.color, colors.alpha) : packColor(colors.bgColor, colors.bgAlpha);
  }

  function swapColors() {
    [colors.color, colors.bgColor] = [colors.bgColor, colors.color];
    [colors.alpha, colors.bgAlpha] = [colors.bgAlpha, colors.alpha];
  }

  function resetColors() {
    colors.color = '#000000';
    colors.alpha = 255;
    colors.bgColor = '#ffffff';
    colors.bgAlpha = 255;
  }

  function setZoom(next: number) {
    next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, Math.floor(next)));
    if (next === view.zoom) return;
    view.zoom = next;
  }

  function zoomIn() {
    setZoom(view.zoom * 2);
  }

  function zoomOut() {
    setZoom(view.zoom >>> 1);
  }

  // Fit the whole sprite to the viewport: the largest integer zoom that
  // shows it with a small margin, then re-center (pan 0). Runs on every
  // document replacement (new / open / recover) so a freshly-loaded
  // sprite always lands fully in view regardless of its dimensions, and
  // backs the "Reset" control + the `0` shortcut. Falls back to the
  // historical 8× default when the stage hasn't been measured yet
  // (e.g. first paint before layout settles).
  function fitView() {
    view.zoom =
      view.stageW > 0 && view.stageH > 0
        ? fitZoom(view.stageW, view.stageH, ed.canvasW, ed.canvasH, MIN_ZOOM, MAX_ZOOM, FIT_MARGIN)
        : 8;
    view.panX = 0;
    view.panY = 0;
  }

  // 1:1 — one screen pixel per sprite pixel, re-centered.
  function zoomActual() {
    setZoom(1);
    view.panX = 0;
    view.panY = 0;
  }

  // "Reset view" — re-centers and fits the sprite to the viewport.
  // Wired to the toolbar button and the View ▸ Reset Zoom menu item.
  function resetView() {
    fitView();
  }

  // Cursor-anchored mouse-wheel zoom. The canvas is flex-centered, so it
  // scales about its own center for a fixed pan — which means keeping the
  // sprite pixel under the cursor put is a pure pan adjustment, computed
  // from the live rect with no async DOM read. Wheel up zooms in, down
  // zooms out; steps are multiplicative but nudged by ±1 so low zoom
  // levels still respond. See docs/specs/pincel.md §5 (Move/zoom).
  function onWheel(e: WheelEvent) {
    if (!view.canvas) return;
    // Ignore purely horizontal gestures (deltaY === 0): they aren't a
    // zoom intent, and swallowing them would suppress horizontal
    // trackpad scroll over the canvas.
    if (e.deltaY === 0) return;
    // Stop the gesture from scrolling the page / parent containers.
    e.preventDefault();
    const rect = view.canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const oldZoom = view.zoom;
    let next =
      e.deltaY < 0 ? Math.round(oldZoom * 1.25) : Math.round(oldZoom / 1.25);
    if (next === oldZoom) next = oldZoom + (e.deltaY < 0 ? 1 : -1);
    next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, next));
    if (next === oldZoom) return;
    const ratio = next / oldZoom;
    const rectCx = rect.left + rect.width / 2;
    const rectCy = rect.top + rect.height / 2;
    view.panX += (rectCx - e.clientX) * (ratio - 1);
    view.panY += (rectCy - e.clientY) * (ratio - 1);
    view.zoom = next;
  }


  function cancelGesture() {
    if (live.painting) {
      live.painting = false;
      ed.doc?.endStroke();
    }
    if (ed.doc && ed.doc.undoDepth > live.gestureUndoDepth) {
      ed.doc.undo();
      syncMeta();
      ed.docRev += 1;
    }
    view.panning = false;
    live.panStartClient = null;
    live.panStartOffset = null;
    toolState.moveSelStart = null;
    toolState.moveSelPreview = null;
    live.lassoPoints = null;
    live.dragStart = null;
    live.dragPreview = null;
    live.dragTool = null;
    live.needsRecompose = true;
  }

  function applyPinch(f: PinchFrame) {
    if (!view.canvas) return;
    const rect = view.canvas.getBoundingClientRect();
    const next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, Math.round(live.pinchZoom0 * f.scale)));
    if (next !== view.zoom && rect.width > 0) {
      // Same anchoring as `onWheel`: keep the sprite pixel under the
      // fingers' midpoint in place.
      const ratio = next / view.zoom;
      view.panX += (rect.left + rect.width / 2 - f.midX) * (ratio - 1);
      view.panY += (rect.top + rect.height / 2 - f.midY) * (ratio - 1);
      view.zoom = next;
    }
    view.panX += f.dx;
    view.panY += f.dy;
  }

  function spriteCoord(e: PointerEvent): { x: number; y: number } | null {
    if (!view.canvas) return null;
    const rect = view.canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    const x = Math.floor(((e.clientX - rect.left) * view.canvas.width) / rect.width);
    const y = Math.floor(((e.clientY - rect.top) * view.canvas.height) / rect.height);
    return { x, y };
  }

  // Find a tilemap layer bound to `tilesetId`. The Stamp tool needs
  // a target layer; today we auto-pick the topmost matching tilemap
  // layer (highest z-index), which matches the natural "the last
  // tilemap layer you added is the one you draw on" expectation.
  // An explicit active-layer selector lands when the Layers panel
  // ships.
  function activeTilemapLayerForTileset(tilesetId: number): number | null {
    if (!ed.doc) return null;
    for (let i = ed.doc.layerCount - 1; i >= 0; i -= 1) {
      let layerId: number;
      try {
        layerId = ed.doc.layerIdAt(i);
      } catch {
        continue;
      }
      if (
        ed.doc.layerKind(layerId) === 'tilemap' &&
        ed.doc.layerTilesetId(layerId) === tilesetId
      ) {
        return layerId;
      }
    }
    return null;
  }

  // Commit the active stamp tile onto the canvas cell under `point`.
  // Resolves the target tilemap layer, the tileset's tile size, and
  // forwards the grid cell to wasm `placeTile`. Out-of-grid clicks
  // (canvas larger than grid * tile_size) and missing layers surface
  // in the status bar rather than throwing.
  function commitStamp(point: { x: number; y: number }) {
    if (!ed.doc || !toolState.stampTile) return;
    const layerId = activeTilemapLayerForTileset(toolState.stampTile.tilesetId);
    if (layerId === null) {
      ed.status = t('px.needTilemap');
      return;
    }
    const tileW = ed.doc.tilesetTileWidth(toolState.stampTile.tilesetId);
    const tileH = ed.doc.tilesetTileHeight(toolState.stampTile.tilesetId);
    if (tileW === 0 || tileH === 0) return;
    if (point.x < 0 || point.y < 0) return;
    const gx = Math.floor(point.x / tileW);
    const gy = Math.floor(point.y / tileH);
    try {
      ed.doc.placeTile(layerId, ed.currentFrame, gx, gy, toolState.stampTile.tileId);
      live.needsRecompose = true;
      syncMeta();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      ed.status = t('status.failed', { action: t('act.stamp'), error: msg });
    }
  }


  function paintAt(e: PointerEvent) {
    if (!ed.doc) return;
    // Drag-shape tools (Line, Rectangle, …) have their own press /
    // drag / release pipeline below and commit on `pointerup`. The
    // wasm `applyTool` surface only accepts per-pixel tools, so a
    // mid-drag tool switch must not route a stroke through here.
    if (isDragShapeTool(toolState.tool) || toolState.tool === 'move') return;
    const point = spriteCoord(e);
    if (!point) return;
    if (toolState.tool === 'eyedropper') {
      // The eyedropper is read-only: sample the composed pixel and
      // bind it to the foreground color picker. Drags keep sampling
      // so the user can scrub for the exact pixel they want.
      try {
        const picked = ed.doc.pickColor(ed.currentFrame, point.x, point.y);
        if (live.useBg) {
          colors.bgColor = unpackColor(picked);
          colors.bgAlpha = picked & 0xff;
        } else {
          colors.color = unpackColor(picked);
          colors.alpha = picked & 0xff;
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('pickColor failed', err);
        ed.status = t('status.failed', { action: t('act.pick'), error: msg });
      }
      return;
    }
    const from = live.lastPaint;
    live.lastPaint = point;
    // Pixels after the first of the segment from the previous sample (the
    // first was painted then); a fresh press paints just the point.
    const pixels = from ? linePoints(from, point).slice(1) : [point];
    const packed = strokeColor();
    try {
      for (const p of pixels) {
        try {
          // The wasm eraser ignores the `color` arg, but we still pass
          // the packed foreground so the JS surface stays uniform.
          ed.doc.applyTool(toolState.tool, p.x, p.y, packed);
        } catch (err) {
          // A segment may cross the canvas edge: skip off-canvas pixels.
          const msg = err instanceof Error ? err.message : String(err);
          if (!msg.includes('pixel out of bounds')) throw err;
        }
      }
    } catch (err) {
      // Drags that leave the canvas raise PixelOutOfBounds; that is
      // expected and silenced. Anything else (missing layer, unknown
      // tool, …) is a real failure: surface it in the status bar
      // and log so it doesn't disappear.
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes('pixel out of bounds')) return;
      console.error('applyTool failed', err);
      ed.status = t('status.failed', { action: t('act.paint'), error: msg });
    }
  }


  function lockedEditTarget(): string | null {
    if (!ed.doc) return null;
    if (toolState.tool.startsWith('selection') || toolState.tool === 'eyedropper' || toolState.tool === 'slice') return null;
    if (toolState.tool === 'move' && !ed.selection) return null;
    const id = toolState.tool === 'tilemap-stamp' ? ed.activeLayerId : ed.doc.paintTargetLayer();
    if (id === null || id === 0xffffffff || !ed.doc.layerLocked(id)) return null;
    return ed.doc.layerName(id);
  }

  function onPointerDown(e: PointerEvent) {
    if (live.pinch.down(e)) {
      view.canvas?.setPointerCapture(e.pointerId);
      cancelGesture();
      live.pinchZoom0 = view.zoom;
      return;
    }
    if (live.pinch.suppressing) return;
    // Right button: the painting tools use the background color
    // (Aseprite / paint.net); every other tool ignores it.
    if (e.button === 2 && !BG_TOOLS.has(toolState.tool)) return;
    if (e.button !== 0 && e.button !== 2) return;
    live.useBg = e.button === 2;
    view.canvas?.setPointerCapture(e.pointerId);
    live.gestureUndoDepth = ed.doc?.undoDepth ?? 0;
    // Space-drag always pans, regardless of the active tool (spec §5.2
    // — Move tool "Pans canvas with space-drag"). Move-tool press
    // splits on whether a selection is active: with one, drag the
    // selection content (M7.7b); without one, fall back to viewport
    // pan (M7.7a). spaceDown wins so the user can always pan even
    // when the Move tool would otherwise translate the marquee.
    if (view.spaceDown) {
      view.panning = true;
      live.panStartClient = { x: e.clientX, y: e.clientY };
      live.panStartOffset = { x: view.panX, y: view.panY };
      return;
    }
    const locked = lockedEditTarget();
    if (locked !== null) {
      ed.status = live.lockNotice = t('layers.locked', { name: locked });
      return;
    }
    if (live.lockNotice !== null && ed.status === live.lockNotice) ed.status = '';
    live.lockNotice = null;
    if (toolState.tool === 'move') {
      if (ed.selection) {
        const point = spriteCoord(e);
        if (point) {
          toolState.moveSelStart = point;
          toolState.moveSelPreview = point;
          live.needsRecompose = true;
        }
        return;
      }
      view.panning = true;
      live.panStartClient = { x: e.clientX, y: e.clientY };
      live.panStartOffset = { x: view.panX, y: view.panY };
      return;
    }
    if (toolState.tool === 'selection-lasso') {
      const point = spriteCoord(e);
      if (!point) return;
      live.lassoPoints = [point];
      live.dragSelMode = selectionModeFor(e);
      live.needsRecompose = true;
      return;
    }
    if (toolState.tool === 'selection-polygon') {
      const point = spriteCoord(e);
      if (!point) return;
      if (!toolState.polyPoints) {
        toolState.polyPoints = [point];
        live.dragSelMode = selectionModeFor(e);
      } else if (e.detail >= 2) {
        commitPolygon();
        return;
      } else {
        toolState.polyPoints = [...toolState.polyPoints, point];
      }
      live.polyHover = point;
      live.needsRecompose = true;
      return;
    }
    if (toolState.tool === 'selection-wand') {
      commitWand(e);
      return;
    }
    if (toolState.tool === 'text') {
      const point = spriteCoord(e);
      if (point && !toolState.textAnchor) void openTextAt(point);
      return;
    }
    if (isDragShapeTool(toolState.tool)) {
      const point = spriteCoord(e);
      if (!point) return;
      live.dragStart = point;
      live.dragPreview = point;
      live.dragTool = toolState.tool;
      live.dragShift = shiftHeld(e);
      live.dragSelMode = selectionModeFor(e);
      live.needsRecompose = true;
      return;
    }
    if (toolState.tool === 'bucket') {
      // Bucket commits once per click; entering painting mode would have
      // `pointermove` re-fire `applyBucket` and push a no-op fill onto
      // the bus on every pixel of the drag.
      commitBucket(e);
      return;
    }
    if (toolState.tool === 'tilemap-stamp') {
      // Stamp commits once per click for the same reason as Bucket.
      // Drag-to-paint over tiles can land alongside auto-tile mode
      // in Phase 2 (spec §5.3 / §13.2).
      const point = spriteCoord(e);
      if (point) commitStamp(point);
      return;
    }
    live.painting = true;
    live.lastPaint = null;
    paintAt(e);
  }

  function commitBucket(e: PointerEvent) {
    if (!ed.doc) return;
    const point = spriteCoord(e);
    if (!point) return;
    try {
      ed.doc.applyBucket(point.x, point.y, strokeColor());
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('applyBucket failed', err);
      ed.status = t('status.failed', { action: t('act.bucket'), error: msg });
    }
  }

  // Commit a Slice tool drag from the two corner points of the
  // press / drag / release gesture. Routes to `addSlice` when no
  // slice is active (creating a new slice with the drawn bounds), or
  // to `setSliceKey` when a slice is active (preserving its 9-patch
  // center and pivot). Degenerate (single-pixel) drags are accepted
  // as 1×1 slices — the underlying commands reject empty bounds, so
  // a click without a drag still surfaces a valid 1-pixel slice.
  function commitSliceDrag(x0: number, y0: number, x1: number, y1: number) {
    if (!ed.doc) return;
    const minX = Math.min(x0, x1);
    const minY = Math.min(y0, y1);
    const w = Math.abs(x1 - x0) + 1;
    const h = Math.abs(y1 - y0) + 1;
    try {
      if (ed.activeSliceId === null) {
        const autoName = `Slice ${ed.doc.sliceCount + 1}`;
        const packed = packColor(colors.color, colors.alpha);
        const newId = ed.doc.addSlice(autoName, minX, minY, w, h, packed);
        ed.activeSliceId = newId;
      } else {
        const id = ed.activeSliceId;
        // Preserve the active slice's 9-patch center and pivot
        // around the new bounds. The wasm side validates the
        // partial-quartet invariant on its own.
        let keyIndex = 0;
        const keyCount = ed.doc.sliceKeyCount(id);
        for (let k = 0; k < keyCount; k += 1) {
          if (ed.doc.sliceKeyFrame(id, k) === 0) {
            keyIndex = k;
            break;
          }
        }
        const hasCenter =
          keyCount > 0 ? ed.doc.sliceKeyHasCenter(id, keyIndex) : false;
        const hasPivot =
          keyCount > 0 ? ed.doc.sliceKeyHasPivot(id, keyIndex) : false;
        ed.doc.setSliceKey(
          id,
          0,
          minX,
          minY,
          w,
          h,
          hasCenter ? ed.doc.sliceKeyCenterX(id, keyIndex) : undefined,
          hasCenter ? ed.doc.sliceKeyCenterY(id, keyIndex) : undefined,
          hasCenter ? ed.doc.sliceKeyCenterWidth(id, keyIndex) : undefined,
          hasCenter ? ed.doc.sliceKeyCenterHeight(id, keyIndex) : undefined,
          hasPivot ? ed.doc.sliceKeyPivotX(id, keyIndex) : undefined,
          hasPivot ? ed.doc.sliceKeyPivotY(id, keyIndex) : undefined,
        );
      }
      ed.docRev += 1;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('slice commit failed', err);
      ed.status = t('status.failed', { action: t('act.slice'), error: msg });
    }
  }

  // Commit a selection marquee from the two corner points of the
  // press / drag / release gesture. A no-move click (start === end)
  // clears the selection, matching Aseprite's "click outside to
  // deselect" UX; otherwise the rect is normalized to min / max
  // corners (inclusive) and forwarded to `setSelection`. Sprite-bounds
  // clipping is intentionally not applied here — the wasm side stores
  // the raw rect and the marching-ants overlay's per-pixel clip keeps
  // off-canvas extents from leaking visually.
  function commitSelection(x0: number, y0: number, x1: number, y1: number) {
    if (!ed.doc) return;
    if (x0 === x1 && y0 === y1 && live.dragSelMode === 'replace') {
      ed.doc.clearSelection();
      return;
    }
    const minX = Math.min(x0, x1);
    const minY = Math.min(y0, y1);
    const w = Math.abs(x1 - x0) + 1;
    const h = Math.abs(y1 - y0) + 1;
    ed.doc.selectRect(minX, minY, w, h, live.dragSelMode);
  }

  // Ellipse marquee: same inclusive-corner gesture as the rect, combined
  // per the modifier mode captured on pointerdown.
  function commitEllipseSelection(x0: number, y0: number, x1: number, y1: number) {
    if (!ed.doc) return;
    if (x0 === x1 && y0 === y1 && live.dragSelMode === 'replace') {
      ed.doc.clearSelection();
      return;
    }
    const minX = Math.min(x0, x1);
    const minY = Math.min(y0, y1);
    const w = Math.abs(x1 - x0) + 1;
    const h = Math.abs(y1 - y0) + 1;
    ed.doc.selectEllipse(minX, minY, w, h, live.dragSelMode);
  }

  // Lasso / polygon commit. Fewer than three vertices is a click: in
  // replace mode that deselects (like the rect click), otherwise nothing.
  function commitPolygonPoints(pts: { x: number; y: number }[], mode: SelectionMode) {
    if (!ed.doc) return;
    try {
      if (pts.length < 3) {
        if (mode === 'replace') ed.doc.clearSelection();
        return;
      }
      const flat = new Int32Array(pts.length * 2);
      pts.forEach((p, i) => {
        flat[i * 2] = p.x;
        flat[i * 2 + 1] = p.y;
      });
      ed.doc.selectPolygon(flat, mode);
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.selection'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function commitPolygon() {
    const pts = toolState.polyPoints;
    toolState.polyPoints = null;
    live.polyHover = null;
    if (pts) commitPolygonPoints(pts, live.dragSelMode);
    live.needsRecompose = true;
  }

  function cancelPolygon() {
    toolState.polyPoints = null;
    live.polyHover = null;
    live.lassoPoints = null;
    live.needsRecompose = true;
  }

  function commitWand(e: PointerEvent) {
    if (!ed.doc) return;
    const point = spriteCoord(e);
    if (!point) return;
    try {
      ed.doc.selectWand(
        point.x,
        point.y,
        Math.max(0, Math.min(255, Math.round(toolState.wandTolerance))),
        toolState.wandContiguous,
        toolState.wandSample,
        selectionModeFor(e),
      );
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.wand'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function selectAction(action: SelectAction) {
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
          ed.doc.expandSelection(1);
          break;
        case 'contract':
          ed.doc.contractSelection(1);
          break;
      }
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.selection'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function onPointerMove(e: PointerEvent) {
    const frame = live.pinch.move(e);
    if (frame) {
      applyPinch(frame);
      return;
    }
    if (live.pinch.suppressing) return;
    if (view.panning && live.panStartClient && live.panStartOffset) {
      view.panX = live.panStartOffset.x + (e.clientX - live.panStartClient.x);
      view.panY = live.panStartOffset.y + (e.clientY - live.panStartClient.y);
      return;
    }
    if (toolState.moveSelStart) {
      const point = spriteCoord(e);
      if (!point) return;
      toolState.moveSelPreview = point;
      live.needsRecompose = true;
      return;
    }
    if (live.lassoPoints) {
      const point = spriteCoord(e);
      if (!point) return;
      const last = live.lassoPoints[live.lassoPoints.length - 1];
      if (!last || last.x !== point.x || last.y !== point.y) {
        live.lassoPoints.push(point);
        live.needsRecompose = true;
      }
      return;
    }
    if (toolState.polyPoints) {
      const point = spriteCoord(e);
      if (!point) return;
      live.polyHover = point;
      live.needsRecompose = true;
      return;
    }
    if (live.dragStart) {
      const point = spriteCoord(e);
      if (!point) return;
      live.dragPreview = point;
      live.dragShift = shiftHeld(e);
      live.needsRecompose = true;
      return;
    }
    if (toolState.tool === 'tilemap-stamp') {
      // Update the hover indicator regardless of button state — the
      // overlay should follow the cursor whether or not the user is
      // mid-click. `dirty = true` triggers a recompose so the grid
      // overlay redraws at the new cell.
      const point = spriteCoord(e);
      toolState.stampHover = point;
      live.needsRecompose = true;
      return;
    }
    if (!live.painting) return;
    paintAt(e);
  }

  function onPointerLeave() {
    if (toolState.tool === 'tilemap-stamp' && toolState.stampHover) {
      toolState.stampHover = null;
      live.needsRecompose = true;
    }
  }

  function onPointerUp(e: PointerEvent) {
    if (view.canvas?.hasPointerCapture(e.pointerId)) {
      view.canvas.releasePointerCapture(e.pointerId);
    }
    const pinched = live.pinch.suppressing;
    live.pinch.up(e);
    if (pinched) return;
    if (view.panning) {
      view.panning = false;
      live.panStartClient = null;
      live.panStartOffset = null;
      return;
    }
    if (toolState.moveSelStart && toolState.moveSelPreview && ed.doc) {
      const dx = toolState.moveSelPreview.x - toolState.moveSelStart.x;
      const dy = toolState.moveSelPreview.y - toolState.moveSelStart.y;
      if (dx !== 0 || dy !== 0) {
        try {
          ed.doc.applyMoveSelection(dx, dy);
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          console.error('applyMoveSelection failed', err);
          ed.status = t('status.failed', { action: t('act.move'), error: msg });
        }
      }
      toolState.moveSelStart = null;
      toolState.moveSelPreview = null;
      live.needsRecompose = true;
      syncMeta();
      return;
    }
    if (live.lassoPoints && ed.doc) {
      const pts = live.lassoPoints;
      live.lassoPoints = null;
      commitPolygonPoints(pts, live.dragSelMode);
      live.needsRecompose = true;
      return;
    }
    if (live.dragStart && live.dragPreview && live.dragTool && ed.doc) {
      live.dragShift = shiftHeld(e);
      const end = constrainedEndpoint();
      const packed = strokeColor();
      try {
        if (live.dragTool === 'line') {
          ed.doc.applyLine(live.dragStart.x, live.dragStart.y, end.x, end.y, packed);
        } else if (isShapeTool(live.dragTool)) {
          const kind =
            live.dragTool === 'rectangle'
              ? 'rectangle'
              : live.dragTool === 'ellipse'
                ? 'ellipse'
                : live.dragTool === 'rounded-rect'
                  ? 'rounded_rectangle'
                  : 'polygon';
          const param = live.dragTool === 'rounded-rect' ? toolState.cornerRadius : toolState.polygonSides;
          ed.doc.applyShape(
            kind,
            live.dragStart.x,
            live.dragStart.y,
            end.x,
            end.y,
            toolState.shapeMode,
            Math.max(1, Math.round(toolState.strokeWidth)),
            packed,
            shapeFillColor(),
            param,
          );
        } else if (live.dragTool === 'rectangle-fill') {
          ed.doc.applyRectangle(live.dragStart.x, live.dragStart.y, end.x, end.y, packed, true);
        } else if (live.dragTool === 'ellipse-fill') {
          ed.doc.applyEllipse(live.dragStart.x, live.dragStart.y, end.x, end.y, packed, true);
        } else if (live.dragTool === 'selection-rect') {
          commitSelection(live.dragStart.x, live.dragStart.y, end.x, end.y);
        } else if (live.dragTool === 'selection-ellipse') {
          commitEllipseSelection(live.dragStart.x, live.dragStart.y, end.x, end.y);
        } else if (live.dragTool === 'slice') {
          commitSliceDrag(live.dragStart.x, live.dragStart.y, end.x, end.y);
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error(`${live.dragTool} failed`, err);
        ed.status = t('px.toolFailed', { tool: live.dragTool, error: msg });
      }
      live.dragStart = null;
      live.dragPreview = null;
      live.dragTool = null;
      live.dragShift = false;
      live.needsRecompose = true;
      syncMeta();
      return;
    }
    if (live.painting) {
      live.painting = false;
      // Close the gesture on the undo bus so this drag stays one undo
      // entry and the next stroke starts a fresh one.
      ed.doc?.endStroke();
    }
  }

  // wasm-bindgen classes own Rust-side allocations; freeing the prior
  // `Document` before replacing it avoids leaking memory across
  // repeated New / Open operations. Safe to call when `doc` is null.
  function disposeDoc() {
    if (ed.doc) {
      ed.doc.free();
      ed.doc = null;
    }
  }

  // Re-derive every piece of view / panel state that tracks the
  // document after it has been replaced. Ids held from the previous
  // document (stamp target, active slice / layer, tile editor) are
  // meaningless against the new one and are dropped.
  // Switch the viewed / edited frame. Clamped; the wasm side emits a
  // dirty-canvas event that drives the recompose.
  function setFrame(next: number) {
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

  function addLayer() {
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
      ed.status = t('status.failed', { action: t('act.addLayer'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function addFrame() {
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
      ed.status = t('status.failed', { action: t('act.addFrame'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function removeFrame(frame: number) {
    if (!ed.doc) return;
    try {
      ed.doc.removeFrame(frame);
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.removeFrame'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function applyOnion(next: OnionSettings) {
    play.onion = next;
    ed.doc?.setOnionSkin(next.enabled, next.back, next.forward, next.alpha, next.alpha);
  }

  // The range playback loops over: the chosen tag, or every frame.
  function playRange(): PlayRange {
    if (ed.doc && play.loopTag !== null && play.loopTag < ed.doc.tagCount) {
      return {
        from: ed.doc.tagFrom(play.loopTag),
        to: ed.doc.tagTo(play.loopTag),
        direction: ed.doc.tagDirection(play.loopTag) as TagDirection,
      };
    }
    return { from: 0, to: Math.max(0, ed.frameCount - 1), direction: 'forward' };
  }

  function stopPlayback() {
    play.playing = false;
    if (live.playHandle !== null) {
      cancelAnimationFrame(live.playHandle);
      live.playHandle = null;
    }
  }

  // rAF-driven playback honouring per-frame durations (spec §3.3) and the
  // loop tag's direction (§3.6). The clock is wall time, so a slow
  // compose drops frames rather than slowing the animation down.
  function togglePlay() {
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

  function resetDocViewState() {
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

  // Run `action` immediately when the document is clean; otherwise park
  // it behind the inline "discard unsaved changes?" confirm dialog.
  function guardUnsaved(action: () => void) {
    if (!isDirty) {
      action();
      return;
    }
    dialogs.confirm = {
      message: t('px.discardNamed', { name: file.saveTarget.name }),
      run: action,
    };
  }

  function confirmDiscard() {
    const pending = dialogs.confirm;
    dialogs.confirm = null;
    pending?.run();
  }

  // The New dialog lives in the shell (it offers both editor modes and
  // runs the unsaved-changes guard from the mirrored dirty state).
  function openNewDialog() {
    if (file.busy) return;
    onRequestNew();
  }

  async function openDoc() {
    if (file.busy) return;
    file.busy = true;
    try {
      await openDocInner();
    } finally {
      file.busy = false;
    }
  }

  async function openDocInner() {
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
      onOpenForeign(opened);
      return;
    }
    await openOpened(opened);
  }

  // Replace the current document with the already-read `opened` file
  // (Open…, or the shell's initial file on mount).
  async function openOpened(opened: OpenedFile) {
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

  async function save(opts: { forceAs?: boolean } = {}) {
    if (!ed.doc || file.busy) return;
    const forceAs = opts.forceAs ?? false;
    file.busy = true;
    try {
      const bytes = new Uint8Array(ed.doc.saveAseprite());
      const next = await saveBytes(bytes, file.saveTarget, { forceAs });
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
  const autosave = new AutosaveSession({
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
      const act = action === 'autosave' ? 'act.autosave' : action === 'recover' ? 'act.recover' : 'act.discard';
      ed.status = t('status.failed', { action: t(act), error });
    },
  });

  // Persist the current `saveTarget` to the recent-files registry and
  // refresh the in-memory list. No-op when the registry isn't
  // available (non-FSA browsers / IDB disabled) or the current target
  // doesn't carry a handle (a download-only save, or a New document
  // that hasn't been written yet).
  async function recordRecent() {
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

  async function openRecent(r: RecentFile) {
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
        bytes =
          raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
        name = r.name;
        nextTarget = { name: r.name, handle: null, path: r.path };
      } else if (r.handle) {
        // Read access is enough to open. The next save will prompt
        // for write via `saveBytes` → `ensureReadWritePermission`.
        if (!(await ensureReadPermission(r.handle))) {
          ed.status = t('px.recentDenied', { name: r.name });
          return;
        }
        const file = await r.handle.getFile();
        bytes = new Uint8Array(await file.arrayBuffer());
        name = file.name;
        nextTarget = { name: file.name, handle: r.handle, path: null };
      } else {
        ed.status = t('px.recentNoHandle', { name: r.name });
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
      ed.status = t('px.recentFailed', { name: r.name, error: msg });
    } finally {
      file.busy = false;
    }
  }


  function openEffect(def: EffectDef) {
    if (!ed.doc) return;
    dialogs.effect = def;
  }

  function previewEffectParams(params: number[]) {
    if (!ed.doc || !live.renderer || !dialogs.effect) return;
    if (params.length === 0 && dialogs.effect.params.length > 0) {
      recompose();
      return;
    }
    try {
      const frame = ed.doc.previewEffect(dialogs.effect.id, Float64Array.from(params), 1);
      try {
        live.renderer.draw(frame);
      } finally {
        frame.free();
      }
      paintOverlays();
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.preview'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function applyEffectParams(params: number[]) {
    if (!ed.doc || !dialogs.effect) return;
    const def = dialogs.effect;
    dialogs.effect = null;
    try {
      if (ed.doc.applyEffect(def.id, Float64Array.from(params))) {
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
        ed.status = t('px.effectApplied', { effect: effectLabel(def) });
      } else {
        ed.status = t('px.effectOutside', { effect: effectLabel(def) });
        recompose();
      }
    } catch (err) {
      ed.status = t('px.effectFailed', { effect: effectLabel(def), error: err instanceof Error ? err.message : String(err) });
      recompose();
    }
  }

  function cancelEffect() {
    dialogs.effect = null;
    recompose();
  }


  function afterImageCommand(label: string) {
    live.needsRecompose = true;
    syncMeta();
    ed.docRev += 1;
    ed.status = label;
  }

  function imageAction(action: ImageAction) {
    if (!ed.doc) return;
    try {
      switch (action) {
        case 'layer:flip_horizontal':
        case 'layer:flip_vertical':
        case 'layer:rotate_90_cw':
        case 'layer:rotate_90_ccw':
        case 'layer:rotate_180':
          ed.doc.transformLayer(action.slice('layer:'.length));
          afterImageCommand(t('px.layerTransformed'));
          break;
        case 'canvas:flip_horizontal':
        case 'canvas:flip_vertical':
        case 'canvas:rotate_90_cw':
        case 'canvas:rotate_90_ccw':
        case 'canvas:rotate_180':
          ed.doc.transformCanvas(action.slice('canvas:'.length));
          afterImageCommand(t('px.canvasTransformed'));
          break;
        case 'canvas:resize':
          dialogs.resize = 'resize';
          break;
        case 'canvas:scale':
          dialogs.resize = 'scale';
          break;
        case 'canvas:crop':
          if (ed.doc.cropToSelection()) afterImageCommand(t('px.cropped'));
          else ed.status = t('px.cropNone');
          break;
      }
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.imageCommand'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  // Import a PNG as a new layer on top of the current document.
  async function importPngLayer() {
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
      ed.status = t('status.failed', { action: t('menu.importPng'), error: err instanceof Error ? err.message : String(err) });
    } finally {
      file.busy = false;
    }
  }

  // Export the current frame's visible composite as a PNG (spec §7.3).
  async function exportPng() {
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
      ed.status = t('status.failed', { action: t('act.export'), error: err instanceof Error ? err.message : String(err) });
    } finally {
      file.busy = false;
    }
  }


  function openSheetDialog() {
    if (ed.doc && !file.busy) dialogs.sheet = true;
  }

  const sheetTags = $derived.by(() => {
    void ed.docRev;
    if (!ed.doc) return [];
    const out: { name: string; from: number; to: number }[] = [];
    for (let i = 0; i < ed.doc.tagCount; i++) {
      out.push({ name: ed.doc.tagName(i), from: ed.doc.tagFrom(i), to: ed.doc.tagTo(i) });
    }
    return out;
  });

  async function exportSheet(options: SpriteSheetOptions) {
    dialogs.sheet = false;
    if (!ed.doc || file.busy) return;
    file.busy = true;
    try {
      const base = file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '') + (options.tag ? `-${options.tag}` : '');
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
      ed.status = t('status.failed', { action: t('act.sheetExport'), error: err instanceof Error ? err.message : String(err) });
    } finally {
      file.busy = false;
    }
  }

  function submitResize(result: ResizeResult) {
    dialogs.resize = null;
    if (!ed.doc) return;
    try {
      if (result.mode === 'resize') {
        ed.doc.resizeCanvas(result.width, result.height, result.anchor);
        afterImageCommand(t('px.resized', { w: result.width, h: result.height }));
      } else {
        ed.doc.scaleImage(result.width, result.height, result.interpolation);
        afterImageCommand(t('px.scaled', { w: result.width, h: result.height }));
      }
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.resize'), error: err instanceof Error ? err.message : String(err) });
    }
  }


  async function openTextAt(point: { x: number; y: number }) {
    if (!ed.doc) return;
    try {
      live.textFontId = await loadDefaultFont();
      toolState.textAnchor = point;
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.textFont'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function previewTextParams(p: TextParams) {
    if (!ed.doc || !live.renderer || !toolState.textAnchor || live.textFontId === null) return;
    if (p.text.length === 0) {
      recompose();
      return;
    }
    try {
      const frame = ed.doc.previewText(
        live.textFontId,
        p.text,
        toolState.textAnchor.x,
        toolState.textAnchor.y,
        p.size,
        packColor(colors.color, colors.alpha),
        p.bold,
        p.italic,
        p.antiAlias,
        p.align,
        1,
      );
      try {
        live.renderer.draw(frame);
      } finally {
        frame.free();
      }
      paintOverlays();
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.textPreview'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function applyTextParams(p: TextParams) {
    if (!ed.doc || !toolState.textAnchor || live.textFontId === null) return;
    const anchor = toolState.textAnchor;
    toolState.textAnchor = null;
    try {
      const placed = ed.doc.drawText(
        live.textFontId,
        p.text,
        anchor.x,
        anchor.y,
        p.size,
        packColor(colors.color, colors.alpha),
        p.bold,
        p.italic,
        p.antiAlias,
        p.align,
      );
      if (placed) {
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
        ed.status = t('px.textPlaced');
      } else {
        ed.status = t('px.textMissed');
        recompose();
      }
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.text'), error: err instanceof Error ? err.message : String(err) });
      recompose();
    }
  }

  function cancelText() {
    toolState.textAnchor = null;
    recompose();
  }

  function undo() {
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

  function redo() {
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
  function reconcileActiveSlice() {
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
  function reconcileActiveLayerAndStamp() {
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

  // Fold one compose-cost sample into the rolling window and refresh
  // the derived average / max. Only called when the probe is active.
  function recordComposeSample(ms: number) {
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

  function tick() {
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
          (live.dragTool === 'selection-rect' || live.dragTool === 'selection-ellipse' || live.dragTool === 'slice')) ||
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

  // Filter space presses originating in any form input or
  // contenteditable element so the user keeps native space-bar behavior
  // there (typing a literal space in a future search / filename box,
  // toggling a checkbox, etc.). The current toolbar only exposes a
  // color input (no text intake) and a hidden file input, but the
  // guard is forward-looking and conservative — any `<input>`,
  // `<textarea>`, or contenteditable target opts out.
  function isEditableTarget(target: EventTarget | null): boolean {
    return isTypingTarget(target);
  }

  // Deselect the marquee. The wasm side enqueues a `selection-changed`
  // event, which the next tick drains into a repaint.
  function deselect() {
    ed.doc?.clearSelection();
  }

  function selectAll() {
    ed.doc?.selectAll();
  }

  // Delete/Backspace: clear the pixels inside the marquee (the marquee
  // stays, Aseprite-style). Recompose rides the dirty-rect event.
  function deleteSelectionPixels() {
    if (!ed.doc || !ed.selection) return;
    try {
      if (ed.doc.deleteSelection()) {
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      }
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.delete'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  // Layer commands shared by the Layers panel and the Layer menu.
  function afterLayerEdit() {
    live.needsRecompose = true;
    syncMeta();
    ed.docRev += 1;
  }

  function duplicateLayer(layerId: number) {
    if (!ed.doc) return;
    try {
      const id = ed.doc.duplicateLayer(layerId);
      ed.activeLayerId = id;
      ed.doc.setActiveLayer(id);
      afterLayerEdit();
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.duplicateLayer'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function mergeDownLayer(layerId: number) {
    if (!ed.doc) return;
    try {
      ed.doc.mergeDown(layerId);
      ed.activeLayerId = ed.doc.paintTargetLayer();
      afterLayerEdit();
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.mergeDown'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function flattenLayers() {
    if (!ed.doc) return;
    try {
      ed.activeLayerId = ed.doc.flattenImage();
      afterLayerEdit();
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.flatten'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  function removeLayerById(layerId: number) {
    if (!ed.doc) return;
    try {
      ed.doc.removeLayer(layerId);
      if (ed.activeLayerId === layerId) ed.activeLayerId = null;
      afterLayerEdit();
    } catch (err) {
      ed.status = t('status.failed', { action: t('act.removeLayer'), error: err instanceof Error ? err.message : String(err) });
    }
  }

  // The active layer, falling back to the paint target the core picks.
  function targetLayer(): number | null {
    return ed.activeLayerId ?? ed.doc?.paintTargetLayer() ?? null;
  }

  // Merge Down needs an image layer directly below an image layer.
  const canMergeDown = $derived.by(() => {
    void ed.docRev;
    const id = ed.activeLayerId;
    if (!ed.doc || id === null) return false;
    for (let i = 1; i < ed.doc.layerCount; i += 1) {
      if (ed.doc.layerIdAt(i) !== id) continue;
      return ed.doc.layerKind(id) === 'image' && ed.doc.layerKind(ed.doc.layerIdAt(i - 1)) === 'image';
    }
    return false;
  });


  const layerCountNow = $derived.by(() => {
    void ed.docRev;
    return ed.doc?.layerCount ?? 0;
  });

  const railGroups = $derived(toolGroups(toolState.stampTile));
  const toolLabel = $derived(
    groupOf(railGroups, toolState.tool)?.tools.find((x) => x.id === toolState.tool)?.label ?? '',
  );

  const menus = $derived(
    pixelMenus({
      hasDoc: ed.doc !== null,
      busy: file.busy,
      canSaveInPlace: fsAccessAvailable || tauriHost,
      recents: recentsAvailable ? file.recents : null,
      canUndo: ed.undoDepth > 0,
      canRedo: ed.redoDepth > 0,
      hasSelection: ed.selection !== null,
      canMergeDown,
      layerCount: layerCountNow,
      frameCount: ed.frameCount,
      currentFrame: ed.currentFrame,
      playing: play.playing,
      probeOn: probe.on,
      newDoc: openNewDialog,
      open: () => guardUnsaved(() => void openDoc()),
      openRecent: (r) => guardUnsaved(() => void openRecent(r)),
      save: (asNew) => void save({ forceAs: asNew }),
      importPng: () => void importPngLayer(),
      exportPng: () => void exportPng(),
      exportSheet: openSheetDialog,
      undo,
      redo,
      deleteSelection: deleteSelectionPixels,
      swapColors,
      resetColors,
      selectAll: () => selectAction('all'),
      deselect: () => selectAction('none'),
      invertSelection: () => selectAction('invert'),
      growSelection: (d) => selectAction(d > 0 ? 'expand' : 'contract'),
      transformCanvas: (op) => imageAction(`canvas:${op}`),
      transformLayer: (op) => imageAction(`layer:${op}`),
      resizeCanvas: () => imageAction('canvas:resize'),
      scaleImage: () => imageAction('canvas:scale'),
      crop: () => imageAction('canvas:crop'),
      addLayer,
      duplicateLayer: () => {
        const id = targetLayer();
        if (id !== null) duplicateLayer(id);
      },
      removeLayer: () => {
        const id = targetLayer();
        if (id !== null) removeLayerById(id);
      },
      mergeDown: () => {
        const id = targetLayer();
        if (id !== null) mergeDownLayer(id);
      },
      flatten: flattenLayers,
      renameLayer: () => {
        const id = targetLayer();
        if (id !== null && ed.doc) dialogs.renaming = { id, name: ed.doc.layerName(id) };
      },
      togglePlay,
      stepFrame: (d) => {
        stopPlayback();
        setFrame(ed.currentFrame + d);
      },
      addFrame,
      removeFrame: () => removeFrame(ed.currentFrame),
      openEffect,
      zoomIn,
      zoomOut,
      zoomFit: resetView,
      zoomActual,
      toggleProbe,
    }),
  );

  function onKeyDown(e: KeyboardEvent) {
    if (e.code === 'Space' && !e.repeat && !isEditableTarget(e.target)) {
      // Prevent the browser from page-scrolling on space.
      e.preventDefault();
      view.spaceDown = true;
      return;
    }
    // Dialogs own the keyboard while open.
    if (dialogs.effect || toolState.textAnchor || dialogs.resize || dialogs.sheet || autosave.snapshots.length > 0) return;
    if (e.key === 'Escape') {
      if (dialogs.confirm) {
        dialogs.confirm = null;
        return;
      }
      if (toolState.polyPoints || live.lassoPoints) {
        cancelPolygon();
        return;
      }
      if (ed.selection && !isEditableTarget(e.target)) {
        deselect();
        return;
      }
    }
    if (isEditableTarget(e.target)) return;
    if (e.key === 'Enter' && toolState.polyPoints) {
      e.preventDefault();
      commitPolygon();
      return;
    }
    // F2 toggles the frame-time probe (M12.6).
    if (e.key === 'F2') {
      e.preventDefault();
      toggleProbe();
      return;
    }
    // The keyboard map both modes share (lib/shell/shortcuts.ts). In the
    // desktop app the native menu owns its accelerators (New / Open /
    // Save / Export / Undo / Redo) — handling them here too would
    // double-fire.
    const action = actionFor(e);
    if (action !== null) {
      if (tauriHost && NATIVE_MENU_ACTIONS.has(action)) return;
      if (runShortcut(action)) e.preventDefault();
      return;
    }
    // Single-key tool selection; repeated presses cycle a group's variants.
    const group = toolGroupForKey(e);
    const next = group ? cycleGroup(railGroups, group, toolState.tool) : null;
    if (next) {
      e.preventDefault();
      toolState.tool = next;
    }
  }

  // F2: frame-time probe. Reset the window on enable so a reading
  // reflects only post-toggle frames.
  function toggleProbe() {
    probe.on = !probe.on;
    live.composeSamples = [];
    probe.composeMs = 0;
    probe.composeMaxMs = 0;
    probe.fpsEma = 0;
    live.lastTickTs = 0;
  }

  /** Runs a shared shortcut; false when it does not apply right now. */
  function runShortcut(action: ShortcutAction): boolean {
    switch (action) {
      case 'open':
        guardUnsaved(() => void openDoc());
        return true;
      case 'save':
      case 'saveAs':
        void save({ forceAs: action === 'saveAs' });
        return true;
      case 'export':
        void exportPng();
        return true;
      case 'exportSheet':
        openSheetDialog();
        return true;
      case 'undo':
        undo();
        return true;
      case 'redo':
        redo();
        return true;
      case 'swapColors':
        swapColors();
        return true;
      case 'resetColors':
        resetColors();
        return true;
    }
    if (!ed.doc) return false;
    switch (action) {
      case 'selectAll':
        selectAll();
        return true;
      case 'deselect':
        deselect();
        return true;
      case 'invertSelection':
        selectAction('invert');
        return true;
      case 'deleteSelection':
        if (!ed.selection) return false;
        deleteSelectionPixels();
        return true;
      case 'zoomIn':
        zoomIn();
        return true;
      case 'zoomOut':
        zoomOut();
        return true;
      case 'zoomFit':
        resetView();
        return true;
      case 'zoomActual':
        zoomActual();
        return true;
      case 'prevFrame':
      case 'nextFrame':
        stopPlayback();
        setFrame(ed.currentFrame + (action === 'nextFrame' ? 1 : -1));
        return true;
      case 'togglePlay':
        togglePlay();
        return true;
      default:
        return false;
    }
  }

  function onKeyUp(e: KeyboardEvent) {
    if (e.code === 'Space') {
      view.spaceDown = false;
    }
  }

  // If the window loses focus (alt-tab, DevTools, OS shortcut) between
  // a Space keydown and keyup, the keyup never reaches us and
  // `spaceDown` stays stuck — and any in-flight pan drag would similarly
  // outlive the gesture. Clear both on blur / hidden so the user
  // returns to a clean state.
  function onWindowBlur() {
    view.spaceDown = false;
    view.panning = false;
    live.panStartClient = null;
    live.panStartOffset = null;
    toolState.moveSelStart = null;
    toolState.moveSelPreview = null;
  }

  function onVisibilityChange() {
    if (document.hidden) onWindowBlur();
  }

  onMount(() => {
    let cancelled = false;
    loadCore()
      .then(async () => {
        if (cancelled) return;
        // Pick the base-layer renderer before the first frame. The base
        // canvas is bound by the time onMount runs (spec §4.4, M12.5).
        if (view.canvas && !live.renderer) {
          const r = await createRenderer(view.canvas);
          if (cancelled) {
            r.destroy();
            return;
          }
          live.renderer = r;
          ed.backend = r.backend;
        }
        const w = Math.max(1, Math.min(MAX_DOC_SIZE, Math.floor(initialNew?.width ?? 64)));
        const h = Math.max(1, Math.min(MAX_DOC_SIZE, Math.floor(initialNew?.height ?? 64)));
        ed.doc = new Document(w, h);
        syncMeta();
        fitView();
        syncSelection();
        live.needsRecompose = true;
        ed.lastSavedUndoDepth = ed.doc.undoDepth;
        ed.status = t('px.ready');
        live.rafHandle = requestAnimationFrame(tick);
        // The shell hands over a file it already read (start screen,
        // drag-and-drop, file association, Open Recent).
        if (initialFile) {
          file.busy = true;
          try {
            await openOpened(initialFile);
          } finally {
            file.busy = false;
          }
        }
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const msg = err instanceof Error ? err.message : String(err);
        console.error('loadCore failed', err);
        ed.status = t('status.failed', { action: t('act.wasmInit'), error: msg });
      });
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    window.addEventListener('blur', onWindowBlur);
    window.addEventListener('beforeunload', onBeforeUnload);
    document.addEventListener('visibilitychange', onVisibilityChange);
    // Wheel zoom is registered imperatively (not via `onwheel`) so the
    // listener is non-passive and `preventDefault()` actually suppresses
    // page scroll. `canvas` is bound by the time onMount runs.
    view.canvas?.addEventListener('wheel', onWheel, { passive: false });
    // Restore the last-used foreground color (best-effort; the default
    // stands when the pref is missing or IDB is unavailable). The
    // persist effect below only starts writing once this load settled,
    // so a slow IDB can't overwrite the stored value with the default.
    if (idbAvailable) {
      getPref(COLOR_PREF)
        .then((v) => {
          if (cancelled) return;
          if (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v)) colors.color = v;
          return getPref(BG_COLOR_PREF);
        })
        .then((v) => {
          if (cancelled) return;
          if (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v)) colors.bgColor = v;
          live.colorPrefLoaded = true;
        })
        .catch(() => {
          live.colorPrefLoaded = true;
        });
    }
    // Best-effort recents load — failures are silent (the dropdown
    // just stays empty / hidden).
    if (recentsAvailable) {
      listRecents()
        .then((rows) => {
          if (!cancelled) file.recents = rows;
        })
        .catch((err: unknown) => {
          console.error('listRecents failed', err);
        });
    }
    // Recovery probe and the autosave timer. A tick before `loadCore`
    // resolves is a no-op (no document yet).
    autosave.start();
    // Native-menu items this editor serves (Tauri only; the shell owns
    // New / Open / Open Recent and the open-file events).
    registerMenuHandlers({
      'menu:save': () => save(),
      'menu:saveAs': () => save({ forceAs: true }),
      'menu:exportPng': () => void exportPng(),
      'menu:exportSheet': openSheetDialog,
      'menu:undo': undo,
      'menu:redo': redo,
      'menu:zoomIn': zoomIn,
      'menu:zoomOut': zoomOut,
      'menu:resetZoom': resetView,
    });
    return () => {
      cancelled = true;
      if (live.rafHandle !== null) cancelAnimationFrame(live.rafHandle);
      stopPlayback();
      autosave.stop();
      registerMenuHandlers(null);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onWindowBlur);
      window.removeEventListener('beforeunload', onBeforeUnload);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      view.canvas?.removeEventListener('wheel', onWheel);
      live.renderer?.destroy();
      live.renderer = null;
      disposeDoc();
    };
  });


  // Persist the foreground / background colors once the initial load
  // settled.
  $effect(() => {
    const fg = colors.color;
    const bg = colors.bgColor;
    if (!live.colorPrefLoaded || !idbAvailable) return;
    Promise.all([setPref(COLOR_PREF, fg), setPref(BG_COLOR_PREF, bg)]).catch((err: unknown) => {
      console.error('setPref colors failed', err);
    });
  });

  // Reflect the document identity + dirty state in the tab title.
  $effect(() => {
    document.title = `${isDirty ? '● ' : ''}${file.saveTarget.name} – Pincel`;
  });

  // Mirror the dirty state to the shell (its guards for mode switches,
  // native New / Open and open-file events).
  $effect(() => {
    onDirtyChange(isDirty);
  });

  // Warn before the tab closes with unsaved edits. Autosave (30 s
  // cadence) softens the loss, but the user should still get the
  // browser's "leave site?" prompt.
  function onBeforeUnload(e: BeforeUnloadEvent) {
    if (!isDirty) return;
    e.preventDefault();
    // Chrome still requires returnValue to be set.
    e.returnValue = '';
  }

  // Push the current recents list to the Rust-side `Open Recent`
  // submenu so the native menu stays in sync with the in-memory list.
  // Only entries with a path can be re-opened by the menu (FSA handles
  // need an in-page permission gesture). Failures are logged but don't
  // surface in the UI — the toolbar dropdown remains the authoritative
  // recents UI on the web.
  $effect(() => {
    if (!tauriHost) return;
    const items = file.recents
      .filter((r) => r.path !== null)
      .map((r) => ({ id: r.id, name: r.name }));
    syncRecentMenu(items).catch((err: unknown) => {
      console.error('syncRecentMenu failed', err);
    });
  });
</script>

{#snippet colorsBody()}
  <Panel>
    <ColorPair
      fg={colors.color}
      bg={colors.bgColor}
      onFg={(hex) => (colors.color = hex)}
      onBg={(hex) => (colors.bgColor = hex)}
      onSwap={swapColors}
      onReset={resetColors}
      alpha={colors.alpha}
      onAlpha={(a) => (colors.alpha = a)}
    />
  </Panel>
{/snippet}
{#snippet paletteBody()}
  <PalettePanel
    doc={ed.doc}
    rev={ed.docRev}
    activeColor={colors.color}
    addColor={packColor(colors.color, colors.alpha)}
    onChange={() => {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
    }}
    onError={(msg) => (ed.status = msg)}
    onStatus={(msg) => (ed.status = msg)}
    fileStem={file.saveTarget.name.replace(/\.(aseprite|ase)$/i, '')}
    onPick={(packed) => {
      // Set both the RGB (via the picker) and the alpha slider from
      // the palette entry so a swatch's transparency carries over.
      colors.color = unpackColor(packed);
      colors.alpha = packed & 0xff;
    }}
    onPickBg={(packed) => {
      colors.bgColor = unpackColor(packed);
      colors.bgAlpha = packed & 0xff;
    }}
  />
{/snippet}
{#snippet layersBody()}
  <LayersPanel
    doc={ed.doc}
    rev={ed.docRev}
    frame={ed.currentFrame}
    activeLayerId={ed.activeLayerId}
    onActivate={(layerId) => {
      ed.activeLayerId = layerId;
      // Route pixel tools to the selected layer (M13.3b). A fresh
      // document resets the wasm-side target to None on its own, so
      // the fallback stays correct after New / Open.
      ed.doc?.setActiveLayer(layerId);
    }}
    onToggleVisible={(layerId, visible) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerVisible(layerId, visible);
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerVisibility'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
    onToggleLocked={(layerId, editable) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerEditable(layerId, editable);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerLock'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
    onReorder={(layerId, targetId) => {
      if (!ed.doc) return;
      try {
        ed.doc.reorderLayer(layerId, targetId);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerReorder'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
    onRename={(layerId, name) => {
      if (!ed.doc) return;
      try {
        ed.doc.renameLayer(layerId, name);
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerRename'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
    onAddLayer={addLayer}
    onSetOpacity={(layerId, opacity, commit) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerOpacity(layerId, opacity);
        if (commit) ed.doc.endStroke();
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerOpacity'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
    onSetBlendMode={(layerId, mode) => {
      if (!ed.doc) return;
      try {
        ed.doc.setLayerBlendMode(layerId, mode);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerBlendMode'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
    onDuplicate={duplicateLayer}
    onMergeDown={mergeDownLayer}
    onFlatten={flattenLayers}
    onRemoveLayer={removeLayerById}
  />
{/snippet}
{#snippet tilesetsBody()}
  <TilesetPanel
    doc={ed.doc}
    rev={ed.docRev}
    selectedTile={toolState.stampTile}
    onChange={() => (ed.docRev += 1)}
    onSelectStampTile={(tilesetId, tileId) => {
      toolState.stampTile = { tilesetId, tileId };
      // Auto-switch to the Stamp tool so a single click on a tile
      // is enough to start placing. The user can switch back via
      // the toolbar if they wanted to keep editing pixels.
      toolState.tool = 'tilemap-stamp';
    }}
    onEditTile={(tilesetId, tileId) => {
      dialogs.editingTile = { tilesetId, tileId };
    }}
  />
{/snippet}
{#snippet slicesBody()}
  <SlicesPanel
    doc={ed.doc}
    rev={ed.docRev}
    activeSliceId={ed.activeSliceId}
    onChange={() => (ed.docRev += 1)}
    onActivate={(sliceId) => {
      ed.activeSliceId = sliceId;
      if (sliceId !== null) toolState.tool = 'slice';
      live.needsRecompose = true;
    }}
  />
{/snippet}
{#snippet timelineBody()}
  <TimelinePanel
    doc={ed.doc}
    rev={ed.docRev}
    currentFrame={ed.currentFrame}
    playing={play.playing}
    loopTag={play.loopTag}
    onion={play.onion}
    onSelectFrame={setFrame}
    onAddFrame={addFrame}
    onRemoveFrame={removeFrame}
    onTogglePlay={togglePlay}
    onLoopTagChange={(next) => (play.loopTag = next)}
    onOnionChange={applyOnion}
    onChange={() => {
      live.needsRecompose = true;
      syncMeta();
      ed.docRev += 1;
    }}
    onError={(msg) => (ed.status = msg)}
  />
{/snippet}

<EditorFrame
  mode="pixel"
  {menus}
  title={ed.doc ? file.saveTarget.name : t('px.noDocument')}
  dirty={isDirty}
  panels={[
    { id: 'colors', title: t('panel.colors'), icon: 'colors', body: colorsBody },
    { id: 'palette', title: t('panel.palette'), icon: 'palette', body: paletteBody },
    { id: 'layers', title: t('panel.layers'), icon: 'layers', body: layersBody, grow: true },
    {
      id: 'tilesets',
      title: t('panel.tilesets'),
      icon: 'grid',
      body: tilesetsBody,
      defaultCollapsed: true,
    },
    { id: 'slices', title: t('panel.slices'), icon: 'slice', body: slicesBody, defaultCollapsed: true },
    { id: 'timeline', title: t('panel.timeline'), icon: 'film', body: timelineBody, placement: 'bottom' },
  ]}
>
  {#snippet quick()}
    <IconButton icon="undo" label={t('menu.undo')} hint={hint('undo')} disabled={ed.undoDepth === 0} onclick={undo} />
    <IconButton icon="redo" label={t('menu.redo')} hint={hint('redo')} disabled={ed.redoDepth === 0} onclick={redo} />
  {/snippet}
  {#snippet tools(orientation)}
    <ToolRail groups={railGroups} active={toolState.tool} onSelect={(next) => (toolState.tool = next)} {orientation} />
  {/snippet}
  {#snippet options()}
    <ToolVariants groups={railGroups} active={toolState.tool} onSelect={(next) => (toolState.tool = next)} />
    {#if toolState.tool === 'selection-wand'}
      <label class="opt">
        {t('opt.tolerance')}
        <input
          type="number"
          min="0"
          max="255"
          bind:value={toolState.wandTolerance}
          class="opt-field w-16 text-right tabular-nums"
          aria-label={t('px.wandTolerance')}
        />
      </label>
      <label class="opt">
        <input type="checkbox" bind:checked={toolState.wandContiguous} aria-label={t('opt.contiguous')} />
        {t('opt.contiguous')}
      </label>
      <label class="opt">
        {t('opt.sample')}
        <select bind:value={toolState.wandSample} class="opt-field" aria-label={t('px.wandSample')}>
          <option value="layer">{t('px.layer')}</option>
          <option value="composite">{t('opt.composite')}</option>
        </select>
      </label>
    {:else if isShapeTool(toolState.tool)}
      <label class="opt">
        {t('opt.stroke')}
        <input
          type="number"
          min="1"
          max="64"
          bind:value={toolState.strokeWidth}
          class="opt-field w-14 text-right tabular-nums"
          aria-label={t('px.strokeWidth')}
        />
      </label>
      <label class="opt">
        {t('opt.mode')}
        <select bind:value={toolState.shapeMode} class="opt-field" aria-label={t('px.shapeMode')}>
          <option value="outline">{t('opt.outline')}</option>
          <option value="fill">{t('opt.fillShape')}</option>
          <option value="fill_outline">{t('opt.fillOutline')}</option>
        </select>
      </label>
      {#if toolState.tool === 'rounded-rect'}
        <label class="opt">
          {t('opt.radius')}
          <input
            type="number"
            min="0"
            max="256"
            bind:value={toolState.cornerRadius}
            class="opt-field w-14 text-right tabular-nums"
            aria-label={t('px.cornerRadius')}
          />
        </label>
      {/if}
      {#if toolState.tool === 'polygon-shape'}
        <label class="opt">
          {t('opt.sides')}
          <input
            type="number"
            min="3"
            max="64"
            bind:value={toolState.polygonSides}
            class="opt-field w-14 text-right tabular-nums"
            aria-label={t('px.polygonSides')}
          />
        </label>
      {/if}
      {#if toolState.shapeMode !== 'outline'}
        <span class="opt text-pc-faint">{t('px.fillsBg')}</span>
      {/if}
    {:else}
      <span class="opt text-pc-faint">{toolLabel}</span>
      {#if BG_TOOLS.has(toolState.tool)}
        <span class="opt text-pc-faint">{t('px.rightBg')}</span>
      {/if}
    {/if}
  {/snippet}
  {#snippet viewport()}
        <div
          class="absolute inset-0 flex items-center justify-center overflow-hidden"
          bind:clientWidth={view.stageW}
          bind:clientHeight={view.stageH}
        >
    <!--
            Stacked render surfaces (spec §4.4, M12.5): the base canvas
            shows the composed sprite (driven by a `CanvasRenderer` — WebGPU
            or Canvas2D) and the overlay canvas, sitting exactly on top with
            `pointer-events: none`, carries the transient Canvas2D furniture
            (selection marquee, drag previews, tile grid, slice accents).
            The wrapper owns the display size / pan transform / frame chrome
            so both layers scale and translate as one; the canvases fill it
            via `h-full w-full` and CSS upscales their sprite-sized backing
            stores (`image-rendering: pixelated`).
          -->
          <div
            class="relative shrink-0 border border-pc-border bg-pc-panel shadow-lg"
            style:width="{ed.canvasW * view.zoom}px"
            style:height="{ed.canvasH * view.zoom}px"
            style:transform="translate({view.panX}px, {view.panY}px)"
          >
            <canvas
              bind:this={view.canvas}
              class="canvas-pixelated absolute inset-0 h-full w-full touch-none"
              style:cursor={view.panning
                ? 'grabbing'
                : toolState.moveSelStart
                  ? 'move'
                  : toolState.tool === 'move' && ed.selection && !view.spaceDown
                    ? 'move'
                    : toolState.tool === 'move' || view.spaceDown
                      ? 'grab'
                      : 'crosshair'}
              aria-label="Pincel canvas"
              oncontextmenu={(e) => e.preventDefault()}
              onpointerdown={onPointerDown}
              onpointermove={onPointerMove}
              onpointerup={onPointerUp}
              onpointercancel={onPointerUp}
              onpointerleave={onPointerLeave}
            ></canvas>
            <canvas
              bind:this={view.overlay}
              class="canvas-pixelated pointer-events-none absolute inset-0 h-full w-full"
              width={ed.canvasW}
              height={ed.canvasH}
              aria-hidden="true"
            ></canvas>
          </div>
          {#if ed.doc && dialogs.editingTile}
            <TileEditor
              doc={ed.doc}
              tilesetId={dialogs.editingTile.tilesetId}
              tileId={dialogs.editingTile.tileId}
              color={colors.color}
              rev={ed.docRev}
              onClose={() => (dialogs.editingTile = null)}
              onChange={() => {
                ed.docRev += 1;
                live.needsRecompose = true;
                syncMeta();
              }}
            />
          {/if}
          {#if ed.doc}
            <ZoomControl
              label={`${view.zoom}×`}
              onZoomIn={zoomIn}
              onZoomOut={zoomOut}
              onFit={resetView}
              onActual={zoomActual}
              canZoomIn={view.zoom < MAX_ZOOM}
              canZoomOut={view.zoom > MIN_ZOOM}
            />
          {/if}
        </div>
  {/snippet}
  {#snippet statusbar()}
    {#if ed.doc}
      <span>{ed.canvasW}×{ed.canvasH}</span>
      <span>·</span>
      <span>{t('px.frameOf', { n: ed.currentFrame + 1, total: ed.frameCount })}</span>
      <span class="hidden md:inline">·</span>
      <span class="hidden md:inline">{t('px.undoRedo', { u: ed.undoDepth, r: ed.redoDepth })}</span>
    {/if}
    <span class="min-w-0 flex-1 truncate text-center text-pc-muted">{ed.status}</span>
    {#if probe.on}
      <span class="text-emerald-400">
        {probe.fpsEma.toFixed(0)} fps · compose {probe.composeMs.toFixed(2)}ms (max
        {probe.composeMaxMs.toFixed(2)}ms)
      </span>
    {/if}
    {#if ed.backend !== 'none'}
      <span>{ed.backend}</span>
      <span>·</span>
    {/if}
    <span title={t('px.version')}>{appVersion}</span>
  {/snippet}
</EditorFrame>

{#if autosave.snapshots.length > 0}
  <RecoveryDialog
    snapshots={autosave.snapshots}
    errors={autosave.errors}
    onRecover={(snap) => void autosave.recover(snap)}
    onDiscard={(id) => void autosave.discard(id)}
    onDismiss={() => autosave.dismiss()}
  />
{/if}

{#if dialogs.renaming && ed.doc}
  {@const target = dialogs.renaming}
  <RenameDialog
    name={target.name}
    onCancel={() => (dialogs.renaming = null)}
    onRename={(name) => {
      // Read the target before closing: `target` follows `renaming`.
      const id = target.id;
      dialogs.renaming = null;
      try {
        ed.doc?.renameLayer(id, name);
        live.needsRecompose = true;
        syncMeta();
        ed.docRev += 1;
      } catch (err) {
        ed.status = t('status.failed', { action: t('act.layerRename'), error: err instanceof Error ? err.message : String(err) });
      }
    }}
  />
{/if}

{#if dialogs.sheet && ed.doc}
  <SpriteSheetDialog
    frameCount={ed.frameCount}
    tags={sheetTags}
    frameWidth={ed.canvasW}
    frameHeight={ed.canvasH}
    onSubmit={(o) => void exportSheet(o)}
    onCancel={() => (dialogs.sheet = false)}
  />
{/if}

{#if dialogs.resize}
  {#key dialogs.resize}
    <ResizeDialog
      mode={dialogs.resize}
      width={ed.canvasW}
      height={ed.canvasH}
      maxSize={MAX_DOC_SIZE}
      onSubmit={submitResize}
      onCancel={() => (dialogs.resize = null)}
    />
  {/key}
{/if}

{#if toolState.textAnchor}
  <TextDialog
    x={toolState.textAnchor.x}
    y={toolState.textAnchor.y}
    onPreview={previewTextParams}
    onApply={applyTextParams}
    onCancel={cancelText}
  />
{/if}

{#if dialogs.effect}
  {#key dialogs.effect.id}
    <EffectDialog
      def={dialogs.effect}
      hasSelection={ed.selection !== null}
      onPreview={previewEffectParams}
      onApply={applyEffectParams}
      onCancel={cancelEffect}
    />
  {/key}
{/if}

{#if dialogs.confirm}
  <div
    class="fixed inset-0 z-20 flex items-center justify-center bg-black/50"
    role="alertdialog"
    aria-modal="true"
    aria-label={t('shell.unsavedChanges')}
  >
    <div class="w-80 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl">
      <p>{dialogs.confirm.message}</p>
      <div class="mt-4 flex justify-end gap-2">
        <button class="panel-btn" onclick={() => (dialogs.confirm = null)}>{t('common.cancel')}</button>
        <button
          class="panel-btn border-red-800 text-red-300"
          onclick={confirmDiscard}
        >
          {t('shell.discard')}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .canvas-pixelated {
    image-rendering: pixelated;
  }
</style>
