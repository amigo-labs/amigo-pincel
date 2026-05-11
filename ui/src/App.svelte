<script lang="ts">
  import { onMount } from 'svelte';
  import { Document, loadCore } from './lib/core';
  import {
    blitFrame,
    paintEllipsePreview,
    paintLinePreview,
    paintRectanglePreview,
    paintSelectionMarquee,
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
    | 'ellipse-fill'
    | 'selection-rect'
    | 'move';

  // Tools that use the press / drag / release pipeline (a start point
  // captured on `pointerdown`, a live endpoint tracked on
  // `pointermove`, committed on `pointerup`). The Selection (Rect)
  // tool shares the same shape so a Shift constraint / mid-drag
  // pre-empt can be added uniformly; its release path commits via
  // `setSelection` / `clearSelection` rather than the paint commands.
  function isDragShapeTool(t: Tool): boolean {
    return (
      t === 'line' ||
      t === 'rectangle' ||
      t === 'rectangle-fill' ||
      t === 'ellipse' ||
      t === 'ellipse-fill' ||
      t === 'selection-rect'
    );
  }

  const MIN_ZOOM = 1;
  const MAX_ZOOM = 64;

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
  // Display zoom: integer multiplier from sprite pixels to CSS pixels.
  // Independent of `pincel_core::compose`'s zoom arg — we still ask
  // the wasm side for a 1× framebuffer and let `image-rendering:
  // pixelated` upscale it in CSS. 8× preserves the M6.6 64×64 → 512×512
  // default look.
  let zoom = $state(8);
  // Pan offset (CSS pixels) applied as a `transform: translate(...)`
  // on the canvas, relative to the flex-centered layout box. Zero
  // means "centered in the viewport".
  let panX = $state(0);
  let panY = $state(0);
  // In-flight pan drag (Move tool press, or space-drag temporary
  // override). `panStartClient` is the pointer position at press-time
  // and `panStartOffset` snapshots `panX` / `panY` so cursor deltas
  // translate one-to-one into pan deltas.
  let panning = $state(false);
  let panStartClient: { x: number; y: number } | null = null;
  let panStartOffset: { x: number; y: number } | null = null;
  // Whether the space key is currently held. Triggers temporary
  // pan-on-drag regardless of the active tool (spec §5.2 — Move tool
  // "Pans canvas with space-drag").
  let spaceDown = $state(false);
  // Local mirror of the wasm `Sprite::selection`, updated whenever a
  // `selection-changed` event drains (or on doc replacement). `null`
  // when no selection is active. The marching-ants overlay reads this
  // each recompose; the wasm side stays the source of truth.
  let selection = $state<{ x: number; y: number; w: number; h: number } | null>(
    null,
  );
  // Phase counter for the marching-ants animation: increments mod 4
  // every `MARCH_FRAMES_PER_STEP` RAF ticks while a selection is
  // active, producing the classic clockwise crawl. Frozen at the
  // current value when the selection is cleared so the next selection
  // starts wherever the previous one left off (visually consistent).
  let marchPhase = $state(0);
  let marchTicks = 0;
  // ~60 / 7 ≈ 8.5 Hz, matching Aseprite's marquee crawl rate. The
  // overlay redraws once per phase step; intermediate ticks skip the
  // recompose so an idle selection costs near-zero CPU.
  const MARCH_FRAMES_PER_STEP = 7;

  function syncMeta() {
    if (!doc) return;
    undoDepth = doc.undoDepth;
    redoDepth = doc.redoDepth;
    canvasW = doc.width;
    canvasH = doc.height;
  }

  // Re-read the selection rect from the wasm side. Called after any
  // event drain that may have included a `selection-changed`, and on
  // doc replacement (new / open) so the overlay reflects the loaded
  // sprite (always `None` today — Aseprite files do not persist
  // selection, M7.8a follow-up).
  function syncSelection() {
    if (!doc || !doc.hasSelection) {
      selection = null;
      return;
    }
    selection = {
      x: doc.selectionX,
      y: doc.selectionY,
      w: doc.selectionWidth,
      h: doc.selectionHeight,
    };
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
        } else if (dragTool === 'selection-rect') {
          // Inclusive-corner marquee preview: matches the rect that
          // `commitSelection` will hand to `setSelection` on release
          // (so the user sees the exact pixels that will be inside the
          // committed selection).
          const minX = Math.min(dragStart.x, end.x);
          const maxX = Math.max(dragStart.x, end.x);
          const minY = Math.min(dragStart.y, end.y);
          const maxY = Math.max(dragStart.y, end.y);
          paintSelectionMarquee(
            canvas,
            minX,
            minY,
            maxX - minX + 1,
            maxY - minY + 1,
            marchPhase,
          );
        }
      } else if (selection) {
        // No in-flight drag: paint the committed marquee. Drawn after
        // the blit so the ants ride on top of any composed pixels.
        paintSelectionMarquee(
          canvas,
          selection.x,
          selection.y,
          selection.w,
          selection.h,
          marchPhase,
        );
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

  // Clamp + apply a new zoom level. Pan offset stays in CSS pixels,
  // so the canvas's flex-centered position keeps the sprite center
  // anchored across zoom changes; further pan offsets shift uniformly
  // from there.
  function setZoom(next: number) {
    next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, Math.floor(next)));
    if (next === zoom) return;
    zoom = next;
  }

  function zoomIn() {
    setZoom(zoom * 2);
  }

  function zoomOut() {
    setZoom(zoom >>> 1);
  }

  // Default view: 8× zoom + zero pan offset. Matches the M6.6 default
  // look (64×64 canvas at 512×512 CSS) and re-centers the canvas in
  // the viewport regardless of where it had been dragged.
  function resetView() {
    zoom = 8;
    panX = 0;
    panY = 0;
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
    if (isDragShapeTool(tool) || tool === 'move') return;
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
    // Move tool press and space-drag both activate a pan drag. The
    // active-tool check stays first so the Move tool works even when
    // space happens to be down at press time.
    if (tool === 'move' || spaceDown) {
      panning = true;
      panStartClient = { x: e.clientX, y: e.clientY };
      panStartOffset = { x: panX, y: panY };
      return;
    }
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

  // Commit a selection marquee from the two corner points of the
  // press / drag / release gesture. A no-move click (start === end)
  // clears the selection, matching Aseprite's "click outside to
  // deselect" UX; otherwise the rect is normalized to min / max
  // corners (inclusive) and forwarded to `setSelection`. Sprite-bounds
  // clipping is intentionally not applied here — the wasm side stores
  // the raw rect and the marching-ants overlay's per-pixel clip keeps
  // off-canvas extents from leaking visually.
  function commitSelection(x0: number, y0: number, x1: number, y1: number) {
    if (!doc) return;
    if (x0 === x1 && y0 === y1) {
      doc.clearSelection();
      return;
    }
    const minX = Math.min(x0, x1);
    const minY = Math.min(y0, y1);
    const w = Math.abs(x1 - x0) + 1;
    const h = Math.abs(y1 - y0) + 1;
    doc.setSelection(minX, minY, w, h);
  }

  function onPointerMove(e: PointerEvent) {
    if (panning && panStartClient && panStartOffset) {
      panX = panStartOffset.x + (e.clientX - panStartClient.x);
      panY = panStartOffset.y + (e.clientY - panStartClient.y);
      return;
    }
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
    if (panning) {
      panning = false;
      panStartClient = null;
      panStartOffset = null;
      return;
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
        } else if (dragTool === 'selection-rect') {
          commitSelection(dragStart.x, dragStart.y, end.x, end.y);
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
    syncSelection();
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
      syncSelection();
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
      let selectionTouched = false;
      if (events.length > 0) dirty = true;
      for (const ev of events) {
        if (ev.kind === 'selection-changed') selectionTouched = true;
        ev.free();
      }
      if (selectionTouched) syncSelection();
      // Drive the marching-ants animation: when a selection is
      // active, advance `marchPhase` once every
      // `MARCH_FRAMES_PER_STEP` ticks and force a re-render so the
      // overlay redraws. With no selection the counters reset and the
      // recompose path stays idle.
      if (selection || (dragStart && dragTool === 'selection-rect')) {
        marchTicks += 1;
        if (marchTicks >= MARCH_FRAMES_PER_STEP) {
          marchTicks = 0;
          marchPhase = (marchPhase + 1) & 0x3;
          dirty = true;
        }
      } else {
        marchTicks = 0;
      }
      if (dirty) {
        dirty = false;
        recompose();
        syncMeta();
      }
    }
    rafHandle = requestAnimationFrame(tick);
  }

  // Filter space presses originating in any form input or
  // contenteditable element so the user keeps native space-bar behavior
  // there (typing a literal space in a future search / filename box,
  // toggling a checkbox, etc.). The current toolbar only exposes a
  // color input (no text intake) and a hidden file input, but the
  // guard is forward-looking and conservative — any `<input>`,
  // `<textarea>`, or contenteditable target opts out.
  function isEditableTarget(target: EventTarget | null): boolean {
    if (!(target instanceof HTMLElement)) return false;
    const tag = target.tagName;
    return tag === 'INPUT' || tag === 'TEXTAREA' || target.isContentEditable;
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.code === 'Space' && !e.repeat && !isEditableTarget(e.target)) {
      // Prevent the browser from page-scrolling on space.
      e.preventDefault();
      spaceDown = true;
    }
  }

  function onKeyUp(e: KeyboardEvent) {
    if (e.code === 'Space') {
      spaceDown = false;
    }
  }

  // If the window loses focus (alt-tab, DevTools, OS shortcut) between
  // a Space keydown and keyup, the keyup never reaches us and
  // `spaceDown` stays stuck — and any in-flight pan drag would similarly
  // outlive the gesture. Clear both on blur / hidden so the user
  // returns to a clean state.
  function onWindowBlur() {
    spaceDown = false;
    panning = false;
    panStartClient = null;
    panStartOffset = null;
  }

  function onVisibilityChange() {
    if (document.hidden) onWindowBlur();
  }

  onMount(() => {
    let cancelled = false;
    loadCore()
      .then(() => {
        if (cancelled) return;
        doc = new Document(64, 64);
        syncMeta();
        syncSelection();
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
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('keyup', onKeyUp);
    window.addEventListener('blur', onWindowBlur);
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      cancelled = true;
      if (rafHandle !== null) cancelAnimationFrame(rafHandle);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onWindowBlur);
      document.removeEventListener('visibilitychange', onVisibilityChange);
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
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'selection-rect'}
        aria-pressed={tool === 'selection-rect'}
        onclick={() => (tool = 'selection-rect')}
      >
        Select
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'move'}
        aria-pressed={tool === 'move'}
        onclick={() => (tool = 'move')}
      >
        Move
      </button>
    </span>
    <span class="ml-2 flex items-center gap-1" role="group" aria-label="Zoom">
      <button
        class="toolbar-btn"
        onclick={zoomOut}
        disabled={zoom <= MIN_ZOOM}
        aria-label="Zoom out"
      >
        −
      </button>
      <span class="w-10 text-center text-xs text-neutral-400">{zoom}×</span>
      <button
        class="toolbar-btn"
        onclick={zoomIn}
        disabled={zoom >= MAX_ZOOM}
        aria-label="Zoom in"
      >
        +
      </button>
      <button class="toolbar-btn" onclick={resetView} aria-label="Reset view">
        Reset
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

  <section class="flex flex-1 items-center justify-center overflow-hidden">
    <canvas
      bind:this={canvas}
      class="canvas-pixelated shrink-0 touch-none border border-neutral-700 bg-neutral-900 shadow-lg"
      style:width="{canvasW * zoom}px"
      style:height="{canvasH * zoom}px"
      style:transform="translate({panX}px, {panY}px)"
      style:cursor={panning
        ? 'grabbing'
        : tool === 'move' || spaceDown
          ? 'grab'
          : 'crosshair'}
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
