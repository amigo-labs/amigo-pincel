// Pixel-mode editor state (Svelte 5 runes), one object per concern —
// the same pattern as `modes/image/stores/editor.svelte.ts`. The shell
// mounts one editor per session, so these are module singletons;
// `resetPixelStore()` runs when `PixelEditor` mounts.
//
// Several documents are open as tabs (docs/specs/document-tabs.md): the
// per-document keys of `ed`, `view`, `play` and `file` belong to the
// active tab (`lib/shell/tab-store`), the rest is shared by all tabs.
//
// The wasm `Document` stays the source of truth for sprite state
// (CLAUDE.md §9); the fields here mirror what the UI renders from it.

import type { Document } from '../../../lib/core';
import { tabStore } from '../../../lib/shell/tab-store';
import type { SaveTarget } from '../../../lib/fs';
import { EMPTY_HISTORY, type HistoryOrigin, type HistorySnapshot } from '../../../lib/history/names';
import type { RecentFile } from '../../../lib/idb/recent-files';
import type { EffectSession } from '../../../lib/effects/session.svelte';
import { t } from '../../../lib/i18n/index.svelte';
import type { CanvasRenderer, RenderBackend } from '../../../lib/render/types';
import { PinchTracker } from '../../../lib/view/pinch';
import type { ResizeMode } from '../components/ResizeDialog.svelte';
import type { OnionSettings } from '../playback';
import type { RotateDialogState } from '../rotate-ops';
import type { BrushShape } from '../brush';
import type { Direction, Tool } from '../tools';
import type { PasteDialogState } from '../clipboard-ops';

/** Shape paint mode: stroke only, fill only, or both. */
export type ShapeMode = 'outline' | 'fill' | 'fill_outline';
/** Marquee combination mode (Shift = add, Alt = subtract, both = intersect). */
export type SelectionMode = 'replace' | 'add' | 'subtract' | 'intersect';
/** Sprite-space point. */
export type Pt = { x: number; y: number };
/** Sprite-space rectangle. */
export type Box = { x: number; y: number; w: number; h: number };
/** Symmetry / tiled-mode axes: off, x (left–right), y (top–bottom) or both. */
export type AxisMode = 'none' | 'x' | 'y' | 'xy';
/** A (tileset, tile) pair. */
export type TileRef = { tilesetId: number; tileId: number };

/** Largest sprite edge (New, Resize, Paste ▸ Expand canvas). */
export const MAX_DOC_SIZE = 4096;

/** Default suggested filename for a new document. */
export const DEFAULT_FILE_NAME = 'pincel.aseprite';

/** A tab's document and what the UI mirrors from it. */
function docInit() {
  return {
    doc: null as Document | null,
    undoDepth: 0,
    redoDepth: 0,
    /** The undo history the History panel lists (history-panel AC-02). */
    history: EMPTY_HISTORY as HistorySnapshot,
    /** `historyRevision` the panel's list was read at; `-1` before. */
    historyRev: -1,
    /** How the document began: names the start entry (history-panel AC-03). */
    origin: 'start' as HistoryOrigin,
    /** Undo depth at the last save / load; `-1` marks a recovered
     *  document as unsaved. */
    lastSavedUndoDepth: 0,
    canvasW: 64,
    canvasH: 64,
    currentFrame: 0,
    frameCount: 1,
    /** Mirror of the wasm selection's bounds; `null` without one. */
    selection: null as Box | null,
    /** Row highlighted in the Layers panel. */
    activeLayerId: null as number | null,
    /** Slice focused in the Slices panel. */
    activeSliceId: null as number | null,
  };
}

function edInit() {
  return {
    /** The mount's document is loaded, fitted and composed once — the
     *  editor takes input. Published as `data-ready` on the canvas. */
    ready: false,
    /** Bumped whenever the wasm side may have changed what panels derive
     *  from; the wasm getters are opaque to Svelte's reactive graph. */
    docRev: 0,
    /** Footer status line. */
    status: t('px.initializing'),
    /** Active base-layer renderer, surfaced in the footer. */
    backend: 'none' as RenderBackend | 'none',
  };
}

