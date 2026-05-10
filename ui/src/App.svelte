<script lang="ts">
  import { onMount } from 'svelte';
  import { Document, loadCore } from './lib/core';
  import { blitFrame } from './lib/render/canvas2d';

  // The wasm `Document` is the source of truth for sprite state
  // (CLAUDE.md §9 — "canvas-as-source-of-truth" anti-pattern). The UI
  // holds an opaque handle, paints with `applyTool`, and re-renders by
  // calling `compose()` and blitting the resulting `ComposeFrame`.
  type Tool = 'pencil' | 'eraser' | 'eyedropper';

  let canvas = $state<HTMLCanvasElement | null>(null);
  let doc = $state<Document | null>(null);
  let color = $state('#f87171');
  let tool = $state<Tool>('pencil');
  let undoDepth = $state(0);
  let redoDepth = $state(0);
  let canvasW = $state(64);
  let canvasH = $state(64);
  let status = $state('initializing…');
  let painting = false;
  let dirty = false;
  let rafHandle: number | null = null;
  let fileInput: HTMLInputElement | null = null;

  function syncMeta() {
    if (!doc) return;
    undoDepth = doc.undoDepth;
    redoDepth = doc.redoDepth;
    canvasW = doc.width;
    canvasH = doc.height;
  }

  function recompose() {
    if (!doc || !canvas) return;
    const frame = doc.compose(0, 1);
    try {
      blitFrame(canvas, frame);
    } finally {
      frame.free();
    }
  }

  // `<input type="color">` reports `#RRGGBB`. The wasm `applyTool`
  // expects a packed `0xRRGGBBAA`; alpha is fixed at fully opaque
  // until the UI grows an alpha control.
  function packColor(hex: string): number {
    const rgb = Number.parseInt(hex.slice(1), 16);
    return ((rgb << 8) | 0xff) >>> 0;
  }

  // Convert a packed `0xRRGGBBAA` back to the `#RRGGBB` form the color
  // input expects. Alpha is intentionally dropped — the input has no
  // alpha control yet, and `pickColor` callers that need it can read
  // the raw u32 themselves.
  function unpackColor(rgba: number): string {
    const rgb = (rgba >>> 8) & 0xffffff;
    return '#' + rgb.toString(16).padStart(6, '0');
  }

  function spriteCoord(e: PointerEvent): { x: number; y: number } | null {
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    const x = Math.floor(((e.clientX - rect.left) * canvas.width) / rect.width);
    const y = Math.floor(((e.clientY - rect.top) * canvas.height) / rect.height);
    return { x, y };
  }

  function paintAt(e: PointerEvent) {
    if (!doc) return;
    const point = spriteCoord(e);
    if (!point) return;
    if (tool === 'eyedropper') {
      // The eyedropper is read-only: sample the composed pixel and
      // bind it to the foreground color picker. Drags keep sampling
      // so the user can scrub for the exact pixel they want.
      try {
        const picked = doc.pickColor(0, point.x, point.y);
        color = unpackColor(picked);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error('pickColor failed', err);
        status = `pick failed: ${msg}`;
      }
      return;
    }
    try {
      // The wasm eraser ignores the `color` arg, but we still pass
      // the packed foreground so the JS surface stays uniform.
      doc.applyTool(tool, point.x, point.y, packColor(color));
    } catch (err) {
      // Drags that leave the canvas raise PixelOutOfBounds; that is
      // expected and silenced. Anything else (missing layer, unknown
      // tool, …) is a real failure: surface it in the status bar
      // and log so it doesn't disappear.
      const msg = err instanceof Error ? err.message : String(err);
      if (msg.includes('pixel out of bounds')) return;
      console.error('applyTool failed', err);
      status = `paint failed: ${msg}`;
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

  // wasm-bindgen classes own Rust-side allocations; freeing the prior
  // `Document` before replacing it avoids leaking memory across
  // repeated New / Open operations. Safe to call when `doc` is null.
  function disposeDoc() {
    if (doc) {
      doc.free();
      doc = null;
    }
  }

  function newDoc() {
    disposeDoc();
    doc = new Document(64, 64);
    dirty = true;
    syncMeta();
    status = 'new 64×64 document';
  }

  async function openFile(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const next = Document.openAseprite(bytes);
      disposeDoc();
      doc = next;
      dirty = true;
      syncMeta();
      status = `opened ${file.name} · ${doc.width}×${doc.height}`;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `open failed: ${msg}`;
    } finally {
      input.value = '';
    }
  }

  function save() {
    if (!doc) return;
    try {
      const bytes = doc.saveAseprite();
      const blob = new Blob([new Uint8Array(bytes)], {
        type: 'application/octet-stream',
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'pincel.aseprite';
      a.click();
      // Defer the revoke: some browsers cancel the download if the
      // blob URL is revoked synchronously after `.click()`.
      setTimeout(() => URL.revokeObjectURL(url), 0);
      status = `saved ${bytes.length} bytes`;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `save failed: ${msg}`;
    }
  }

  function undo() {
    if (doc?.undo()) {
      dirty = true;
      syncMeta();
    }
  }

  function redo() {
    if (!doc) return;
    try {
      if (doc.redo()) {
        dirty = true;
        syncMeta();
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `redo failed: ${msg}`;
    }
  }

  function tick() {
    if (doc) {
      const events = doc.drainEvents();
      if (events.length > 0) dirty = true;
      for (const ev of events) ev.free();
      if (dirty) {
        dirty = false;
        recompose();
        syncMeta();
      }
    }
    rafHandle = requestAnimationFrame(tick);
  }

  onMount(() => {
    let cancelled = false;
    loadCore()
      .then(() => {
        if (cancelled) return;
        doc = new Document(64, 64);
        syncMeta();
        dirty = true;
        status = 'ready';
        rafHandle = requestAnimationFrame(tick);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const msg = err instanceof Error ? err.message : String(err);
        console.error('loadCore failed', err);
        status = `wasm init failed: ${msg}`;
      });
    return () => {
      cancelled = true;
      if (rafHandle !== null) cancelAnimationFrame(rafHandle);
      disposeDoc();
    };
  });
</script>

<main class="flex h-full flex-col bg-neutral-950 text-neutral-100">
  <header class="flex flex-wrap items-center gap-2 border-b border-neutral-800 px-4 py-2 text-sm">
    <span class="mr-2 font-semibold tracking-wide">Pincel</span>
    <button class="toolbar-btn" onclick={newDoc}>New</button>
    <button class="toolbar-btn" onclick={() => fileInput?.click()}>Open…</button>
    <button class="toolbar-btn" onclick={save} disabled={!doc}>Save</button>
    <input
      bind:this={fileInput}
      type="file"
      accept=".aseprite,.ase"
      class="hidden"
      onchange={openFile}
    />
    <span class="ml-2 flex items-center gap-1" role="group" aria-label="Active tool">
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'pencil'}
        aria-pressed={tool === 'pencil'}
        onclick={() => (tool = 'pencil')}
      >
        Pencil
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'eraser'}
        aria-pressed={tool === 'eraser'}
        onclick={() => (tool = 'eraser')}
      >
        Eraser
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'eyedropper'}
        aria-pressed={tool === 'eyedropper'}
        onclick={() => (tool = 'eyedropper')}
      >
        Eyedropper
      </button>
    </span>
    <label class="ml-2 flex items-center gap-1 text-xs text-neutral-400">
      <span>Color</span>
      <input
        type="color"
        bind:value={color}
        class="h-6 w-8 cursor-pointer rounded border border-neutral-700 bg-transparent"
      />
    </label>
    <button class="toolbar-btn ml-2" onclick={undo} disabled={undoDepth === 0}>Undo</button>
    <button class="toolbar-btn" onclick={redo} disabled={redoDepth === 0}>Redo</button>
  </header>

  <section class="flex flex-1 items-center justify-center overflow-hidden p-6">
    <canvas
      bind:this={canvas}
      width="64"
      height="64"
      class="canvas-pixelated h-[512px] w-[512px] touch-none border border-neutral-700 bg-neutral-900 shadow-lg"
      aria-label="Pincel canvas"
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
    ></canvas>
  </section>

  <footer class="flex items-center gap-3 border-t border-neutral-800 px-4 py-2 text-xs text-neutral-500">
    <span>{status}</span>
    {#if doc}
      <span>·</span>
      <span>{canvasW}×{canvasH}</span>
      <span>·</span>
      <span>undo {undoDepth} / redo {redoDepth}</span>
    {/if}
  </footer>
</main>

<style>
  .canvas-pixelated {
    image-rendering: pixelated;
  }

  .toolbar-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.125rem 0.5rem;
    font-size: 0.75rem;
  }
  .toolbar-btn:hover:not(:disabled) {
    background-color: rgb(38 38 38);
  }
  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .toolbar-btn-active {
    background-color: rgb(64 64 64);
    border-color: rgb(115 115 115);
  }
</style>
