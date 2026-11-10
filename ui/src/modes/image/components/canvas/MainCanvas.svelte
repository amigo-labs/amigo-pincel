<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import {
    editor,
    live,
    tool,
    ui,
    view,
    viewCommands,
    MIN_ZOOM,
    MAX_ZOOM,
  } from '../../stores/editor.svelte';
  import ZoomControl from '../../../../lib/components/layout/ZoomControl.svelte';
  import { altHeld } from '../../../../lib/shell/modifiers.svelte';
  import { isSpaceActivated, isTypingTarget } from '../../../../lib/shell/shortcuts';
  import { readCompositeRegion, redo, undo } from '../../core/controller';
  import { TapTracker } from '../../../../lib/view/taps';
  import { touchGestures } from '../../../../lib/shell/touch-gestures.svelte';
  import { PinchTracker } from '../../../../lib/view/pinch';
  import { drawViewport, sampleFullBuffer } from '../../render/canvas2d';
  import { attachTools, gestureControl } from '../../tools/pointer';
  import { canvasPick, type CanvasPickPhase } from '../../../../lib/effects/canvas-pick.svelte';
  import CanvasOverlay from './CanvasOverlay.svelte';
  import CloneCrosshair from './CloneCrosshair.svelte';
  import PixelGrid from './PixelGrid.svelte';
  import LiveHandles from './LiveHandles.svelte';
  import TextBox from './TextBox.svelte';

  // `canvas` is the viewport-sized render target; `surface` is a backing-
  // store-free element covering the document inside the transformed stage
  // — tools take their pointer coordinates from it.
  let canvas: HTMLCanvasElement;
  let surface: HTMLDivElement;
  let viewport: HTMLDivElement;

  function redraw(): void {
    if (!canvas || editor.handle === null) {
      return;
    }
    // An open effect dialog overrides the canvas with its live preview.
    const preview = editor.previewComposite;
    const sample = preview ? sampleFullBuffer(preview, editor.width) : readCompositeRegion;
    drawViewport(canvas, editor.width, editor.height, view, sample);
  }

  /** Fits the document to the viewport and centres it (spec §6.4). */
  function fitView(): void {
    if (!viewport || editor.width === 0) {
      return;
    }
    const vw = viewport.clientWidth;
    const vh = viewport.clientHeight;
    const pad = 32;
    const zoom = Math.min((vw - pad) / editor.width, (vh - pad) / editor.height, MAX_ZOOM);
    view.zoom = Math.max(zoom, MIN_ZOOM);
    view.panX = (vw - editor.width * view.zoom) / 2;
    view.panY = (vh - editor.height * view.zoom) / 2;
    view.fit = true;
  }

  /** Sets an explicit zoom about the viewport centre. */
  function setZoom(z: number): void {
    if (!viewport) {
      return;
    }
    zoomAround(z, viewport.clientWidth / 2, viewport.clientHeight / 2);
  }

  /** Zooms to `z` keeping the viewport point `(vx, vy)` fixed. */
  function zoomAround(z: number, vx: number, vy: number): void {
    const next = Math.min(Math.max(z, MIN_ZOOM), MAX_ZOOM);
    const worldX = (vx - view.panX) / view.zoom;
    const worldY = (vy - view.panY) / view.zoom;
    view.panX = vx - worldX * next;
    view.panY = vy - worldY * next;
    view.zoom = next;
    view.fit = false;
  }

  function onWheel(e: WheelEvent): void {
    if (editor.handle === null) {
      return;
    }
    e.preventDefault();
    const rect = viewport.getBoundingClientRect();
    const factor = Math.exp(-e.deltaY * 0.0015);
    zoomAround(view.zoom * factor, e.clientX - rect.left, e.clientY - rect.top);
  }

  // Two-finger pinch / pan (spec §17.4), handled in the capture phase so
  // the tool dispatcher on the canvas never sees the gesture. The second
  // finger cancels the first finger's gesture and rolls back the stroke
  // it had started; touch input is then ignored until every finger lifts.
  const pinch = new PinchTracker();
  // Two- / three-finger taps: Undo / Redo (docs/specs/tablet-gestures.md).
  const taps = new TapTracker();
  let pinchZoom0 = 1;
  let gestureUndoDepth = 0;

  // Pick on canvas (effect-catalog-v2 AC-11): while an effect dialog has
  // picking armed, a primary press goes to it instead of the active tool.
  let pickPointer: number | null = null;
  let pickLast: [number, number] = [0.5, 0.5];

  function sendPick(e: PointerEvent, phase: CanvasPickPhase): void {
    const rect = surface.getBoundingClientRect();
    pickLast = [(e.clientX - rect.left) / rect.width, (e.clientY - rect.top) / rect.height];
    canvasPick.handler?.(pickLast[0], pickLast[1], phase);
  }

  /** Ends a pick drag where it last was (a second finger turns it into a pinch). */
  function endPick(): void {
    if (pickPointer === null) return;
    pickPointer = null;
    canvasPick.handler?.(pickLast[0], pickLast[1], 'up');
  }

  function onPointerDownCapture(e: PointerEvent): void {
    taps.down(e);
    if (pinch.down(e)) {
      e.stopPropagation();
      endPick();
      // A second finger ends any Zoom framing in progress, or its box would
      // stay live and swallow the moves the pinch needs.
      zoomBox = null;
      gestureControl.cancel();
      if (editor.undoDepth > gestureUndoDepth) undo();
      pinchZoom0 = view.zoom;
      viewport.setPointerCapture(e.pointerId);
      return;
    }
    if (pinch.suppressing) {
      e.stopPropagation();
      return;
    }
    if (spaceDown && e.button === 0 && editor.handle !== null) {
      e.stopPropagation();
      e.preventDefault();
      startPan(e);
      return;
    }
    if (
      canvasPick.handler !== null &&
      pickPointer === null &&
      e.button === 0 &&
      editor.handle !== null &&
      !onInteractiveTarget(e)
    ) {
      e.stopPropagation();
      e.preventDefault();
      pickPointer = e.pointerId;
      viewport.setPointerCapture(e.pointerId);
      sendPick(e, 'down');
      return;
    }
    // Hand and Zoom move the viewport, not the document, so they are served
    // here — beside the Space-drag they generalise — and never reach the
    // tool dispatcher (spec §9.2 tool rail). The floating ZoomControl is a
    // child of the viewport, so its buttons would otherwise never see their
    // own clicks: with Zoom selected, "Zoom out" would zoom in.
    if (editor.handle !== null && !onInteractiveTarget(e)) {
      if (tool.kind === 'hand' && e.button === 0) {
        e.stopPropagation();
        e.preventDefault();
        startPan(e);
        return;
      }
      // The right button zooms out one notch, click or drag, as Alt+click
      // does (docs/specs/image-editing.md AC-35).
      if (tool.kind === 'zoom' && (e.button === 0 || e.button === 2)) {
        e.stopPropagation();
        e.preventDefault();
        const rect = viewport.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        zoomBox = { x0: x, y0: y, x1: x, y1: y, out: e.button === 2 };
        viewport.setPointerCapture(e.pointerId);
        return;
      }
    }
    // While an effect (or another editor dialog) is open the canvas only
    // zooms and pans: the press never reaches the tool dispatcher; the
    // middle-button pan below still gets it (effect-panel AC-03).
    if (ui.modalOpen && e.button !== 1 && !onInteractiveTarget(e)) {
      e.stopPropagation();
      e.preventDefault();
      return;
    }
    gestureUndoDepth = editor.undoDepth;
  }

  function onPointerMoveCapture(e: PointerEvent): void {
    taps.move(e);
    if (pickPointer === e.pointerId) {
      e.stopPropagation();
      sendPick(e, 'move');
      return;
    }
    if (zoomBox) {
      e.stopPropagation();
      const rect = viewport.getBoundingClientRect();
      zoomBox = { ...zoomBox, x1: e.clientX - rect.left, y1: e.clientY - rect.top };
      return;
    }
    const f = pinch.move(e);
    if (f) {
      e.stopPropagation();
      const rect = viewport.getBoundingClientRect();
      zoomAround(pinchZoom0 * f.scale, f.midX - rect.left, f.midY - rect.top);
      view.panX += f.dx;
      view.panY += f.dy;
      return;
    }
    if (pinch.suppressing) e.stopPropagation();
  }

  function onPointerUpCapture(e: PointerEvent): void {
    // Always retire the pointer, whatever the gesture was: the tracker keys
    // its touches by pointer id and only `up` removes them, so a Zoom release
    // that skipped this left a finger registered forever — the next tap then
    // counted as a second finger and rolled back the previous edit.
    const pinched = pinch.suppressing;
    pinch.up(e);
    // The second finger already rolled back the first finger's stroke; the
    // tap undoes or redoes one more step (AC-05).
    if (e.type === 'pointercancel') taps.cancel(e);
    else {
      const action = taps.up(e);
      if (action !== null && touchGestures.enabled) {
        if (action === 'undo') undo();
        else redo();
      }
    }
    if (pickPointer === e.pointerId) {
      e.stopPropagation();
      // A cancelled pointer reports no position; keep the last one.
      if (e.type === 'pointercancel') endPick();
      else {
        pickPointer = null;
        sendPick(e, 'up');
      }
      if (viewport.hasPointerCapture(e.pointerId)) viewport.releasePointerCapture(e.pointerId);
      return;
    }
    if (zoomBox) {
      e.stopPropagation();
      commitZoom(e);
      return;
    }
    if (pinched) e.stopPropagation();
  }

  /** True when the event started on a control inside the viewport (the
   *  floating zoom buttons), which owns the click rather than the canvas. */
  function onInteractiveTarget(e: PointerEvent): boolean {
    const el = e.target;
    return el instanceof Element && el.closest('button, input, select, a') !== null;
  }

  // Middle-button drag, or any drag with Space held, pans (the tool
  // dispatcher ignores button 1; a Space drag is stopped in the capture
  // phase so tools never see it). Same gesture as Pixel mode.
  let panning = $state(false);
  let spaceDown = $state(false);
  let panStart = { x: 0, y: 0, panX: 0, panY: 0 };

  // The view offset when Space went down: a hold that panned must not
  // also activate the focused button on release.
  let spacePan = { x: 0, y: 0 };

  function onSpace(e: KeyboardEvent): void {
    if (e.code !== 'Space' || !ui.active || isTypingTarget(e.target)) return;
    if (e.type === 'keydown') {
      if (!e.repeat) spacePan = { x: view.panX, y: view.panY };
      if (!isSpaceActivated(e.target)) e.preventDefault();
      spaceDown = true;
    } else {
      if (view.panX !== spacePan.x || view.panY !== spacePan.y) e.preventDefault();
      spaceDown = false;
    }
  }

  // Zoom tool: a drag frames the area to fill the viewport, a click steps
  // one notch — in, or out with Alt (the latching toggle counts too, so a
  // touch device can zoom out without a keyboard).
  const ZOOM_STEP = 1.6;
  const ZOOM_DRAG_MIN = 8;
  let zoomBox = $state<{
    x0: number;
    y0: number;
    x1: number;
    y1: number;
    out: boolean;
  } | null>(null);

  function commitZoom(e: PointerEvent): void {
    const box = zoomBox;
    zoomBox = null;
    if (!box || !viewport) return;
    const w = Math.abs(box.x1 - box.x0);
    const h = Math.abs(box.y1 - box.y0);
    const cx = (box.x0 + box.x1) / 2;
    const cy = (box.y0 + box.y1) / 2;
    if (box.out || w < ZOOM_DRAG_MIN || h < ZOOM_DRAG_MIN) {
      const out = box.out || altHeld(e);
      zoomAround(view.zoom * (out ? 1 / ZOOM_STEP : ZOOM_STEP), box.x0, box.y0);
      return;
    }
    const z = view.zoom * Math.min(viewport.clientWidth / w, viewport.clientHeight / h);
    zoomAround(z, cx, cy);
    // `zoomAround` kept the framed centre where it was; bring it to the
    // middle of the viewport.
    view.panX += viewport.clientWidth / 2 - cx;
    view.panY += viewport.clientHeight / 2 - cy;
  }

  function startPan(e: PointerEvent): void {
    panning = true;
    panStart = { x: e.clientX, y: e.clientY, panX: view.panX, panY: view.panY };
    viewport.setPointerCapture(e.pointerId);
  }

  function onPointerDown(e: PointerEvent): void {
    if (e.button !== 1 || editor.handle === null) {
      return;
    }
    e.preventDefault();
    startPan(e);
  }

  function onPointerMove(e: PointerEvent): void {
    if (!panning) {
      return;
    }
    view.panX = panStart.panX + (e.clientX - panStart.x);
    view.panY = panStart.panY + (e.clientY - panStart.y);
    view.fit = false;
  }

  function onPointerUp(e: PointerEvent): void {
    if (panning) {
      panning = false;
      viewport.releasePointerCapture(e.pointerId);
    }
  }

  const toolCursors: Record<string, string> = {
    pencil: 'crosshair',
    eraser: 'crosshair',
    fill: 'cell',
    gradient: 'crosshair',
    recolor: 'crosshair',
    smudge: 'crosshair',
    clone: 'crosshair',
    eyedropper: 'copy',
    move: 'move',
    move_selection: 'move',
    move_mask: 'move',
    rect_select: 'crosshair',
    ellipse_select: 'crosshair',
    lasso: 'crosshair',
    polygon_lasso: 'crosshair',
    magic_wand: 'cell',
    hand: 'grab',
    zoom: 'zoom-in',
    line: 'crosshair',
    rectangle: 'crosshair',
    rounded_rect: 'crosshair',
    ellipse: 'crosshair',
    polygon: 'crosshair',
    triangle: 'crosshair',
    diamond: 'crosshair',
    star: 'crosshair',
    block_arrow: 'crosshair',
    heart: 'crosshair',
    callout: 'crosshair',
    text: 'text',
  };
  const cursor = $derived(
    panning
      ? 'grabbing'
      : spaceDown
        ? 'grab'
        : canvasPick.handler !== null
          ? 'crosshair'
          : (live.cursor ?? toolCursors[tool.kind] ?? 'default'),
  );
  // A right-button drag zooms out, so it frames no box.
  const zoomRect = $derived(
    zoomBox === null || zoomBox.out
      ? null
      : {
          left: Math.min(zoomBox.x0, zoomBox.x1),
          top: Math.min(zoomBox.y0, zoomBox.y1),
          width: Math.abs(zoomBox.x1 - zoomBox.x0),
          height: Math.abs(zoomBox.y1 - zoomBox.y0),
        },
  );
  const stageTransform = $derived(`translate(${view.panX}px, ${view.panY}px) scale(${view.zoom})`);

  onMount(() => {
    viewCommands.zoomIn = () => setZoom(view.zoom * 1.25);
    viewCommands.zoomOut = () => setZoom(view.zoom / 1.25);
    viewCommands.fit = fitView;
    viewCommands.actual = () => setZoom(1);
    const detach = attachTools(surface, viewport, redraw);
    // Re-fit when the viewport resizes while in fit mode.
    const ro = new ResizeObserver(() => {
      if (view.fit) {
        fitView();
      }
      redraw();
    });
    ro.observe(viewport);
    return () => {
      detach();
      ro.disconnect();
    };
  });

  // Recompose whenever the document mutates or the view moves (only the
  // visible rect is composited); fit a freshly opened document. A tab
  // switched back to keeps its own zoom and scroll (document-tabs AC-05):
  // only a view still in fit mode is re-fitted.
  let lastHandle: number | null = null;
  // A document is open, fitted and drawn — the canvas takes input.
  // Published as `data-ready` on the surface.
  let ready = $state(false);
  $effect(() => {
    void editor.revision;
    void editor.previewComposite;
    void view.zoom;
    void view.panX;
    void view.panY;
    if (editor.handle !== lastHandle) {
      lastHandle = editor.handle;
      if (editor.handle !== null && view.fit) {
        fitView();
      }
    }
    redraw();
    ready = editor.handle !== null;
  });
