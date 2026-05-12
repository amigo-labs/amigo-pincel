<script lang="ts">
  import type { Document } from '../core';

  // Paints a single tile from a tileset onto a small Canvas2D element.
  // Tile bytes are fetched through the M8.7b wasm surface
  // (`tilePixels(tilesetId, tileId)`) which returns non-premultiplied
  // RGBA8 in row-major order — the same layout `ImageData` expects, so
  // we wrap the underlying buffer in a `Uint8ClampedArray` view (no
  // copy) before handing it to `putImageData`.
  //
  // `rev` is a parent-managed change counter that bumps whenever the
  // wasm side may have mutated tile pixels (open / undo / redo, plus
  // per-tile edits once M8.7d lands). Reading it inside the effect
  // forces a repaint on those events without needing a deep-equal on
  // the byte buffer.
  let {
    doc,
    tilesetId,
    tileId,
    tileW,
    tileH,
    rev = 0,
  }: {
    doc: Document;
    tilesetId: number;
    tileId: number;
    tileW: number;
    tileH: number;
    rev?: number;
  } = $props();

  let canvas: HTMLCanvasElement | null = $state(null);

  $effect(() => {
    void rev;
    if (!canvas || tileW === 0 || tileH === 0) return;
    let bytes: Uint8Array;
    try {
      bytes = doc.tilePixels(tilesetId, tileId);
    } catch {
      return;
    }
    const expected = tileW * tileH * 4;
    if (bytes.length !== expected) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    // Copy into a fresh `Uint8ClampedArray` rather than aliasing
    // `bytes.buffer`: TypeScript types `Uint8Array.buffer` as
    // `ArrayBufferLike` (could be `SharedArrayBuffer`), and the
    // `ImageData` constructor only accepts `ArrayBuffer`-backed views.
    // The copy is small (one tile's worth of bytes) and avoids the
    // structural cast.
    const clamped = new Uint8ClampedArray(bytes);
    const image = new ImageData(clamped, tileW, tileH);
    ctx.putImageData(image, 0, 0);
  });
</script>

<canvas
  bind:this={canvas}
  width={tileW}
  height={tileH}
  class="tile-thumb"
  aria-label={`tile ${tileId}`}
  title={`tile ${tileId}`}
></canvas>

<style>
  .tile-thumb {
    width: 2rem;
    height: 2rem;
    image-rendering: pixelated;
    background-color: rgb(23 23 23);
    border: 1px solid rgb(38 38 38);
    border-radius: 0.125rem;
  }
</style>
