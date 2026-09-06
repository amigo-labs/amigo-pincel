<script lang="ts">
  import { onMount } from 'svelte';
  import { editor, tool, view, MIN_ZOOM, MAX_ZOOM } from '../../stores/editor.svelte';
  import { readComposite, renderText } from '../../core/controller';
  import { drawComposite } from '../../render/canvas2d';
  import { attachTools } from '../../tools/pointer';
  import CanvasOverlay from './CanvasOverlay.svelte';

  let canvas: HTMLCanvasElement;
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
    const rgba = editor.previewComposite ?? readComposite();
    if (rgba) {
      drawComposite(canvas, editor.width, editor.height, rgba);
    }
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

  // Middle-button drag pans (the tool dispatcher ignores button 1).
  let panning = false;
  let panStart = { x: 0, y: 0, panX: 0, panY: 0 };

  function onPointerDown(e: PointerEvent): void {
    if (e.button !== 1 || editor.handle === null) {
      return;
    }
    e.preventDefault();
    panning = true;
    panStart = { x: e.clientX, y: e.clientY, panX: view.panX, panY: view.panY };
    viewport.setPointerCapture(e.pointerId);
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
  const cursor = $derived(toolCursors[tool.kind] ?? 'default');
  const stageTransform = $derived(`translate(${view.panX}px, ${view.panY}px) scale(${view.zoom})`);

  onMount(() => {
    const detach = attachTools(canvas, redraw, placeText);
    // Re-fit when the viewport resizes while in fit mode.
    const ro = new ResizeObserver(() => {
      if (view.fit) {
        fitView();
      }
    });
    ro.observe(viewport);
    return () => {
      detach();
      ro.disconnect();
    };
  });

  // Recompose whenever the document mutates; fit a freshly opened document.
  let lastHandle: number | null = null;
  $effect(() => {
    void editor.revision;
    if (editor.handle !== lastHandle) {
      lastHandle = editor.handle;
      if (editor.handle !== null) {
        fitView();
      }
    }
    redraw();
  });
</script>

<div
  bind:this={viewport}
  class="relative h-full w-full overflow-hidden bg-[var(--fl-app-bg)]"
  onwheel={onWheel}
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
  role="presentation"
>
  {#if editor.handle === null}
    <p class="absolute inset-0 flex items-center justify-center text-sm text-neutral-500">
      Open an image or create a new document to start.
    </p>
  {/if}

  <!-- The stage holds the canvas + overlay; the view transform zooms/pans both
       together, and pointer coords stay correct via getBoundingClientRect. -->
  <div
    class="absolute left-0 top-0 origin-top-left"
    style="transform: {stageTransform}; {editor.handle === null ? 'display:none' : ''}"
  >
    <canvas
      bind:this={canvas}
      class="block shadow-2xl shadow-black/60"
      style="width: {editor.width}px; height: {editor.height}px; image-rendering: pixelated; cursor: {cursor};"
    ></canvas>
    <CanvasOverlay />
    {#if textEntry}
      <textarea
        use:autofocus
        bind:value={textEntry.text}
        onkeydown={onEntryKeydown}
        onblur={commitText}
        spellcheck="false"
        class="absolute z-10 resize-none overflow-hidden whitespace-pre border border-dashed border-[var(--fl-accent)] bg-transparent p-0 leading-none outline-none"
        style="left: {textEntry.cx}px; top: {textEntry.cy}px; transform: translateX({entryShift}); text-align: {tool.textAlign}; font-size: {tool.fontSize}px; color: {tool.foreground}; font-family: 'Liberation Sans', Arial, sans-serif; font-weight: {tool.textBold
          ? 'bold'
          : 'normal'}; font-style: {tool.textItalic ? 'italic' : 'normal'}; min-width: 4ch;"></textarea>
    {/if}
  </div>

  <!-- Zoom controls (spec §6.4). -->
  {#if editor.handle !== null}
    <div
      class="absolute bottom-3 right-3 flex items-center gap-1 rounded border border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)]/90 px-1 py-0.5 text-xs shadow-lg"
    >
      <button
        class="rounded px-2 py-0.5 hover:bg-neutral-700"
        onclick={() => setZoom(view.zoom / 1.25)}
        aria-label="Zoom out">−</button
      >
      <button
        class="w-14 rounded px-1 py-0.5 text-center tabular-nums hover:bg-neutral-700"
        onclick={fitView}
      >
        {Math.round(view.zoom * 100)}%
      </button>
      <button
        class="rounded px-2 py-0.5 hover:bg-neutral-700"
        onclick={() => setZoom(view.zoom * 1.25)}
        aria-label="Zoom in">+</button
      >
      <button class="rounded px-2 py-0.5 hover:bg-neutral-700" onclick={() => setZoom(1)}>1:1</button>
    </div>
  {/if}
</div>
