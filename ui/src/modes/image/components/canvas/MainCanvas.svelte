<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  import { onMount } from 'svelte';
  import {
    editor,
    tool,
    view,
    viewCommands,
    MIN_ZOOM,
    MAX_ZOOM,
  } from '../../stores/editor.svelte';
  import ZoomControl from '../../../../lib/components/layout/ZoomControl.svelte';
  import { isTypingTarget } from '../../../../lib/shell/shortcuts';
  import { readCompositeRegion, renderText, undo } from '../../core/controller';
  import { PinchTracker } from '../../../../lib/view/pinch';
  import { drawViewport, sampleFullBuffer } from '../../render/canvas2d';
  import { attachTools, gestureControl } from '../../tools/pointer';
  import CanvasOverlay from './CanvasOverlay.svelte';

  // `canvas` is the viewport-sized render target; `surface` is a backing-
  // store-free element covering the document inside the transformed stage
  // — tools take their pointer coordinates from it.
  let canvas: HTMLCanvasElement;
  let surface: HTMLDivElement;
  let viewport: HTMLDivElement;

  // In-progress text entry (Text tool). `cx`/`cy` are canvas-space; the floating
  // textarea lives inside the transformed stage, so it is positioned in those
  // same document pixels and the stage transform scales it (spec §9.2).
  let textEntry = $state<{ cx: number; cy: number; text: string } | null>(null);

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
  let pinchZoom0 = 1;
  let gestureUndoDepth = 0;

  function onPointerDownCapture(e: PointerEvent): void {
    if (pinch.down(e)) {
      e.stopPropagation();
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
    gestureUndoDepth = editor.undoDepth;
  }

  function onPointerMoveCapture(e: PointerEvent): void {
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
    const pinched = pinch.suppressing;
    pinch.up(e);
    if (pinched) e.stopPropagation();
  }

  // Middle-button drag, or any drag with Space held, pans (the tool
  // dispatcher ignores button 1; a Space drag is stopped in the capture
  // phase so tools never see it). Same gesture as Pixel mode.
  let panning = $state(false);
  let spaceDown = $state(false);
  let panStart = { x: 0, y: 0, panX: 0, panY: 0 };

  function onSpace(e: KeyboardEvent): void {
    if (e.code !== 'Space' || isTypingTarget(e.target)) return;
    if (e.type === 'keydown') {
      e.preventDefault();
      spaceDown = true;
    } else {
      spaceDown = false;
    }
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

  /** Opens a text-entry box at a click, committing any prior entry first. */
  function placeText(cx: number, cy: number): void {
    commitText();
    textEntry = { cx, cy, text: '' };
  }

  /** Rasterizes the current entry to pixels (no-op if empty), then clears it. */
  function commitText(): void {
    const entry = textEntry;
    if (!entry) {
      return;
    }
    textEntry = null; // clear first so a trailing blur does not double-commit
    if (entry.text.trim().length > 0) {
      void renderText(entry.cx, entry.cy, entry.text)
        .then(redraw)
        .catch(() => {});
    }
  }

  function onEntryKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape' || (e.key === 'Enter' && (e.ctrlKey || e.metaKey))) {
      e.preventDefault();
      e.stopPropagation();
      commitText();
    }
  }

  function autofocus(node: HTMLTextAreaElement): void {
    node.focus();
  }

  // The core anchors center/right-aligned text about `x` (tools/text.rs), so
  // the preview box shifts by the same fraction of its own width.
  const entryShift = $derived(
    tool.textAlign === 'center' ? '-50%' : tool.textAlign === 'right' ? '-100%' : '0',
  );

  const toolCursors: Record<string, string> = {
    pencil: 'crosshair',
    eraser: 'crosshair',
    fill: 'cell',
    eyedropper: 'copy',
    move: 'move',
    rect_select: 'crosshair',
    ellipse_select: 'crosshair',
    lasso: 'crosshair',
    polygon_lasso: 'crosshair',
    magic_wand: 'cell',
    shapes: 'crosshair',
    text: 'text',
  };
  const cursor = $derived(
    panning ? 'grabbing' : spaceDown ? 'grab' : (toolCursors[tool.kind] ?? 'default'),
  );
  const stageTransform = $derived(`translate(${view.panX}px, ${view.panY}px) scale(${view.zoom})`);

  onMount(() => {
    viewCommands.zoomIn = () => setZoom(view.zoom * 1.25);
    viewCommands.zoomOut = () => setZoom(view.zoom / 1.25);
    viewCommands.fit = fitView;
    viewCommands.actual = () => setZoom(1);
    const detach = attachTools(surface, redraw, placeText);
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
  // visible rect is composited); fit a freshly opened document.
  let lastHandle: number | null = null;
  $effect(() => {
    void editor.revision;
    void editor.previewComposite;
    void view.zoom;
    void view.panX;
    void view.panY;
    if (editor.handle !== lastHandle) {
      lastHandle = editor.handle;
      if (editor.handle !== null) {
        fitView();
      }
    }
    redraw();
  });
</script>

<svelte:window onkeydown={onSpace} onkeyup={onSpace} onblur={() => (spaceDown = false)} />

<div
  bind:this={viewport}
  class="relative h-full w-full touch-none overflow-hidden bg-pc-bg"
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

  <!-- The stage carries the view transform: the pointer surface and the text
       entry live in document pixels inside it (pointer coords come from the
       surface's getBoundingClientRect). Pixels are drawn by the viewport-sized
       canvas above, the marching ants by the overlay below. -->
  <div
    class="absolute left-0 top-0 origin-top-left"
    style="transform: {stageTransform}; {editor.handle === null ? 'display:none' : ''}"
  >
    <div
      bind:this={surface}
      class="block shadow-2xl shadow-black/60"
      role="img"
      aria-label="Image canvas"
      style="width: {editor.width}px; height: {editor.height}px; cursor: {cursor};"
    ></div>
    {#if textEntry}
      <textarea
        use:autofocus
        bind:value={textEntry.text}
        onkeydown={onEntryKeydown}
        onblur={commitText}
        spellcheck="false"
        class="absolute z-10 resize-none overflow-hidden whitespace-pre border border-dashed border-pc-accent bg-transparent p-0 leading-none outline-none"
        style="left: {textEntry.cx}px; top: {textEntry.cy}px; transform: translateX({entryShift}); text-align: {tool.textAlign}; font-size: {tool.fontSize}px; color: {tool.foreground}; font-family: 'Liberation Sans', Arial, sans-serif; font-weight: {tool.textBold
          ? 'bold'
          : 'normal'}; font-style: {tool.textItalic ? 'italic' : 'normal'}; min-width: 4ch;"
      ></textarea>
    {/if}
  </div>

  <CanvasOverlay />

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
