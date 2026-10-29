// Pixel-mode editor state (Svelte 5 runes), one object per concern —
// the same pattern as `modes/image/stores/editor.svelte.ts`. The shell
// mounts one editor per session, so these are module singletons;
// `resetPixelStore()` runs when `PixelEditor` mounts.
//
// The wasm `Document` stays the source of truth for sprite state
// (CLAUDE.md §9); the fields here mirror what the UI renders from it.

import type { Document } from '../../../lib/core';
import type { SaveTarget } from '../../../lib/fs';
import type { RecentFile } from '../../../lib/idb/recent-files';
import type { EffectDef } from '../../../lib/effects/catalog';
import { t } from '../../../lib/i18n/index.svelte';
import type { CanvasRenderer, RenderBackend } from '../../../lib/render/types';
import { PinchTracker } from '../../../lib/view/pinch';
import type { ResizeMode } from '../components/ResizeDialog.svelte';
import type { OnionSettings } from '../playback';
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
/** A (tileset, tile) pair. */
export type TileRef = { tilesetId: number; tileId: number };

/** Largest sprite edge (New, Resize, Paste ▸ Expand canvas). */
export const MAX_DOC_SIZE = 4096;

/** Default suggested filename for a new document. */
export const DEFAULT_FILE_NAME = 'pincel.aseprite';

function edInit() {
  return {
    doc: null as Document | null,
    /** The mount's document is loaded, fitted and composed once — the
     *  editor takes input. Published as `data-ready` on the canvas. */
    ready: false,
    /** Bumped whenever the wasm side may have changed what panels derive
     *  from; the wasm getters are opaque to Svelte's reactive graph. */
    docRev: 0,
    undoDepth: 0,
    redoDepth: 0,
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
    /** Footer status line. */
    status: t('px.initializing'),
    /** Active base-layer renderer, surfaced in the footer. */
    backend: 'none' as RenderBackend | 'none',
  };
}

function viewInit() {
  return {
    /** Integer display zoom (sprite px → CSS px). */
    zoom: 8,
    /** Pan offset (CSS px) from the centred layout box. */
    panX: 0,
    panY: 0,
    /** Live size of the canvas stage, for `fitView`. */
    stageW: 0,
    stageH: 0,
    panning: false,
    /** Space held: temporary pan-on-drag. */
    spaceDown: false,
    /** Marching-ants phase (mod 4). */
    marchPhase: 0,
    canvas: null as HTMLCanvasElement | null,
    /** Canvas2D overlay stacked over the base canvas. */
    overlay: null as HTMLCanvasElement | null,
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
    strokeWidth: 1,
    shapeMode: 'outline' as ShapeMode,
    cornerRadius: 4,
    polygonSides: 6,
    wandTolerance: 0,
    wandContiguous: true,
    wandSample: 'layer' as 'layer' | 'composite',
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
  return { color: '#f87171', alpha: 255, bgColor: '#ffffff', bgAlpha: 255 };
}

function playInit() {
  return {
    playing: false,
    /** Tag index playback loops over (`null` = every frame). */
    loopTag: null as number | null,
    onion: { enabled: false, back: 1, forward: 1, alpha: 76 } as OnionSettings,
  };
}

function fileInit() {
  return {
    saveTarget: { name: DEFAULT_FILE_NAME, handle: null, path: null } as SaveTarget,
    /** An async open / save / recover is in flight. */
    busy: false,
    recents: [] as RecentFile[],
  };
}

function dialogsInit() {
  return {
    /** Unsaved-changes confirmation awaiting an answer. */
    confirm: null as { message: string; run: () => void } | null,
    effect: null as EffectDef | null,
    resize: null as ResizeMode | null,
    sheet: false,
    /** File ▸ Export ▸ Animation… is open. */
    animation: false,
    renaming: null as { id: number; name: string } | null,
    /** Select ▸ Expand… / Contract… awaiting a radius. */
    selectionRadius: null as 'expand' | 'contract' | null,
    /** Tileset Editor sub-mode target. */
    editingTile: null as TileRef | null,
    /** Edit ▸ Paste of an image larger than the canvas (spec §5.4). */
    paste: null as PasteDialogState | null,
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
    composeSamples: [] as number[],
    lastTickTs: 0,
    marchTicks: 0,
    colorPrefLoaded: false,
    brushPrefLoaded: false,
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
    /** Last locked-layer status line. */
    lockNotice: null as string | null,
    textFontId: null as number | null,
  };
}

/** The open document and what the UI mirrors from it. */
export const ed = $state(edInit());
/** Viewport transform and canvas elements. */
export const view = $state(viewInit());
/** Active tool and its options. */
export const toolState = $state(toolInit());
/** Foreground / background colours. */
export const colors = $state(colorsInit());
/** Timeline playback and onion skin. */
export const play = $state(playInit());
/** On-disk identity and recents. */
export const file = $state(fileInit());
/** Which dialog is open. */
export const dialogs = $state(dialogsInit());
/** Frame-time probe (F2). */
export const probe = $state(probeInit());
/** Plain, non-reactive state. */
export const live = liveInit();

/** Restore every field to its initial value (editor mount). */
export function resetPixelStore(): void {
  Object.assign(ed, edInit());
  Object.assign(view, viewInit());
  Object.assign(toolState, toolInit());
  Object.assign(colors, colorsInit());
  Object.assign(play, playInit());
  Object.assign(file, fileInit());
  Object.assign(dialogs, dialogsInit());
  Object.assign(probe, probeInit());
  Object.assign(live, liveInit());
}