/** A tab's viewport transform. */
function tabViewInit() {
  return {
    /** Integer display zoom (sprite px → CSS px). */
    zoom: 8,
    /** Pan offset (CSS px) from the centred layout box. */
    panX: 0,
    panY: 0,
  };
}

function viewInit() {
  return {
    /** Live size of the canvas stage, for `fitView`. */
    stageW: 0,
    stageH: 0,
    /** The canvas stage itself: `fitView` measures it when the bound size
     *  has not caught up with a layout change yet. */
    stage: null as HTMLElement | null,
    panning: false,
    /** Space held: temporary pan-on-drag. */
    spaceDown: false,
    /** Marching-ants phase (mod 4). */
    marchPhase: 0,
    canvas: null as HTMLCanvasElement | null,
    /** Canvas2D overlay stacked over the base canvas. */
    overlay: null as HTMLCanvasElement | null,
    /** Stage-sized canvas the pixel grid and custom grid draw on. */
    gridCanvas: null as HTMLCanvasElement | null,
    /** View ▸ Show Pixel Grid: on by default (pixel-grid AC-04). */
    showPixelGrid: true,
    /** View ▸ Show Grid: off by default (pixel-grid AC-16). */
    showGrid: false,
    /** Custom-grid line colour `#rrggbb` and opacity (%). */
    gridColor: '#0000ff',
    gridOpacity: 60,
    /** View ▸ Tiled Mode: session-only, never stored in the document
     *  (symmetry-tiled-mode AC-12, AC-18). */
    tiled: 'none' as AxisMode,
  };
}

function toolInit() {
  return {
    tool: 'pencil' as Tool,
    /** Pencil / Eraser brush, shared by both tools and persisted. */
    brushSize: 1,
    brushShape: 'square' as BrushShape,
    /** Off by default (pixel-pencil AC-15); only acts at size 1. */
    pixelPerfect: false,
    /** Pen pressure scales the brush size; on by default like Image mode
     *  (pixel-pressure AC-08). Shared by Pencil and Eraser, not persisted. */
    pressureSize: true,
    /** Pencil ink: Simple paints, Shading steps along the palette ramp
     *  (shading-ink AC-09: a fresh session starts Simple, not persisted). */
    pencilInk: 'simple' as 'simple' | 'shading',
    /** Pencil / Eraser symmetry, session-only (symmetry-tiled-mode). */
    symmetry: 'none' as AxisMode,
    /** Moved axis positions in half pixels from the left / top canvas
     *  edge; `null` = unmoved, at the canvas centre (AC-04, AC-20). */
    symmetryX: null as number | null,
    symmetryY: null as number | null,
    strokeWidth: 1,
    shapeMode: 'outline' as ShapeMode,
    cornerRadius: 4,
    polygonSides: 6,
    wandTolerance: 0,
    wandContiguous: true,
    wandSample: 'layer' as 'layer' | 'composite',
    /** Gradient ramp and dither matrix size (0 = hard split); linear and
     *  Bayer 4×4 by default (dithered-gradient, Assumptions). */
    gradientKind: 'linear' as 'linear' | 'radial',
    gradientDither: 4 as 0 | 2 | 4 | 8,
    /** Tile the Tilemap Stamp tool places. */
    stampTile: null as TileRef | null,
    /** Hover cell under the cursor while stamping. */
    stampHover: null as Pt | null,
    /** Polygonal-lasso vertices placed so far. */
    polyPoints: null as Pt[] | null,
    /** In-flight Move-tool content drag (press / live point). */
    moveSelStart: null as Pt | null,
    moveSelPreview: null as Pt | null,
    /** Where the Text dialog was anchored. */
    textAnchor: null as Pt | null,
  };
}

