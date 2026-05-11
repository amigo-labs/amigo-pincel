<script lang="ts">
  import { onMount } from 'svelte';
  import { Document, loadCore } from './lib/core';
  import {
    blitFrame,
    paintEllipsePreview,
    paintLinePreview,
    paintRectanglePreview,
  } from './lib/render/canvas2d';

  // The wasm `Document` is the source of truth for sprite state
  // (CLAUDE.md §9 — "canvas-as-source-of-truth" anti-pattern). The UI
  // holds an opaque handle, paints with `applyTool`, and re-renders by
  // calling `compose()` and blitting the resulting `ComposeFrame`.
  type Tool =
    | 'pencil'
    | 'eraser'
    | 'eyedropper'
    | 'bucket'
    | 'line'
    | 'rectangle'
    | 'rectangle-fill'
    | 'ellipse'
    | 'ellipse-fill';

  // Tools that use the press / drag / release pipeline (a start point
  // captured on `pointerdown`, a live endpoint tracked on
  // `pointermove`, committed on `pointerup`).
  function isDragShapeTool(t: Tool): boolean {
    return (
      t === 'line' ||
      t === 'rectangle' ||
      t === 'rectangle-fill' ||
      t === 'ellipse' ||
      t === 'ellipse-fill'
    );
  }

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
  // Press / current point of an in-flight drag-shape tool (Line,
  // Rectangle, Rectangle Fill). `null` outside a drag. `dragPreview`
  // is the live endpoint; both are sprite-space. `dragTool` snapshots
  // the active tool at press-time so a mid-drag toolbar change does
  // not flip the in-flight shape kind.
  let dragStart: { x: number; y: number } | null = null;
  let dragPreview: { x: number; y: number } | null = null;
  let dragTool: Tool | null = null;
  // Whether Shift was held during the most recent pointer event in an
  // in-flight Rectangle drag. The Rust command takes raw corners — the
  // square constraint is purely a UI affordance applied to the live
  // endpoint before committing.
  let dragShift = false;

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
      if (dragStart && dragPreview && dragTool) {
        // Overlay the in-flight drag-shape preview after the blit. The
        // next recompose clears it; on release we commit through the
        // appropriate wasm method and the composed cel surfaces the
        // same pixels naturally.
        const end = constrainedEndpoint();
        if (dragTool === 'line') {
          paintLinePreview(canvas, dragStart.x, dragStart.y, end.x, end.y, color);
        } else if (dragTool === 'rectangle') {
          paintRectanglePreview(
            canvas,
            dragStart.x,
            dragStart.y,
            end.x,
            end.y,
            color,
            false,
          );
        } else if (dragTool === 'rectangle-fill') {
          paintRectanglePreview(
            canvas,
            dragStart.x,
            dragStart.y,
            end.x,
            end.y,
            color,
            true,
          );
        } else if (dragTool === 'ellipse') {
          paintEllipsePreview(
            canvas,
            dragStart.x,
            dragStart.y,
            end.x,
            end.y,
            color,
            false,
          );
        } else if (dragTool === 'ellipse-fill') {
          paintEllipsePreview(
            canvas,
            dragStart.x,
            dragStart.y,
            end.x,
            end.y,
            color,
            true,
          );
        }
      }
    } finally {
      frame.free();
    }
  }

  // Sprite-space endpoint for the in-flight drag, after applying any
  // active modifier constraint (Shift = square for Rectangle, circle
  // for Ellipse — both forms reduce to a square bbox).
  function constrainedEndpoint(): { x: number; y: number } {
    if (!dragStart || !dragPreview) {
      return { x: 0, y: 0 };
    }
    if (
      dragShift &&
      (dragTool === 'rectangle' ||
        dragTool === 'rectangle-fill' ||
        dragTool === 'ellipse' ||
        dragTool === 'ellipse-fill')
    ) {
      const dx = dragPreview.x - dragStart.x;
      const dy = dragPreview.y - dragStart.y;
      const side = Math.max(Math.abs(dx), Math.abs(dy));
      const sx = dx < 0 ? -1 : 1;
      const sy = dy < 0 ? -1 : 1;
      return { x: dragStart.x + side * sx, y: dragStart.y + side * sy };
    }
    return dragPreview;
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
    // Drag-shape tools (Line, Rectangle, …) have their own press /
    // drag / release pipeline below and commit on `pointerup`. The
    // wasm `applyTool` surface only accepts per-pixel tools, so a
    // mid-drag tool switch must not route a stroke through here.
    if (isDragShapeTool(tool)) return;
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
    if (isDragShapeTool(tool)) {
      const point = spriteCoord(e);
      if (!point) return;
      dragStart = point;
      dragPreview = point;
      dragTool = tool;
      dragShift = e.shiftKey;
      dirty = true;
      return;
    }
    if (tool === 'bucket') {
      // Bucket commits once per click; entering painting mode would have
      // `pointermove` re-fire `applyBucket` and push a no-op fill onto
      // the bus on every pixel of the drag.
      commitBucket(e);
      return;
    }
    painting = true;
    paintAt(e);
  }

  function commitBucket(e: PointerEvent) {
    if (!doc) return;
    const point = spriteCoord(e);
    if (!point) return;
    try {
      doc.applyBucket(point.x, point.y, packColor(color));
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('applyBucket failed', err);
      status = `bucket failed: ${msg}`;
    }
  }

  function onPointerMove(e: PointerEvent) {
    if (dragStart) {
      const point = spriteCoord(e);
      if (!point) return;
      dragPreview = point;
      dragShift = e.shiftKey;
      dirty = true;
      return;
    }
    if (!painting) return;
    paintAt(e);
  }

  function onPointerUp(e: PointerEvent) {
    if (canvas?.hasPointerCapture(e.pointerId)) {
      canvas.releasePointerCapture(e.pointerId);
    }
    if (dragStart && dragPreview && dragTool && doc) {
      dragShift = e.shiftKey;
      const end = constrainedEndpoint();
      const packed = packColor(color);
      try {
        if (dragTool === 'line') {
          doc.applyLine(dragStart.x, dragStart.y, end.x, end.y, packed);
        } else if (dragTool === 'rectangle') {
          doc.applyRectangle(dragStart.x, dragStart.y, end.x, end.y, packed, false);
        } else if (dragTool === 'rectangle-fill') {
          doc.applyRectangle(dragStart.x, dragStart.y, end.x, end.y, packed, true);
        } else if (dragTool === 'ellipse') {
          doc.applyEllipse(dragStart.x, dragStart.y, end.x, end.y, packed, false);
        } else if (dragTool === 'ellipse-fill') {
          doc.applyEllipse(dragStart.x, dragStart.y, end.x, end.y, packed, true);
        }
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        console.error(`${dragTool} failed`, err);
        status = `${dragTool} failed: ${msg}`;
      }
      dragStart = null;
      dragPreview = null;
      dragTool = null;
      dragShift = false;
      dirty = true;
      syncMeta();
      return;
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
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'bucket'}
        aria-pressed={tool === 'bucket'}
        onclick={() => (tool = 'bucket')}
      >
        Bucket
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'line'}
        aria-pressed={tool === 'line'}
        onclick={() => (tool = 'line')}
      >
        Line
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'rectangle'}
        aria-pressed={tool === 'rectangle'}
        onclick={() => (tool = 'rectangle')}
      >
        Rect
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'rectangle-fill'}
        aria-pressed={tool === 'rectangle-fill'}
        onclick={() => (tool = 'rectangle-fill')}
      >
        Rect Fill
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'ellipse'}
        aria-pressed={tool === 'ellipse'}
        onclick={() => (tool = 'ellipse')}
      >
        Ellipse
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'ellipse-fill'}
        aria-pressed={tool === 'ellipse-fill'}
        onclick={() => (tool = 'ellipse-fill')}
      >
        Ellipse Fill
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
