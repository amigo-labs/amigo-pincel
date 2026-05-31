<script lang="ts">
  import { onMount } from 'svelte';
  import { Document, loadCore } from './lib/core';
  import RecoveryDialog from './lib/components/RecoveryDialog.svelte';
  import TileEditor from './lib/components/TileEditor.svelte';
  import TilesetPanel from './lib/components/TilesetPanel.svelte';
  import SlicesPanel from './lib/components/SlicesPanel.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import FileAssocDialog from './lib/components/FileAssocDialog.svelte';
  import {
    ensureReadPermission,
    hasFsAccess,
    pickAndOpen,
    saveBytes,
    type SaveTarget,
  } from './lib/fs';
  import { getPref, setPref } from './lib/idb/prefs';
  import { syncRecentMenu, wireNativeMenu } from './lib/menu';
  import { isTauri } from './lib/platform';
  import { isIdbAvailable } from './lib/idb/db';
  import {
    latestSnapshot,
    listLatestSnapshots,
    removeSnapshots,
    writeSnapshot,
    type AutosaveSnapshotMeta,
  } from './lib/idb/autosave';
  import {
    listRecents,
    upsertRecent,
    type RecentFile,
  } from './lib/idb/recent-files';
  import {
    paintEllipsePreview,
    paintLinePreview,
    paintPivotCrosshair,
    paintRectOutline,
    paintRectanglePreview,
    paintSelectionMarquee,
  } from './lib/render/canvas2d';
  import { Canvas2DRenderer } from './lib/render/canvas2d-renderer';
  import type { CanvasRenderer, RenderBackend } from './lib/render/types';
  import { WebGPURenderer } from './lib/render/webgpu-renderer';
  import { fitZoom } from './lib/view/fit';

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
  // CSS px of breathing room left on each edge when auto-fitting a
  // sprite to the viewport (see `fitView`).
  const FIT_MARGIN = 24;

  let canvas = $state<HTMLCanvasElement | null>(null);
  // Transparent Canvas2D layer stacked over the base canvas; carries the
  // marching-ants marquee, drag-shape previews, tile grid, and active
  // slice accents so the base layer can be WebGPU (spec §4.4, M12.5).
  let overlay = $state<HTMLCanvasElement | null>(null);
  // The base-layer blit surface. Created in `onMount` once the base
  // canvas binds — WebGPU when available, else Canvas2D. Not reactive:
  // only the render path touches it.
  let renderer: CanvasRenderer | null = null;
  // Active backend, surfaced in the footer (M12.5). `'none'` until the
  // renderer is selected.
  let backend = $state<RenderBackend | 'none'>('none');
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
  // Live size of the flex-centered canvas stage (the `overflow-hidden`
  // wrapper). Bound to the wrapper's `clientWidth` / `clientHeight` so
  // `fitView` can pick a zoom that lands the whole sprite in view.
  let stageW = $state(0);
  let stageH = $state(0);
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
  // M12.6: frame-time probe. Off by default so normal use pays zero
  // cost (a single boolean test in `tick()`). Toggled with F2. When on,
  // `tick()` times each recompose into a rolling window and tracks the
  // effective frame rate from inter-tick spacing; the footer surfaces
  // average / worst compose cost and fps. See STATUS.md M12.6.
  let probeOn = $state(false);
  const PROBE_WINDOW = 60;
  let composeSamples: number[] = [];
  let composeMs = $state(0);
  let composeMaxMs = $state(0);
  let fpsEma = $state(0);
  let lastTickTs = 0;
  // Current on-disk identity of the document. `handle` is non-null
  // only on File System Access API browsers after a successful
  // open / save-as; subsequent saves write through it in place. `name`
  // is the suggested filename for fallback downloads and the new-file
  // case. See ui/src/lib/fs/index.ts and docs/specs/pincel.md §10.2.
  const DEFAULT_FILE_NAME = 'pincel.aseprite';
  let saveTarget = $state<SaveTarget>({
    name: DEFAULT_FILE_NAME,
    handle: null,
    path: null,
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
  // DOM refs for keyboard navigation of the recent-files menu: focus
  // moves into the list on open and returns to the trigger on close.
  let recentsTrigger = $state<HTMLButtonElement | null>(null);
  let recentsMenu = $state<HTMLUListElement | null>(null);
  const tauriHost = isTauri();
  const recentsAvailable = (fsAccessAvailable || tauriHost) && isIdbAvailable();
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
  let recoverySnapshots = $state<AutosaveSnapshotMeta[]>([]);
  let recoveryOpen = $state(false);
  // Per-row failure surface so a failed Recover / Discard keeps the
  // dialog open with the error visible against the offending row,
  // and the user can retry or pick a different snapshot. Keyed by
  // `docId` so independent rows don't share an error slot.
  let recoveryErrors = $state<Record<string, string>>({});
  // First-launch file-association dialog (Tauri-only). Visible once
  // per install; "Don't show again" persists the pref.
  const FILE_ASSOC_PREF = 'fileAssocPromptShown';
  let fileAssocOpen = $state(false);
  const platform: 'macos' | 'windows' | 'linux' | 'unknown' = (() => {
    if (typeof navigator === 'undefined') return 'unknown';
    const ua = navigator.userAgent;
    if (/Mac/i.test(ua)) return 'macos';
    if (/Win/i.test(ua)) return 'windows';
    if (/Linux/i.test(ua)) return 'linux';
    return 'unknown';
  })();
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
    if (!doc || !renderer) return;
    const frame = doc.compose(0, 1);
    try {
      renderer.draw(frame);
    } finally {
      frame.free();
    }
    paintOverlays();
  }

  // Clear the overlay layer and repaint whatever transient furniture is
  // currently live. Runs on the full recompose path (the dirty fast path
  // only fires when nothing here would paint, per `canRecomposeDirty`).
  function paintOverlays() {
    if (!overlay) return;
    const ctx = overlay.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, overlay.width, overlay.height);
    if (dragStart && dragPreview && dragTool) {
      // In-flight drag-shape preview. On release we commit through the
      // matching wasm method and the composed cel surfaces the same
      // pixels naturally; the next paint clears this preview.
      const end = constrainedEndpoint();
      if (dragTool === 'line') {
        paintLinePreview(overlay, dragStart.x, dragStart.y, end.x, end.y, color);
      } else if (dragTool === 'rectangle') {
        paintRectanglePreview(overlay, dragStart.x, dragStart.y, end.x, end.y, color, false);
      } else if (dragTool === 'rectangle-fill') {
        paintRectanglePreview(overlay, dragStart.x, dragStart.y, end.x, end.y, color, true);
      } else if (dragTool === 'ellipse') {
        paintEllipsePreview(overlay, dragStart.x, dragStart.y, end.x, end.y, color, false);
      } else if (dragTool === 'ellipse-fill') {
        paintEllipsePreview(overlay, dragStart.x, dragStart.y, end.x, end.y, color, true);
      } else if (dragTool === 'selection-rect' || dragTool === 'slice') {
        // Inclusive-corner marquee preview: matches the rect that
        // `commitSelection` / `commitSliceDrag` will hand to the wasm
        // side on release. Slice drags reuse the marching marquee so the
        // gesture matches the active-slice overlay being edited.
        const minX = Math.min(dragStart.x, end.x);
        const maxX = Math.max(dragStart.x, end.x);
        const minY = Math.min(dragStart.y, end.y);
        const maxY = Math.max(dragStart.y, end.y);
        paintSelectionMarquee(overlay, minX, minY, maxX - minX + 1, maxY - minY + 1, marchPhase);
      }
    } else if (selection) {
      // No marquee drag in flight. A live Move-tool selection drag paints
      // a ghost marquee at the translated position (the pixels snap into
      // place on release); otherwise paint the committed marquee.
      if (moveSelStart && moveSelPreview) {
        const dx = moveSelPreview.x - moveSelStart.x;
        const dy = moveSelPreview.y - moveSelStart.y;
        paintSelectionMarquee(
          overlay,
          selection.x + dx,
          selection.y + dy,
          selection.w,
          selection.h,
          marchPhase,
        );
      } else {
        paintSelectionMarquee(overlay, selection.x, selection.y, selection.w, selection.h, marchPhase);
      }
    }
    // Tilemap Stamp tool: tile grid + hovered cell, drawn after the
    // marquee so a marquee + stamp combo keeps both visible.
    if (tool === 'tilemap-stamp' && stampTile && doc) {
      paintTileGridOverlay();
    }
    // Active slice overlay last so its geometry stays above the grid /
    // marquee when several overlays would otherwise compete.
    if (doc && activeSliceId !== null) {
      paintActiveSliceOverlay();
    }
  }

  // Wipe the overlay layer. Used by the dirty fast path so a preview /
  // marquee painted on the previous full frame can't linger once the
  // overlay sources go inactive (the base sub-rect blit leaves the
  // overlay untouched).
  function clearOverlay() {
    if (!overlay) return;
    const ctx = overlay.getContext('2d');
    ctx?.clearRect(0, 0, overlay.width, overlay.height);
  }

  // Sub-rect blit path (M12.4). Only safe when none of the overlay
  // sources are live, since the overlay paints would otherwise stale
  // out on the parts of the canvas this call leaves untouched. The
  // caller (`tick`) checks `canRecomposeDirty()` before routing here.
  function recomposeDirty(rect: { x: number; y: number; w: number; h: number }) {
    if (!doc || !renderer) return;
    const frame = doc.composeDirty(0, 1, rect.x, rect.y, rect.w, rect.h);
    try {
      renderer.drawDirty(frame);
    } finally {
      frame.free();
    }
    clearOverlay();
  }

  // True when no transient overlay is being painted on top of the
  // composited frame — i.e. a sub-rect blit will not leave stale
  // overlay pixels behind. Selection marquee, drag shape preview,
  // tilemap stamp grid, and active-slice marquee all force the full
  // recompose path.
  function canRecomposeDirty(): boolean {
    if (selection) return false;
    if (dragStart || moveSelStart) return false;
    if (tool === 'tilemap-stamp' && stampTile) return false;
    if (activeSliceId !== null) return false;
    return true;
  }

  // Paint marching ants on the active slice's frame-0 bounds, plus
  // its optional 9-patch center band and pivot crosshair. The wasm
  // getters that back this run only when `activeSliceId` is set; if
  // the slice was just removed via undo, the function silently
  // no-ops via the try/catch wrapping the bounds read.
  function paintActiveSliceOverlay() {
    if (!overlay || !doc || activeSliceId === null) return;
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
      paintSelectionMarquee(overlay, x, y, w, h, marchPhase);
      if (doc.sliceKeyHasCenter(id, keyIndex)) {
        const cx = doc.sliceKeyCenterX(id, keyIndex);
        const cy = doc.sliceKeyCenterY(id, keyIndex);
        const cw = doc.sliceKeyCenterWidth(id, keyIndex);
        const ch = doc.sliceKeyCenterHeight(id, keyIndex);
        // Reuse the slice's editor color for the center accent so
        // 9-patch slices read as distinct from the active marquee.
        const color = sliceColorCss(doc.sliceColor(id));
        paintRectOutline(overlay!, cx, cy, cw, ch, color);
      }
      if (doc.sliceKeyHasPivot(id, keyIndex)) {
        const px = doc.sliceKeyPivotX(id, keyIndex);
        const py = doc.sliceKeyPivotY(id, keyIndex);
        paintPivotCrosshair(overlay!, px, py);
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
    if (!overlay || !doc || !stampTile) return;
    const tileW = doc.tilesetTileWidth(stampTile.tilesetId);
    const tileH = doc.tilesetTileHeight(stampTile.tilesetId);
    if (tileW === 0 || tileH === 0) return;
    const ctx = overlay.getContext('2d');
    if (!ctx) return;
    ctx.save();
    ctx.strokeStyle = 'rgba(96, 165, 250, 0.5)';
    ctx.lineWidth = 1;
    for (let gx = tileW; gx < overlay.width; gx += tileW) {
      ctx.beginPath();
      ctx.moveTo(gx + 0.5, 0);
      ctx.lineTo(gx + 0.5, overlay.height);
      ctx.stroke();
    }
    for (let gy = tileH; gy < overlay.height; gy += tileH) {
      ctx.beginPath();
      ctx.moveTo(0, gy + 0.5);
      ctx.lineTo(overlay.width, gy + 0.5);
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

  // Fit the whole sprite to the viewport: the largest integer zoom that
  // shows it with a small margin, then re-center (pan 0). Runs on every
  // document replacement (new / open / recover) so a freshly-loaded
  // sprite always lands fully in view regardless of its dimensions, and
  // backs the "Reset" control + the `0` shortcut. Falls back to the
  // historical 8× default when the stage hasn't been measured yet
  // (e.g. first paint before layout settles).
  function fitView() {
    zoom =
      stageW > 0 && stageH > 0
        ? fitZoom(stageW, stageH, canvasW, canvasH, MIN_ZOOM, MAX_ZOOM, FIT_MARGIN)
        : 8;
    panX = 0;
    panY = 0;
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
    if (!canvas) return;
    // Ignore purely horizontal gestures (deltaY === 0): they aren't a
    // zoom intent, and swallowing them would suppress horizontal
    // trackpad scroll over the canvas.
    if (e.deltaY === 0) return;
    // Stop the gesture from scrolling the page / parent containers.
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    const oldZoom = zoom;
    let next =
      e.deltaY < 0 ? Math.round(oldZoom * 1.25) : Math.round(oldZoom / 1.25);
    if (next === oldZoom) next = oldZoom + (e.deltaY < 0 ? 1 : -1);
    next = Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, next));
    if (next === oldZoom) return;
    const ratio = next / oldZoom;
    const rectCx = rect.left + rect.width / 2;
    const rectCy = rect.top + rect.height / 2;
    panX += (rectCx - e.clientX) * (ratio - 1);
    panY += (rectCy - e.clientY) * (ratio - 1);
    zoom = next;
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
    fitView();
    syncSelection();
    tilesetRev += 1;
    stampTile = null;
    stampHover = null;
    editingTile = null;
    activeSliceId = null;
    saveTarget = { name: DEFAULT_FILE_NAME, handle: null, path: null };
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
      fitView();
      syncSelection();
      tilesetRev += 1;
      stampTile = null;
      stampHover = null;
      editingTile = null;
      activeSliceId = null;
      saveTarget = {
        name: opened.name,
        handle: opened.handle,
        path: opened.path,
      };
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
  // rather than a copy. The snapshot row is dropped only on success;
  // on failure the dialog stays open with a per-row error so the
  // user can retry or pick a different snapshot.
  async function applyRecovery(meta: AutosaveSnapshotMeta) {
    let next: Document;
    try {
      const full = await latestSnapshot(meta.docId);
      if (!full) throw new Error('snapshot bytes not found');
      next = Document.openAseprite(full.bytes);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      recoveryErrors = { ...recoveryErrors, [meta.docId]: msg };
      status = `recover failed: ${msg}`;
      return;
    }
    disposeDoc();
    doc = next;
    dirty = true;
    syncMeta();
    fitView();
    syncSelection();
    tilesetRev += 1;
    stampTile = null;
    stampHover = null;
    editingTile = null;
    activeSliceId = null;
    saveTarget = { name: meta.name, handle: null, path: null };
    docId = meta.docId;
    lastWriteUndoDepth = doc.undoDepth;
    status = `recovered ${meta.name} · ${doc.width}×${doc.height}`;
    await clearAutosave();
    closeRecovery();
  }

  async function discardSnapshot(targetDocId: string) {
    try {
      await removeSnapshots(targetDocId);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      console.error('discard snapshot failed', err);
      recoveryErrors = { ...recoveryErrors, [targetDocId]: msg };
      status = `discard failed: ${msg}`;
      return;
    }
    recoverySnapshots = recoverySnapshots.filter(
      (s) => s.docId !== targetDocId,
    );
    if (targetDocId in recoveryErrors) {
      const next = { ...recoveryErrors };
      delete next[targetDocId];
      recoveryErrors = next;
    }
    if (recoverySnapshots.length === 0) closeRecovery();
  }

  function closeRecovery() {
    recoveryOpen = false;
    recoverySnapshots = [];
    recoveryErrors = {};
  }

  // Persist the current `saveTarget` to the recent-files registry and
  // refresh the in-memory list. No-op when the registry isn't
  // available (non-FSA browsers / IDB disabled) or the current target
  // doesn't carry a handle (a download-only save, or a New document
  // that hasn't been written yet).
  async function recordRecent() {
    if (!recentsAvailable) return;
    // A recent needs something a future re-open can use: an FSA handle
    // (web) or a path (Tauri).
    if (!saveTarget.handle && !saveTarget.path) return;
    try {
      await upsertRecent({
        id: docId,
        name: saveTarget.name,
        handle: saveTarget.handle,
        path: saveTarget.path,
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
          status = `recent ${r.name}: permission denied`;
          return;
        }
        const file = await r.handle.getFile();
        bytes = new Uint8Array(await file.arrayBuffer());
        name = file.name;
        nextTarget = { name: file.name, handle: r.handle, path: null };
      } else {
        status = `recent ${r.name}: no handle / path`;
        return;
      }
      const next = Document.openAseprite(bytes);
      disposeDoc();
      doc = next;
      dirty = true;
      syncMeta();
      fitView();
      syncSelection();
      tilesetRev += 1;
      stampTile = null;
      stampHover = null;
      editingTile = null;
      activeSliceId = null;
      saveTarget = nextTarget;
      // Preserve the recent's id so re-opens count as the same doc
      // and autosave snapshots survive across page reloads.
      docId = r.id;
      lastWriteUndoDepth = doc.undoDepth;
      status = `opened ${name} · ${doc.width}×${doc.height}`;
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

  // Fold one compose-cost sample into the rolling window and refresh
  // the derived average / max. Only called when the probe is active.
  function recordComposeSample(ms: number) {
    composeSamples.push(ms);
    if (composeSamples.length > PROBE_WINDOW) composeSamples.shift();
    let sum = 0;
    let max = 0;
    for (const s of composeSamples) {
      sum += s;
      if (s > max) max = s;
    }
    composeMs = sum / composeSamples.length;
    composeMaxMs = max;
  }

  function tick() {
    if (probeOn) {
      const now = performance.now();
      if (lastTickTs > 0) {
        const fps = 1000 / Math.max(now - lastTickTs, 0.0001);
        // Exponential moving average smooths the per-frame jitter so
        // the footer reading is stable enough to eyeball under load.
        fpsEma = fpsEma === 0 ? fps : fpsEma * 0.9 + fps * 0.1;
      }
      lastTickTs = now;
    }
    if (doc) {
      const events = doc.drainEvents();
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
      if (dirtyKind !== 'none') dirty = true;
      if (selectionTouched) {
        syncSelection();
        // A bare `selection-changed` event (e.g. `setSelection` /
        // `clearSelection` from a tool) needs a repaint to show or
        // hide the marquee. Force the full path so the overlay paint
        // in `recompose()` runs — the sub-rect path skips overlays.
        dirty = true;
        if (dirtyKind === 'none') dirtyKind = 'canvas';
      }
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
          // The march tick repaints overlays, not pixels, but the
          // sub-rect path can't redraw them — force full recompose.
          dirtyKind = 'canvas';
        }
      } else {
        marchTicks = 0;
      }
      if (dirty) {
        dirty = false;
        const t0 = probeOn ? performance.now() : 0;
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
        if (probeOn) recordComposeSample(performance.now() - t0);
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

  // Single-key tool shortcuts, aligned with Aseprite defaults where they
  // don't collide. Modifier-bearing presses (Ctrl/Cmd/Alt) are left to
  // the browser / OS; Shift is tolerated (normalized via toLowerCase).
  const TOOL_KEYS: Record<string, Tool> = {
    b: 'pencil',
    e: 'eraser',
    i: 'eyedropper',
    g: 'bucket',
    l: 'line',
    u: 'rectangle',
    m: 'selection-rect',
    v: 'move',
  };

  function onKeyDown(e: KeyboardEvent) {
    if (e.code === 'Space' && !e.repeat && !isEditableTarget(e.target)) {
      // Prevent the browser from page-scrolling on space.
      e.preventDefault();
      spaceDown = true;
      return;
    }
    if (e.key === 'Escape' && recentMenuOpen) {
      closeRecentMenu();
      return;
    }
    // F2 toggles the frame-time probe (M12.6). Reset the window on
    // enable so a reading reflects only post-toggle frames.
    if (e.key === 'F2' && !isEditableTarget(e.target)) {
      e.preventDefault();
      probeOn = !probeOn;
      composeSamples = [];
      composeMs = 0;
      composeMaxMs = 0;
      fpsEma = 0;
      lastTickTs = 0;
      return;
    }
    // Single-key tool selection. Skip when a modifier is held (browser
    // shortcuts) or focus is in an editable field.
    if (
      doc &&
      !e.ctrlKey &&
      !e.metaKey &&
      !e.altKey &&
      !isEditableTarget(e.target)
    ) {
      // View zoom shortcuts. Bare keys only — Ctrl/Cmd +/- stays the
      // browser's page zoom (the modifier guard above excludes it).
      // `0` fits the sprite to the viewport (same as the Reset control).
      if (e.key === '+' || e.key === '=') {
        e.preventDefault();
        zoomIn();
        return;
      }
      if (e.key === '-' || e.key === '_') {
        e.preventDefault();
        zoomOut();
        return;
      }
      if (e.key === '0') {
        e.preventDefault();
        resetView();
        return;
      }
      const next = TOOL_KEYS[e.key.toLowerCase()];
      if (next) {
        e.preventDefault();
        tool = next;
      }
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
      .then(async () => {
        if (cancelled) return;
        // Pick the base-layer renderer before the first frame. The base
        // canvas is bound by the time onMount runs (spec §4.4, M12.5).
        if (canvas && !renderer) {
          const r = await createRenderer(canvas);
          if (cancelled) {
            r.destroy();
            return;
          }
          renderer = r;
          backend = r.backend;
        }
        doc = new Document(64, 64);
        syncMeta();
        fitView();
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
    // Wheel zoom is registered imperatively (not via `onwheel`) so the
    // listener is non-passive and `preventDefault()` actually suppresses
    // page scroll. `canvas` is bound by the time onMount runs.
    canvas?.addEventListener('wheel', onWheel, { passive: false });
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
    // Native menu wiring (Tauri only). The Rust side emits a "menu"
    // event with the item id as payload; we dispatch into local
    // handlers. The unlisten fn is stored so cleanup tears it down
    // before the window unloads. Best-effort: a wire failure logs but
    // doesn't block the app — the toolbar buttons stay available.
    let unlistenMenu: UnlistenFn | null = null;
    let unlistenOpenFile: UnlistenFn | null = null;
    if (tauriHost) {
      wireNativeMenu({
        'menu:new': newDoc,
        'menu:open': openDoc,
        'menu:save': () => save(),
        'menu:saveAs': () => save({ forceAs: true }),
        'menu:undo': undo,
        'menu:redo': redo,
        'menu:zoomIn': zoomIn,
        'menu:zoomOut': zoomOut,
        'menu:resetZoom': resetView,
        recent: openRecentById,
      })
        .then((fn) => {
          if (cancelled) {
            fn();
            return;
          }
          unlistenMenu = fn;
        })
        .catch((err: unknown) => {
          console.error('wireNativeMenu failed', err);
        });
      // Open-file events from Rust: file-association double-click,
      // CLI arg, macOS RunEvent::Opened, single-instance forward.
      listen<string>('open-file', (e) => {
        if (typeof e.payload === 'string') void openByPath(e.payload);
      })
        .then((fn) => {
          if (cancelled) {
            fn();
            return;
          }
          unlistenOpenFile = fn;
        })
        .catch((err: unknown) => {
          console.error('open-file listen failed', err);
        });
      // First-launch file-association advisory. Best-effort: a missing
      // IDB or a getPref failure silently skips the dialog.
      if (autosaveAvailable) {
        getPref(FILE_ASSOC_PREF)
          .then((shown) => {
            if (!cancelled && !shown) fileAssocOpen = true;
          })
          .catch((err: unknown) => {
            console.error('getPref fileAssoc failed', err);
          });
      }
    }
    return () => {
      cancelled = true;
      if (rafHandle !== null) cancelAnimationFrame(rafHandle);
      if (autosaveTimer !== null) {
        clearInterval(autosaveTimer);
        autosaveTimer = null;
      }
      if (unlistenMenu) unlistenMenu();
      if (unlistenOpenFile) unlistenOpenFile();
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onWindowBlur);
      document.removeEventListener('visibilitychange', onVisibilityChange);
      canvas?.removeEventListener('wheel', onWheel);
      renderer?.destroy();
      renderer = null;
      disposeDoc();
    };
  });

  // Look a recent up by id and route through `openRecent`.
  // Native-menu Recent items only know the id; the full RecentFile
  // lives in the local `recents` state.
  function openRecentById(id: string) {
    const r = recents.find((row) => row.id === id);
    if (r) void openRecent(r);
  }

  // Tauri-only: open a sprite by absolute path. Used by the
  // `open-file` event (file-association double-click, CLI arg) and
  // by `openRecent` when the recent carries a path.
  async function openByPath(path: string) {
    try {
      const raw = await invoke<number[] | ArrayBuffer>('read_file_bytes', {
        path,
      });
      const bytes =
        raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
      const next = Document.openAseprite(bytes);
      disposeDoc();
      doc = next;
      dirty = true;
      syncMeta();
      fitView();
      syncSelection();
      tilesetRev += 1;
      stampTile = null;
      stampHover = null;
      editingTile = null;
      activeSliceId = null;
      const name = path.replace(/^.*[/\\]/, '');
      saveTarget = { name, handle: null, path };
      docId = crypto.randomUUID();
      lastWriteUndoDepth = doc.undoDepth;
      status = `opened ${name} · ${doc.width}×${doc.height}`;
      await clearAutosave();
      await recordRecent();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      status = `open-file failed: ${msg}`;
    }
  }

  // Open/close the recent-files dropdown. Opening moves focus onto the
  // first entry (next frame, once the list is in the DOM); closing
  // returns focus to the trigger so keyboard users keep their place.
  function toggleRecentMenu() {
    recentMenuOpen = !recentMenuOpen;
    if (recentMenuOpen) {
      requestAnimationFrame(() => {
        recentsMenu?.querySelector('button')?.focus();
      });
    }
  }

  function closeRecentMenu() {
    recentMenuOpen = false;
    recentsTrigger?.focus();
  }

  // Arrow / Home / End navigation across the recent-files entries.
  function onRecentsKeydown(e: KeyboardEvent) {
    if (!recentsMenu) return;
    const items = Array.from(
      recentsMenu.querySelectorAll<HTMLButtonElement>('button'),
    );
    if (items.length === 0) return;
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      items[current < 0 ? 0 : (current + 1) % items.length]?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      items[
        current < 0 ? items.length - 1 : (current - 1 + items.length) % items.length
      ]?.focus();
    } else if (e.key === 'Home') {
      e.preventDefault();
      items[0]?.focus();
    } else if (e.key === 'End') {
      e.preventDefault();
      items[items.length - 1]?.focus();
    }
  }

  function dismissFileAssoc(dontShowAgain: boolean) {
    fileAssocOpen = false;
    if (dontShowAgain) {
      setPref(FILE_ASSOC_PREF, true).catch((err: unknown) => {
        console.error('setPref fileAssoc failed', err);
      });
    }
  }

  // Push the current recents list to the Rust-side `Open Recent`
  // submenu so the native menu stays in sync with the in-memory list.
  // Only entries with a path can be re-opened by the menu (FSA handles
  // need an in-page permission gesture). Failures are logged but don't
  // surface in the UI — the toolbar dropdown remains the authoritative
  // recents UI on the web.
  $effect(() => {
    if (!tauriHost) return;
    const items = recents
      .filter((r) => r.path !== null)
      .map((r) => ({ id: r.id, name: r.name }));
    syncRecentMenu(items).catch((err: unknown) => {
      console.error('syncRecentMenu failed', err);
    });
  });
</script>

<main class="flex h-full flex-col bg-neutral-950 text-neutral-100">
  <header class="flex flex-wrap items-center gap-2 border-b border-neutral-800 px-4 py-2 text-sm">
    <span class="mr-2 font-semibold tracking-wide">Pincel</span>
    <button class="toolbar-btn" onclick={newDoc}>New</button>
    <button class="toolbar-btn" onclick={openDoc}>Open…</button>
    <button class="toolbar-btn" onclick={() => save()} disabled={!doc}>
      {fsAccessAvailable || tauriHost ? 'Save' : 'Save As (download)'}
    </button>
    {#if fsAccessAvailable || tauriHost}
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
          bind:this={recentsTrigger}
          class="toolbar-btn"
          class:toolbar-btn-active={recentMenuOpen}
          aria-haspopup="menu"
          aria-expanded={recentMenuOpen}
          disabled={recents.length === 0}
          onclick={toggleRecentMenu}
        >
          Recent…
        </button>
        {#if recentMenuOpen}
          <ul
            bind:this={recentsMenu}
            class="absolute left-0 top-full z-10 mt-1 flex min-w-48 flex-col rounded border border-neutral-700 bg-neutral-900 py-1 shadow-lg"
            role="menu"
            aria-label="Recent files"
            onkeydown={onRecentsKeydown}
          >
            {#each recents as r (r.id)}
              <li>
                <button
                  class="w-full truncate px-3 py-1 text-left text-xs hover:bg-neutral-800"
                  role="menuitem"
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
        title="Pencil (B)"
        onclick={() => (tool = 'pencil')}
      >
        Pencil
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'eraser'}
        aria-pressed={tool === 'eraser'}
        title="Eraser (E)"
        onclick={() => (tool = 'eraser')}
      >
        Eraser
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'eyedropper'}
        aria-pressed={tool === 'eyedropper'}
        title="Eyedropper (I)"
        onclick={() => (tool = 'eyedropper')}
      >
        Eyedropper
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'bucket'}
        aria-pressed={tool === 'bucket'}
        title="Bucket (G)"
        onclick={() => (tool = 'bucket')}
      >
        Bucket
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'line'}
        aria-pressed={tool === 'line'}
        title="Line (L)"
        onclick={() => (tool = 'line')}
      >
        Line
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'rectangle'}
        aria-pressed={tool === 'rectangle'}
        title="Rectangle (U)"
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
        title="Selection (M)"
        onclick={() => (tool = 'selection-rect')}
      >
        Select
      </button>
      <button
        class="toolbar-btn"
        class:toolbar-btn-active={tool === 'move'}
        aria-pressed={tool === 'move'}
        title="Move (V)"
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
    <div
      class="relative flex flex-1 items-center justify-center overflow-hidden"
      bind:clientWidth={stageW}
      bind:clientHeight={stageH}
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
        class="relative shrink-0 border border-neutral-700 bg-neutral-900 shadow-lg"
        style:width="{canvasW * zoom}px"
        style:height="{canvasH * zoom}px"
        style:transform="translate({panX}px, {panY}px)"
      >
        <canvas
          bind:this={canvas}
          class="canvas-pixelated absolute inset-0 h-full w-full touch-none"
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
        <canvas
          bind:this={overlay}
          class="canvas-pixelated pointer-events-none absolute inset-0 h-full w-full"
          width={canvasW}
          height={canvasH}
          aria-hidden="true"
        ></canvas>
      </div>
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
    {#if backend !== 'none'}
      <span>·</span>
      <span>{backend}</span>
    {/if}
    {#if probeOn}
      <span>·</span>
      <span class="text-emerald-400">
        {fpsEma.toFixed(0)} fps · compose {composeMs.toFixed(2)}ms (max
        {composeMaxMs.toFixed(2)}ms)
      </span>
    {/if}
  </footer>
</main>

{#if recoveryOpen && recoverySnapshots.length > 0}
  <RecoveryDialog
    snapshots={recoverySnapshots}
    errors={recoveryErrors}
    onRecover={(snap) => {
      void applyRecovery(snap);
    }}
    onDiscard={(targetDocId) => {
      void discardSnapshot(targetDocId);
    }}
    onDismiss={closeRecovery}
  />
{/if}

{#if fileAssocOpen}
  <FileAssocDialog {platform} onDismiss={dismissFileAssoc} />
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