function colorsInit() {
  return {
    color: '#f87171',
    alpha: 255,
    bgColor: '#ffffff',
    bgAlpha: 255,
    /** Shading-ink ramp: the inclusive palette index range picked with
     *  Shift+click, `null` for the whole palette. Transient UI state, never
     *  written to the document (shading-ink AC-07, AC-12). */
    rampRange: null as { from: number; to: number } | null,
  };
}

/** A tab's playback range and onion skin. */
function tabPlayInit() {
  return {
    /** Tag index playback loops over (`null` = every frame). */
    loopTag: null as number | null,
    onion: { enabled: false, back: 1, forward: 1, alpha: 76 } as OnionSettings,
  };
}

function playInit() {
  return { playing: false };
}

/** Preview panel zoom: an integer factor or the largest fit. */
export type PreviewZoom = 1 | 2 | 4 | 'fit';

function previewInit() {
  return {
    /** The preview's own playback (docs/specs/animation-preview.md). */
    playing: false,
    /** Frame the preview shows while it plays. */
    frame: 0,
    zoom: 1 as PreviewZoom,
    /** Bumped whenever the main canvas recomposes, so the preview does too. */
    contentRev: 0,
  };
}

/** A tab's on-disk identity. */
function tabFileInit() {
  return {
    saveTarget: { name: DEFAULT_FILE_NAME, handle: null, path: null } as SaveTarget,
  };
}

function fileInit() {
  return {
    /** An async open / recover / export is in flight. */
    busy: false,
    recents: [] as RecentFile[],
    /** Scale both export dialogs preselect: the last one used (scaled-export). */
    exportScale: 1,
  };
}

function dialogsInit() {
  return {
    effect: null as EffectSession | null,
    resize: null as ResizeMode | null,
    sheet: false,
    /** File ▸ Export ▸ PNG (current frame) is open (scaled-export). */
    png: false,
    /** View ▸ Grid Settings… is open (pixel-grid). */
    grid: false,
    /** File ▸ Export ▸ Animation… is open. */
    animation: false,
    /** File ▸ Export ▸ SVG… is open (vectorize-dialog). */
    svg: false,
    renaming: null as { id: number; name: string } | null,
    /** Select ▸ Expand… / Contract… awaiting a radius. */
    selectionRadius: null as 'expand' | 'contract' | null,
    /** Tileset Editor sub-mode target. */
    editingTile: null as TileRef | null,
    /** Edit ▸ Paste of an image larger than the canvas (spec §5.4). */
    paste: null as PasteDialogState | null,
    /** Layer ▸ Rotate… is open (rotsprite). */
    rotate: null as RotateDialogState | null,
  };
}

function probeInit() {
  return { on: false, composeMs: 0, composeMaxMs: 0, fpsEma: 0 };
}

/** Non-reactive per-session state: render loop, gesture bookkeeping. */
function liveInit() {
  return {
    renderer: null as CanvasRenderer | null,
    /** The frame needs a full recompose on the next tick. */
    needsRecompose: false,
    rafHandle: null as number | null,
    playHandle: null as number | null,
    previewHandle: null as number | null,
    composeSamples: [] as number[],
    lastTickTs: 0,
    marchTicks: 0,
    colorPrefLoaded: false,
    brushPrefLoaded: false,
    gridPrefLoaded: false,
    exportScalePrefLoaded: false,
    /** What the grid canvas was last painted from (`grid.ts`). */
    gridKey: '',
    /** The gesture in flight started on the right button. */
    useBg: false,
    painting: false,
    /** Last point a pencil / eraser drag sent to the core. */
    lastPaint: null as Pt | null,
    /** Shift-constraint anchor of the drag; `null` while Shift is up. */
    shiftAnchor: null as Pt | null,
    /** 45° step the Shift-constrained drag locked onto; `null` until it moves. */
    shiftDir: null as Direction | null,
    dragStart: null as Pt | null,
    dragPreview: null as Pt | null,
    dragTool: null as Tool | null,
    dragSelMode: 'replace' as SelectionMode,
    dragShift: false,
    lassoPoints: null as Pt[] | null,
    polyHover: null as Pt | null,
    /** Coverage bytes of a shaped selection; `null` for a rect / none. */
    selectionMaskBytes: null as Uint8Array | null,
    panStartClient: null as Pt | null,
    panStartOffset: null as Pt | null,
    pinch: new PinchTracker(),
    pinchZoom0: 1,
    gestureUndoDepth: 0,
    /** Symmetry axis handle being dragged (symmetry-tiled-mode AC-09). */
    axisDrag: null as 'x' | 'y' | null,
    /** Last locked-layer status line. */
    lockNotice: null as string | null,
    textFontId: null as number | null,
  };
}