</script>

<svelte:window onkeydown={onSpace} onkeyup={onSpace} onblur={() => (spaceDown = false)} />

<!-- The viewport is the tool stage (selection gestures start beside the
     canvas, spec §9.4); `select-none` keeps a drag from starting a text
     selection that the next press would drag away (pointercancel). -->
<div
  bind:this={viewport}
  class="relative h-full w-full touch-none overflow-hidden bg-pc-bg select-none"
  style={live.cursor ? `cursor: ${live.cursor}` : undefined}
  onwheel={onWheel}
  onpointerdowncapture={onPointerDownCapture}
  onpointermovecapture={onPointerMoveCapture}
  onpointerupcapture={onPointerUpCapture}
  onpointercancelcapture={onPointerUpCapture}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
  role="presentation"
>
  <canvas
    bind:this={canvas}
    class="pointer-events-none absolute inset-0 h-full w-full"
    style="image-rendering: pixelated;"
  ></canvas>

  {#if editor.handle === null}
    <p class="absolute inset-0 flex items-center justify-center text-sm text-pc-faint">
      {t('img.empty')}
    </p>
  {/if}

  <!-- The stage carries the view transform: the pointer surface lives in
       document pixels inside it (pointer coords come from the surface's
       getBoundingClientRect). Pixels are drawn by the viewport-sized canvas
       above, the marching ants, handles and live text box by the overlays
       below. -->
  <div
    class="absolute left-0 top-0 origin-top-left"
    style="transform: {stageTransform}; {editor.handle === null ? 'display:none' : ''}"
  >
    <div
      bind:this={surface}
      class="block shadow-2xl shadow-black/60"
      role="img"
      aria-label="Image canvas"
      data-ready={ready}
      style="width: {editor.width}px; height: {editor.height}px; cursor: {cursor};"
    ></div>
  </div>

  <PixelGrid />
  <CanvasOverlay />
  <CloneCrosshair />
  <LiveHandles />
  <TextBox />

  {#if zoomRect}
    <div
      class="pointer-events-none absolute border border-pc-accent bg-pc-accent/10"
      style="left: {zoomRect.left}px; top: {zoomRect.top}px; width: {zoomRect.width}px; height: {zoomRect.height}px;"
    ></div>
  {/if}

  <!-- Zoom controls (spec §6.4), shared with Pixel mode. -->
  {#if editor.handle !== null}
    <ZoomControl
      label={`${Math.round(view.zoom * 100)}%`}
      onZoomIn={() => setZoom(view.zoom * 1.25)}
      onZoomOut={() => setZoom(view.zoom / 1.25)}
      onFit={fitView}
      onActual={() => setZoom(1)}
      canZoomIn={view.zoom < MAX_ZOOM}
      canZoomOut={view.zoom > MIN_ZOOM}
    />
  {/if}
</div>
