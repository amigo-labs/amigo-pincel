<script lang="ts">
  import type { Document } from '../core';

  // Inline Tileset Editor sub-mode (CLAUDE.md M8.7d, spec §5.3).
  //
  // Renders the active tile magnified, intercepts clicks / drags, and
  // routes per-pixel paint through `Document::set_tile_pixel`. The
  // wasm side joins the edit to the undo bus, so the global Undo /
  // Redo buttons restore the prior tile state without any editor-
  // specific bookkeeping here.
  //
  // The editor is intentionally orthogonal to the main canvas tools:
  // it has its own pointer handlers and only handles pencil-style
  // single-pixel writes. Routing the existing Line / Rect / Bucket
  // tools through the tile-pixel target is a follow-up (CLAUDE.md
  // §3.3 stopping point — new public API surface would land
  // alongside).
  let {
    doc,
    tilesetId,
    tileId,
    color,
    rev = 0,
    onClose,
    onChange,
  }: {
    doc: Document;
    tilesetId: number;
    tileId: number;
    color: string;
    rev?: number;
    onClose: () => void;
    onChange?: () => void;
  } = $props();

  // Display zoom for the editor canvas. 16× lets a 16x16 tile fit
  // inside a 256-px square — large enough to click individual pixels
  // comfortably without dominating the viewport.
  const ZOOM = 16;

  let canvas: HTMLCanvasElement | null = $state(null);
  let painting = $state(false);
  let lastError = $state<string | null>(null);

  const tileW = $derived(doc.tilesetTileWidth(tilesetId));
  const tileH = $derived(doc.tilesetTileHeight(tilesetId));

  $effect(() => {
    void rev;
    if (!canvas || tileW === 0 || tileH === 0) return;
    let bytes: Uint8Array;
    try {
      bytes = doc.tilePixels(tilesetId, tileId);
    } catch (err) {
      lastError = err instanceof Error ? err.message : String(err);
      return;
    }
    if (bytes.length !== tileW * tileH * 4) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const clamped = new Uint8ClampedArray(bytes);
    const image = new ImageData(clamped, tileW, tileH);
    ctx.putImageData(image, 0, 0);
  });

  function packColor(css: string): number {
    if (css.length !== 7 || css[0] !== '#') return 0xff_00_00_ff;
    const r = parseInt(css.slice(1, 3), 16);
    const g = parseInt(css.slice(3, 5), 16);
    const b = parseInt(css.slice(5, 7), 16);
    return ((r & 0xff) << 24) | ((g & 0xff) << 16) | ((b & 0xff) << 8) | 0xff;
  }

  function tileCoord(e: PointerEvent): { x: number; y: number } | null {
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    const localX = e.clientX - rect.left;
    const localY = e.clientY - rect.top;
    const x = Math.floor((localX / rect.width) * tileW);
    const y = Math.floor((localY / rect.height) * tileH);
    if (x < 0 || y < 0 || x >= tileW || y >= tileH) return null;
    return { x, y };
  }

  function paintAt(e: PointerEvent) {
    const point = tileCoord(e);
    if (!point) return;
    try {
      doc.setTilePixel(tilesetId, tileId, point.x, point.y, packColor(color));
      lastError = null;
      onChange?.();
    } catch (err) {
      lastError = err instanceof Error ? err.message : String(err);
    }
  }

  function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    canvas?.setPointerCapture(e.pointerId);
    painting = true;
    paintAt(e);
  }

  function onPointerMove(e: PointerEvent) {
    if (!painting) return;
    paintAt(e);
  }

  function onPointerUp(e: PointerEvent) {
    if (canvas?.hasPointerCapture(e.pointerId)) {
      canvas.releasePointerCapture(e.pointerId);
    }
    painting = false;
  }
</script>

<div
  class="absolute inset-0 z-10 flex items-center justify-center bg-neutral-950/80 p-4"
  role="dialog"
  aria-modal="true"
  aria-label="Tile editor"
>
  <div class="flex flex-col gap-3 rounded border border-neutral-700 bg-neutral-900 p-4 shadow-xl">
    <header class="flex items-center justify-between gap-4">
      <h2 class="text-sm font-semibold text-neutral-100">
        Editing tileset {tilesetId} · tile {tileId} · {tileW}×{tileH}
      </h2>
      <button type="button" class="editor-btn" onclick={onClose}>Done</button>
    </header>
    <canvas
      bind:this={canvas}
      width={tileW}
      height={tileH}
      class="editor-canvas"
      style="width: {tileW * ZOOM}px; height: {tileH * ZOOM}px;"
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
      aria-label={`tile ${tileId} pixel editor`}
    ></canvas>
    {#if lastError}
      <p class="text-xs text-red-400" role="alert">{lastError}</p>
    {/if}
    <p class="text-xs text-neutral-500">
      Click or drag to paint with the current color. Undo / Redo restores prior
      pixels.
    </p>
  </div>
</div>

<style>
  .editor-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(115 115 115);
    background-color: rgb(55 65 81);
    padding: 0.25rem 0.75rem;
    font-size: 0.75rem;
    color: rgb(229 229 229);
    cursor: pointer;
  }
  .editor-btn:hover {
    background-color: rgb(75 85 99);
  }
  .editor-canvas {
    image-rendering: pixelated;
    background-color: rgb(23 23 23);
    border: 1px solid rgb(64 64 64);
    border-radius: 0.125rem;
    cursor: crosshair;
    touch-action: none;
  }
</style>