/** One tab's document state. */
export type TabDoc = ReturnType<typeof docInit>;

/** One open document (docs/specs/document-tabs.md): its wasm document and
 *  mirrors, view, playback range, onion skin and file. `id` is unique for
 *  the page load. */
export interface PixelTab {
  id: number;
  doc: TabDoc;
  view: ReturnType<typeof tabViewInit>;
  play: ReturnType<typeof tabPlayInit>;
  file: ReturnType<typeof tabFileInit>;
}

/** A fresh tab's state around `doc`, saved to `saveTarget`. */
export function tabInit(id: number, doc: Document, saveTarget: SaveTarget): PixelTab {
  return {
    id,
    doc: { ...docInit(), doc },
    view: tabViewInit(),
    play: tabPlayInit(),
    file: { saveTarget },
  };
}

/** The open tabs in opening order, and the active one. */
export const tabs = $state({
  list: [] as PixelTab[],
  active: null as PixelTab | null,
});

// Stand-ins read while no tab is open (before the first document loads).
const idle = $state({
  doc: docInit(),
  view: tabViewInit(),
  play: tabPlayInit(),
  file: tabFileInit(),
});

function keysOf<T extends object>(init: () => T): (keyof T & string)[] {
  return Object.keys(init()) as (keyof T & string)[];
}

// The editor-wide halves of the split stores.
const sharedEd = $state(edInit());
const sharedView = $state(viewInit());
const sharedPlay = $state(playInit());
const sharedFile = $state(fileInit());

/** The active tab's document and what the UI mirrors from it, plus the
 *  editor-wide state. */
export const ed = tabStore(sharedEd, keysOf(docInit), () => tabs.active?.doc ?? idle.doc);
/** The active tab's viewport transform, plus the canvas elements. */
export const view = tabStore(sharedView, keysOf(tabViewInit), () => tabs.active?.view ?? idle.view);
/** Active tool and its options. */
export const toolState = $state(toolInit());
/** Foreground / background colours. */
export const colors = $state(colorsInit());
/** Timeline playback, and the active tab's loop tag and onion skin. */
export const play = tabStore(sharedPlay, keysOf(tabPlayInit), () => tabs.active?.play ?? idle.play);
/** Preview panel playback and zoom, editor-wide; it follows the active tab. */
export const preview = $state(previewInit());
/** The active tab's on-disk identity, plus recents. */
export const file = tabStore(sharedFile, keysOf(tabFileInit), () => tabs.active?.file ?? idle.file);
/** Which dialog is open. */
export const dialogs = $state(dialogsInit());
/** Frame-time probe (F2). */
export const probe = $state(probeInit());
/** Plain, non-reactive state. */
export const live = liveInit();

/** Restore every field to its initial value (editor mount). */
export function resetPixelStore(): void {
  tabs.list = [];
  tabs.active = null;
  Object.assign(idle, { doc: docInit(), view: tabViewInit(), play: tabPlayInit(), file: tabFileInit() });
  Object.assign(ed, edInit());
  Object.assign(view, viewInit());
  Object.assign(toolState, toolInit());
  Object.assign(colors, colorsInit());
  Object.assign(play, playInit());
  Object.assign(preview, previewInit());
  Object.assign(file, fileInit());
  Object.assign(dialogs, dialogsInit());
  Object.assign(probe, probeInit());
  Object.assign(live, liveInit());
}
