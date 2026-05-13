<script lang="ts">
  import { onMount } from 'svelte';
  import { Document, loadCore } from './lib/core';
  import RecoveryDialog from './lib/components/RecoveryDialog.svelte';
  import TileEditor from './lib/components/TileEditor.svelte';
  import TilesetPanel from './lib/components/TilesetPanel.svelte';
  import SlicesPanel from './lib/components/SlicesPanel.svelte';
  import {
    ensureReadPermission,
    hasFsAccess,
    pickAndOpen,
    saveBytes,
    type SaveTarget,
  } from './lib/fs';
  import { isIdbAvailable } from './lib/idb/db';
  import {
    listLatestSnapshots,
    removeSnapshots,
    writeSnapshot,
    type AutosaveSnapshot,
  } from './lib/idb/autosave';
  import {
    listRecents,
    upsertRecent,
    type RecentFile,
  } from './lib/idb/recent-files';
  import {
    blitFrame,
    paintEllipsePreview,
    paintLinePreview,
    paintPivotCrosshair,
    paintRectOutline,
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
    | 'move'
    | 'tilemap-stamp'
    | 'slice';

  // Tools that use the press / drag / release pipeline (a start point
  // captured on `pointerdown`, a live endpoint tracked on
  // `pointermove`, committed on `pointerup`). The Selection (Rect)
  // tool shares the same shape so a Shift constraint / mid-drag
  // pre-empt can be added uniformly; its release path commits via
  // `setSelection` / `clearSelection` rather than the paint commands.
  // The Slice tool reuses the same shape — release commits via
  // `addSlice` (no active slice) or `setSliceKey` (preserving the
  // active slice's center / pivot).
  function isDragShapeTool(t: Tool): boolean {
    return (
      t === 'line' ||
      t === 'rectangle' ||
      t === 'rectangle-fill' ||
      t === 'ellipse' ||
      t === 'ellipse-fill' ||
      t === 'selection-rect' ||
      t === 'slice'
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
  // Current on-disk identity of the document. `handle` is non-null
  // only on File System Access API browsers after a successful
  // open / save-as; subsequent saves write through it in place. `name`
  // is the suggested filename for fallback downloads and the new-file
  // case. See ui/src/lib/fs/index.ts and docs/specs/pincel.md §10.2.
  const DEFAULT_FILE_NAME = 'pincel.aseprite';
  let saveTarget = $state<SaveTarget>({
    name: DEFAULT_FILE_NAME,
    handle: null,
  });
  // Stable across the session; drives Save / Save As button labels.
  const fsAccessAvailable = hasFsAccess();
  // Per-document UUID. Refreshed on every `New` / `Open` /
  // `Open Recent` so each session has a stable identity that the
  // recent-files registry and (M10.3) autosave snapshots can key on.
  // M10.2 only persists FSA-handle-bearing recents — without a handle
  // the entry would be unclickable.
  let docId = $state<string>(crypto.randomUUID());
  // Most-recently-opened recents, refreshed after every successful
  // open / save-as / open-recent. Empty list on non-FSA browsers and
  // before the IDB layer has loaded.
  let recents = $state<RecentFile[]>([]);
  let recentMenuOpen = $state(false);
  const recentsAvailable = fsAccessAvailable && isIdbAvailable();
  // Autosave (M10.3): on a 30-second cadence, snapshot the current
  // `.aseprite` bytes into the IDB `autosave_snapshots` store keyed
  // by `docId`. The interval keeps ticking once `loadCore` resolves;
  // each tick is a no-op when the undo depth hasn't advanced past
  // the last successful save / snapshot, so an idle session never
  // touches IDB. A successful save (`save` / `openRecent` /
  // `openDoc` / `newDoc`) drops the snapshot row and re-baselines
  // `lastWriteUndoDepth` so the next dirty edit re-arms the timer.
  const AUTOSAVE_INTERVAL_MS = 30_000;
  const autosaveAvailable = isIdbAvailable();
  let autosaveTimer: ReturnType<typeof setInterval> | null = null;
  let lastWriteUndoDepth = $state(0);
  let recoverySnapshots = $state<AutosaveSnapshot[]>([]);
  let recoveryOpen = $state(false);
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
  // In-flight Move-tool selection-content drag. Press point and live
  // pointer point are in sprite coords; the delta drives a ghost
  // marquee at the translated location during the drag, and is
  // committed via `applyMoveSelection(dx, dy)` on release. `null`
  // outside a drag. `$state` because the cursor binding switches to
  // a "move" icon while a content drag is in flight.
  let moveSelStart = $state<{ x: number; y: number } | null>(null);
  let moveSelPreview = $state<{ x: number; y: number } | null>(null);
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
  // Bumped whenever the wasm side may have mutated the tileset list
  // (new / open document, undo, redo, or a panel-initiated addTileset).
  // The Tileset Panel reads it as a reactivity tripwire — the wasm
  // getters it polls are opaque to Svelte's reactive graph, so it needs
  // an explicit "something changed" signal to re-derive its list.
  let tilesetRev = $state(0);
  // Active stamp tile for the Tilemap Stamp tool. Set by clicking a
  // tile thumbnail in the Tileset Panel; null clears the stamp and
  // the tool is essentially disabled until one is picked.
  let stampTile = $state<{ tilesetId: number; tileId: number } | null>(null);
  // Live hover position (sprite-space pixels) under the cursor while
  // the Tilemap Stamp tool is active. Drives the grid + cell overlay
  // on the main canvas; null when the cursor leaves the canvas.
  let stampHover = $state<{ x: number; y: number } | null>(null);
  // When non-null, the Tileset Editor sub-mode is open for the named
  // (tileset, tile) pair. The modal `TileEditor` component owns the
  // pointer routing while it is mounted.
  let editingTile = $state<{ tilesetId: number; tileId: number } | null>(null);
  // Currently focused slice in the SlicesPanel — the canvas paints a
  // marching-ants overlay on its frame-0 bounds, and the Slice tool
  // drag commits via `setSliceKey` (preserving center / pivot)
  // rather than `addSlice` when this is set. `null` means no
  // selection — Slice tool drag falls through to `addSlice`.
  let activeSliceId = $state<number | null>(null);

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
        } else if (dragTool === 'selection-rect' || dragTool === 'slice') {
          // Inclusive-corner marquee preview: matches the rect that
          // `commitSelection` / `commitSliceDrag` will hand to the
          // wasm side on release. Slice drags reuse the marching
          // marquee so the gesture matches the active-slice overlay
          // the user is editing.
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
        // No marquee drag in flight. If a Move-tool selection drag is
        // active, paint a ghost marquee at the translated position so
        // the user sees where the selection will land (the pixels
        // themselves snap into place on release — the live drag
        // doesn't bother rasterizing them). Otherwise paint the
        // committed marquee at its stored sprite-space position.
        if (moveSelStart && moveSelPreview) {
          const dx = moveSelPreview.x - moveSelStart.x;
          const dy = moveSelPreview.y - moveSelStart.y;
          paintSelectionMarquee(
            canvas,
            selection.x + dx,
            selection.y + dy,
            selection.w,
            selection.h,
            marchPhase,
          );
        } else {
          paintSelectionMarquee(
            canvas,
            selection.x,
            selection.y,
            selection.w,
            selection.h,
            marchPhase,
          );
        }
      }
      // Tilemap Stamp tool: overlay the tile grid and highlight the
      // cell under the cursor so the user sees exactly where a click
      // will land. Drawn after the selection marquee so a marquee +
      // stamp tool combo doesn't hide one of them.
      if (tool === 'tilemap-stamp' && stampTile && doc) {
        paintTileGridOverlay();
      }
      // Active slice overlay: paint frame-0 bounds as marching ants,
      // and the optional 9-patch center / pivot as static accents.
      // Drawn last so the slice's geometry stays visible above the
      // tile grid or selection marquee when several overlays would
      // otherwise compete.
      if (doc && activeSliceId !== null) {
        paintActiveSliceOverlay();
      }
    } finally {
      frame.free();
    }
  }

  // Paint marching ants on the active slice's frame-0 bounds, plus
  // its optional 9-patch center band and pivot crosshair. The wasm
  // getters that back this run only when `activeSliceId` is set; if
  // the slice was just removed via undo, the function silently
  // no-ops via the try/catch wrapping the bounds read.
  function paintActiveSliceOverlay() {
    if (!canvas || !doc || activeSliceId === null) return;
    try {
      const id = activeSliceId;
      const keyCount = doc.sliceKeyCount(id);
      if (keyCount === 0) return;
      // Prefer the explicit frame-0 key when present, falling back to
      // key 0 so a slice that only carries a later-frame key still
      // gets a visible overlay.
      let keyIndex = 0;
      for (let k = 0; k < keyCount; k += 1) {
        if (doc.sliceKeyFrame(id, k) === 0) {
          keyIndex = k;
          break;
        }
      }
      const x = doc.sliceKeyX(id, keyIndex);
      const y = doc.sliceKeyY(id, keyIndex);
      const w = doc.sliceKeyWidth(id, keyIndex);
      const h = doc.sliceKeyHeight(id, keyIndex);
      paintSelectionMarquee(canvas, x, y, w, h, marchPhase);
      if (doc.sliceKeyHasCenter(id, keyIndex)) {
        const cx = doc.sliceKeyCenterX(id, keyIndex);
        const cy = doc.sliceKeyCenterY(id, keyIndex);
        const cw = doc.sliceKeyCenterWidth(id, keyIndex);
        const ch = doc.sliceKeyCenterHeight(id, keyIndex);
        // Reuse the slice's editor color for the center accent so
        // 9-patch slices read as distinct from the active marquee.
        const color = sliceColorCss(doc.sliceColor(id));
        paintRectOutline(canvas!, cx, cy, cw, ch, color);
      }
      if (doc.sliceKeyHasPivot(id, keyIndex)) {
        const px = doc.sliceKeyPivotX(id, keyIndex);
        const py = doc.sliceKeyPivotY(id, keyIndex);
        paintPivotCrosshair(canvas!, px, py);
      }
    } catch {
      // Slice disappeared mid-frame (e.g. undo of an addSlice).
      // `reconcileActiveSlice` runs after undo / redo (the same
      // place `tilesetRev` bumps) and clears the stale id.
    }
  }

  // Drop the slice color's alpha and render `#RRGGBB` so the 2D
  // context's `fillStyle` accepts it. Alpha is intentionally
  // dropped — overlays always paint fully opaque to stay legible
  // against the composed sprite.
  function sliceColorCss(packed: number): string {
    const rgb = (packed >>> 8) & 0xffffff;
    return '#' + rgb.toString(16).padStart(6, '0');
  }

  // Paint a 1-CSS-pixel grid over the main canvas at the active
  // stamp tileset's tile_size, then highlight the cell under the
  // cursor. Coordinates use the canvas's intrinsic pixel space
  // (`canvas.width`/`height` == sprite dimensions); CSS scaling does
  // the visual upscale via `image-rendering: pixelated`.
  function paintTileGridOverlay() {
    if (!canvas || !doc || !stampTile) return;
    const tileW = doc.tilesetTileWidth(stampTile.tilesetId);
    const tileH = doc.tilesetTileHeight(stampTile.tilesetId);
    if (tileW === 0 || tileH === 0) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.save();
    ctx.strokeStyle = 'rgba(96, 165, 250, 0.5)';
    ctx.lineWidth = 1;
    for (let gx = tileW; gx < canvas.width; gx += tileW) {
      ctx.beginPath();
      ctx.moveTo(gx + 0.5, 0);
      ctx.lineTo(gx + 0.5, canvas.height);
      ctx.stroke();
    }
    for (let gy = tileH; gy < canvas.height; gy += tileH) {
      ctx.beginPath();
      ctx.moveTo(0, gy + 0.5);
      ctx.lineTo(canvas.width, gy + 0.5);
      ctx.stroke();
    }
    if (stampHover) {
      const cellX = Math.floor(stampHover.x / tileW) * tileW;
      const cellY = Math.floor(stampHover.y / tileH) * tileH;
      ctx.strokeStyle = 'rgba(59, 130, 246, 0.95)';
      ctx.lineWidth = 2;
      ctx.strokeRect(cellX + 0.5, cellY + 0.5, tileW - 1, tileH - 1);
    }
    ctx.restore();
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

  // Find a tilemap layer bound to `tilesetId`. The Stamp tool needs
  // a target layer; today we auto-pick the topmost matching tilemap
  // layer (highest z-index), which matches the natural "the last
  // tilemap layer you added is the one you draw on" expectation.
  // An explicit active-layer selector lands when the Layers panel
  // ships.
  function activeTilemapLayerForTileset(tilesetId: number): number | null {
    if (!doc) return null;
    for (let i = doc.layerCount - 1; i >= 0; i -= 1) {
      let layerId: number;
      try {
        layerId = doc.layerIdAt(i);
      } catch {
        continue;
      }
      if (
        doc.layerKind(layerId) === 'tilemap' &&
        doc.layerTilesetId(layerId) === tilesetId
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
    if (!doc || !stampTile) return;
    const layerId = activeTilemapLayerForTileset(stampTile.tilesetId);
    if (layerId === null) {
      status = 'add a tilemap layer first (+ Layer in the Tilesets panel)';
      return;
    }
    const tileW = doc.tilesetTileWidth(stampTile.tilesetId);
    const tileH = doc.tilesetTileHeight(stampTile.tilesetId);
    if (tileW === 0 || tileH === 0) return;
    if (point.x < 0 || point.y < 0) return;
    const gx = Math.floor(point.x / tileW);
    const gy = Math.floor(point.y / tileH);
    try {
      doc.placeTile(layerId, 0, gx, gy, stampTile.tileId);
      dirty = true;
      syncMeta();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `stamp failed: ${msg}`;
    }
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
    // Space-drag always pans, regardless of the active tool (spec §5.2
    // — Move tool "Pans canvas with space-drag"). Move-tool press
    // splits on whether a selection is active: with one, drag the
    // selection content (M7.7b); without one, fall back to viewport
    // pan (M7.7a). spaceDown wins so the user can always pan even
    // when the Move tool would otherwise translate the marquee.
    if (spaceDown) {
      panning = true;
      panStartClient = { x: e.clientX, y: e.clientY };
      panStartOffset = { x: panX, y: panY };
      return;
    }
    if (tool === 'move') {
      if (selection) {
        const point = spriteCoord(e);
        if (point) {
          moveSelStart = point;
          moveSelPreview = point;
          dirty = true;
        }
        return;
      }
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
    if (tool === 'tilemap-stamp') {
      // Stamp commits once per click for the same reason as Bucket.
      // Drag-to-paint over tiles can land alongside auto-tile mode
      // in Phase 2 (spec §5.3 / §13.2).
      const point = spriteCoord(e);
      if (point) commitStamp(point);
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

  // Commit a Slice tool drag from the two corner points of the
  // press / drag / release gesture. Routes to `addSlice` when no
  // slice is active (creating a new slice with the drawn bounds), or
  // to `setSliceKey` when a slice is active (preserving its 9-patch
  // center and pivot). Degenerate (single-pixel) drags are accepted
  // as 1×1 slices — the underlying commands reject empty bounds, so
  // a click without a drag still surfaces a valid 1-pixel slice.
  function commitSliceDrag(x0: number, y0: number, x1: number, y1: number) {
    if (!doc) return;
    const minX = Math.min(x0, x1);
    const minY = Math.min(y0, y1);
    const w = Math.abs(x1 - x0) + 1;
    const h = Math.abs(y1 - y0) + 1;
    try {
      if (activeSliceId === null) {
        const autoName = `Slice ${doc.sliceCount + 1}`;
        const packed = packColor(color);
        const newId = doc.addSlice(autoName, minX, minY, w, h, packed);
        activeSliceId = newId;
      } else {
        const id = activeSliceId;
        // Preserve the active slice's 9-patch center and pivot
        // around the new bounds. The wasm side validates the
        // partial-quartet invariant on its own.
        let keyIndex = 0;
        const keyCount = doc.sliceKeyCount(id);
        for (let k = 0; k < keyCount; k += 1) {
          if (doc.sliceKeyFrame(id, k) === 0) {
            keyIndex = k;
            break;
          }
        }
        const hasCenter =
          keyCount > 0 ? doc.sliceKeyHasCenter(id, keyIndex) : false;
        const hasPivot =
          keyCount > 0 ? doc.sliceKeyHasPivot(id, keyIndex) : false;
        doc.setSliceKey(
          id,
          0,
          minX,
          minY,
          w,
          h,
          hasCenter ? doc.sliceKeyCenterX(id, keyIndex) : undefined,
          hasCenter ? doc.sliceKeyCenterY(id, keyIndex) : undefined,
          hasCenter ? doc.sliceKeyCenterWidth(id, keyIndex) : undefined,
          hasCenter ? doc.sliceKeyCenterHeight(id, keyIndex) : undefined,
          hasPivot ? doc.sliceKeyPivotX(id, keyIndex) : undefined,
          hasPivot ? doc.sliceKeyPivotY(id, keyIndex) : undefined,
        );
      }
      tilesetRev += 1;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('slice commit failed', err);
      status = `slice failed: ${msg}`;
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
    if (moveSelStart) {
      const point = spriteCoord(e);
      if (!point) return;
      moveSelPreview = point;
      dirty = true;
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
    if (tool === 'tilemap-stamp') {
      // Update the hover indicator regardless of button state — the
      // overlay should follow the cursor whether or not the user is
      // mid-click. `dirty = true` triggers a recompose so the grid
      // overlay redraws at the new cell.
      const point = spriteCoord(e);
      stampHover = point;
      dirty = true;
      return;
    }
    if (!painting) return;
    paintAt(e);
  }

  function onPointerLeave() {
    if (tool === 'tilemap-stamp' && stampHover) {
      stampHover = null;
      dirty = true;
    }
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
    if (moveSelStart && moveSelPreview && doc) {
      const dx = moveSelPreview.x - moveSelStart.x;
      const dy = moveSelPreview.y - moveSelStart.y;
      if (dx !== 0 || dy !== 0) {
        try {
          doc.applyMoveSelection(dx, dy);
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          console.error('applyMoveSelection failed', err);
          status = `move failed: ${msg}`;
        }
      }
      moveSelStart = null;
      moveSelPreview = null;
      dirty = true;
      syncMeta();
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
        } else if (dragTool === 'slice') {
          commitSliceDrag(dragStart.x, dragStart.y, end.x, end.y);
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
    tilesetRev += 1;
    stampTile = null;
    stampHover = null;
    editingTile = null;
    activeSliceId = null;
    saveTarget = { name: DEFAULT_FILE_NAME, handle: null };
    docId = crypto.randomUUID();
    lastWriteUndoDepth = doc.undoDepth;
    status = 'new 64×64 document';
  }

  async function openDoc() {
    let opened;
    try {
      opened = await pickAndOpen();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `open failed: ${msg}`;
      return;
    }
    if (!opened) return;
    try {
      const next = Document.openAseprite(opened.bytes);
      disposeDoc();
      doc = next;
      dirty = true;
      syncMeta();
      syncSelection();
      tilesetRev += 1;
      stampTile = null;
      stampHover = null;
      editingTile = null;
      activeSliceId = null;
      saveTarget = { name: opened.name, handle: opened.handle };
      docId = crypto.randomUUID();
      lastWriteUndoDepth = doc.undoDepth;
      status = `opened ${opened.name} · ${doc.width}×${doc.height}`;
      await clearAutosave();
      await recordRecent();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `open failed: ${msg}`;
    }
  }

  async function save(opts: { forceAs?: boolean } = {}) {
    if (!doc) return;
    const forceAs = opts.forceAs ?? false;
    try {
      const bytes = new Uint8Array(doc.saveAseprite());
      const next = await saveBytes(bytes, saveTarget, { forceAs });
      saveTarget = next;
      lastWriteUndoDepth = doc.undoDepth;
      status = `saved ${bytes.length} bytes · ${next.name}`;
      await clearAutosave();
      await recordRecent();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `save failed: ${msg}`;
    }
  }

  // Drop autosave snapshots for the current `docId`. Called after
  // every successful save / open so the IDB store only ever holds
  // snapshots that represent unsaved edits.
  async function clearAutosave() {
    if (!autosaveAvailable) return;
    try {
      await removeSnapshots(docId);
    } catch (err) {
      // Best-effort; log but don't surface — the user's save itself
      // succeeded.
      console.error('autosave clear failed', err);
    }
  }

  // One autosave tick. No-op when nothing to snapshot (no doc, no
  // change since last write, IDB unavailable). Surfacing failure in
  // the status bar is intentional — autosave silently failing is the
  // worst failure mode for this feature.
  async function autosaveTick() {
    if (!autosaveAvailable || !doc) return;
    if (doc.undoDepth === lastWriteUndoDepth) return;
    const depthAtSnapshot = doc.undoDepth;
    let bytes: Uint8Array;
    try {
      bytes = new Uint8Array(doc.saveAseprite());
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('autosave encode failed', err);
      status = `autosave failed: ${msg}`;
      return;
    }
    try {
      await writeSnapshot(docId, saveTarget.name, bytes);
      lastWriteUndoDepth = depthAtSnapshot;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('autosave write failed', err);
      status = `autosave failed: ${msg}`;
    }
  }

  // Load the recovered bytes into a fresh `Document`, replacing the
  // current one. Re-binds `docId` to the snapshot's id so subsequent
  // saves and autosaves stay grouped under the same identity — this
  // is what makes the recovered session feel like a continuation
  // rather than a copy. The snapshot row is dropped on success.
  async function applyRecovery(snap: AutosaveSnapshot) {
    try {
      const next = Document.openAseprite(snap.bytes);
      disposeDoc();
      doc = next;
      dirty = true;
      syncMeta();
      syncSelection();
      tilesetRev += 1;
      stampTile = null;
      stampHover = null;
      editingTile = null;
      activeSliceId = null;
      saveTarget = { name: snap.name, handle: null };
      docId = snap.docId;
      lastWriteUndoDepth = doc.undoDepth;
      status = `recovered ${snap.name} · ${doc.width}×${doc.height}`;
      await clearAutosave();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `recover failed: ${msg}`;
    }
    closeRecovery();
  }

  async function discardSnapshot(targetDocId: string) {
    try {
      await removeSnapshots(targetDocId);
    } catch (err) {
      console.error('discard snapshot failed', err);
    }
    recoverySnapshots = recoverySnapshots.filter(
      (s) => s.docId !== targetDocId,
    );
    if (recoverySnapshots.length === 0) closeRecovery();
  }

  function closeRecovery() {
    recoveryOpen = false;
    recoverySnapshots = [];
  }

  // Persist the current `saveTarget` to the recent-files registry and
  // refresh the in-memory list. No-op when the registry isn't
  // available (non-FSA browsers / IDB disabled) or the current target
  // doesn't carry a handle (a download-only save, or a New document
  // that hasn't been written yet).
  async function recordRecent() {
    if (!recentsAvailable) return;
    if (!saveTarget.handle) return;
    try {
      await upsertRecent({
        id: docId,
        name: saveTarget.name,
        handle: saveTarget.handle,
      });
      recents = await listRecents();
    } catch (err) {
      // Persistence is best-effort; surface to the status line but
      // don't block the save / open the user already completed.
      const msg = err instanceof Error ? err.message : String(err);
      status = `${status} (recents update failed: ${msg})`;
    }
  }

  async function openRecent(r: RecentFile) {
    recentMenuOpen = false;
    if (!r.handle) {
      status = `recent ${r.name}: no handle`;
      return;
    }
    try {
      // Read access is enough to open. The next save will prompt
      // for write via `saveBytes` → `ensureReadWritePermission`.
      if (!(await ensureReadPermission(r.handle))) {
        status = `recent ${r.name}: permission denied`;
        return;
      }
      const file = await r.handle.getFile();
      const bytes = new Uint8Array(await file.arrayBuffer());
      const next = Document.openAseprite(bytes);
      disposeDoc();
      doc = next;
      dirty = true;
      syncMeta();
      syncSelection();
      tilesetRev += 1;
      stampTile = null;
      stampHover = null;
      editingTile = null;
      activeSliceId = null;
      saveTarget = { name: file.name, handle: r.handle };
      // Preserve the recent's id so re-opens count as the same doc
      // and autosave snapshots survive across page reloads. The
      // open itself is treated as a successful "write to disk" event
      // — its bytes match the on-disk state, so any pending
      // autosave snapshot for this id is stale and gets cleared.
      docId = r.id;
      lastWriteUndoDepth = doc.undoDepth;
      status = `opened ${file.name} · ${doc.width}×${doc.height}`;
      await clearAutosave();
      await recordRecent();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `recent ${r.name} open failed: ${msg}`;
    }
  }

  function undo() {
    if (doc?.undo()) {
      dirty = true;
      syncMeta();
      tilesetRev += 1;
      reconcileActiveSlice();
    }
  }

  function redo() {
    if (!doc) return;
    try {
      if (doc.redo()) {
        dirty = true;
        syncMeta();
        tilesetRev += 1;
        reconcileActiveSlice();
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `redo failed: ${msg}`;
    }
  }

  // If `activeSliceId` references a slice that no longer exists
  // (typical after `undo` of an `addSlice`), drop the local pointer
  // so the marching-ants overlay and the panel highlight stop
  // tracking a phantom id.
  function reconcileActiveSlice() {
    if (!doc || activeSliceId === null) return;
    const count = doc.sliceCount;
    for (let i = 0; i < count; i += 1) {
      try {
        if (doc.sliceIdAt(i) === activeSliceId) return;
      } catch {
        // ignore
      }
    }
    activeSliceId = null;
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
      if (
        selection ||
        (dragStart && (dragTool === 'selection-rect' || dragTool === 'slice')) ||
        moveSelStart ||
        activeSliceId !== null
      ) {
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
      return;
    }
    if (e.key === 'Escape' && recentMenuOpen) {
      recentMenuOpen = false;
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
    moveSelStart = null;
    moveSelPreview = null;
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
        lastWriteUndoDepth = doc.undoDepth;
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
    // Best-effort recents load — failures are silent (the dropdown
    // just stays empty / hidden).
    if (recentsAvailable) {
      listRecents()
        .then((rows) => {
          if (!cancelled) recents = rows;
        })
        .catch((err: unknown) => {
          console.error('listRecents failed', err);
        });
    }
    // Recovery probe: surface any snapshots from a prior session.
    // The interval starts here too — the timer body short-circuits
    // until `doc` is non-null, so it's safe to arm before
    // `loadCore` resolves.
    if (autosaveAvailable) {
      listLatestSnapshots()
        .then((snaps) => {
          if (cancelled || snaps.length === 0) return;
          recoverySnapshots = snaps;
          recoveryOpen = true;
        })
        .catch((err: unknown) => {
          console.error('autosave probe failed', err);
        });
      autosaveTimer = setInterval(() => {
        void autosaveTick();
      }, AUTOSAVE_INTERVAL_MS);
    }
    return () => {
      cancelled = true;
      if (rafHandle !== null) cancelAnimationFrame(rafHandle);
      if (autosaveTimer !== null) {
        clearInterval(autosaveTimer);
        autosaveTimer = null;
      }
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
    <button class="toolbar-btn" onclick={openDoc}>Open…</button>
    <button class="toolbar-btn" onclick={() => save()} disabled={!doc}>
      {fsAccessAvailable ? 'Save' : 'Save As (download)'}
    </button>
    {#if fsAccessAvailable}
      <button
        class="toolbar-btn"
        onclick={() => save({ forceAs: true })}
        disabled={!doc}
      >
        Save As…
      </button>
    {/if}
    {#if recentsAvailable}
      <div class="relative">
        <button
          class="toolbar-btn"
          class:toolbar-btn-active={recentMenuOpen}
          aria-haspopup="menu"
          aria-expanded={recentMenuOpen}
          disabled={recents.length === 0}
          onclick={() => (recentMenuOpen = !recentMenuOpen)}
        >
          Recent…
        </button>
        {#if recentMenuOpen}
          <ul
            class="absolute left-0 top-full z-10 mt-1 flex min-w-48 flex-col rounded border border-neutral-700 bg-neutral-900 py-1 shadow-lg"
            aria-label="Recent files"
          >
            {#each recents as r (r.id)}
              <li>
                <button
                  class="w-full truncate px-3 py-1 text-left text-xs hover:bg-neutral-800"
                  title={r.name}
                  onclick={() => openRecent(r)}
                >
                  {r.name}
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
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
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'tilemap-stamp'}
        aria-pressed={tool === 'tilemap-stamp'}
        onclick={() => (tool = 'tilemap-stamp')}
        disabled={!stampTile}
        title={stampTile
          ? `stamp tile ${stampTile.tileId} from tileset ${stampTile.tilesetId}`
          : 'pick a tile in the Tilesets panel first'}
      >
        Stamp
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'slice'}
        aria-pressed={tool === 'slice'}
        onclick={() => (tool = 'slice')}
        title={activeSliceId !== null
          ? 'drag to resize the active slice'
          : 'drag to create a slice'}
      >
        Slice
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

  <section class="flex flex-1 overflow-hidden">
    <div class="relative flex flex-1 items-center justify-center overflow-hidden">
      <canvas
        bind:this={canvas}
        class="canvas-pixelated shrink-0 touch-none border border-neutral-700 bg-neutral-900 shadow-lg"
        style:width="{canvasW * zoom}px"
        style:height="{canvasH * zoom}px"
        style:transform="translate({panX}px, {panY}px)"
        style:cursor={panning
          ? 'grabbing'
          : moveSelStart
            ? 'move'
            : tool === 'move' && selection && !spaceDown
              ? 'move'
              : tool === 'move' || spaceDown
                ? 'grab'
                : 'crosshair'}
        aria-label="Pincel canvas"
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointercancel={onPointerUp}
        onpointerleave={onPointerLeave}
      ></canvas>
      {#if doc && editingTile}
        <TileEditor
          {doc}
          tilesetId={editingTile.tilesetId}
          tileId={editingTile.tileId}
          {color}
          rev={tilesetRev}
          onClose={() => (editingTile = null)}
          onChange={() => {
            tilesetRev += 1;
            dirty = true;
            syncMeta();
          }}
        />
      {/if}
    </div>
    <TilesetPanel
      {doc}
      rev={tilesetRev}
      selectedTile={stampTile}
      onChange={() => (tilesetRev += 1)}
      onSelectStampTile={(tilesetId, tileId) => {
        stampTile = { tilesetId, tileId };
        // Auto-switch to the Stamp tool so a single click on a tile
        // is enough to start placing. The user can switch back via
        // the toolbar if they wanted to keep editing pixels.
        tool = 'tilemap-stamp';
      }}
      onEditTile={(tilesetId, tileId) => {
        editingTile = { tilesetId, tileId };
      }}
    />
    <SlicesPanel
      {doc}
      rev={tilesetRev}
      {activeSliceId}
      onChange={() => (tilesetRev += 1)}
      onActivate={(sliceId) => {
        activeSliceId = sliceId;
        if (sliceId !== null) tool = 'slice';
        dirty = true;
      }}
    />
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

{#if recoveryOpen && recoverySnapshots.length > 0}
  <RecoveryDialog
    snapshots={recoverySnapshots}
    onRecover={(snap) => {
      void applyRecovery(snap);
    }}
    onDiscard={(targetDocId) => {
      void discardSnapshot(targetDocId);
    }}
    onDismiss={closeRecovery}
  />
{/if}

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
