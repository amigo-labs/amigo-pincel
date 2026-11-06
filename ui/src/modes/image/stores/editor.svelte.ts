// Reactive editor state (Svelte 5 runes). One store per concern (CLAUDE.md §5.4).
// The document and view stores are per tab (docs/specs/document-tabs.md):
// their per-document keys forward to the active tab (`lib/shell/tab-store`).
import { tabStore } from '../../../lib/shell/tab-store';
import type {
  BrushShape,
  DashPattern,
  EraserMode,
  GradientChannel,
  GradientKind,
  LayerInfo,
  LiveObjectSpec,
  SampleSource,
  ShapeKind,
  ShapeMode,
  TextAlign,
} from '../core/wasm';
import type { DocSaveFormat, DocumentFile } from '../core/document-file';

/** A tab's document state: what the editor mirrors from its core handle. */
export function docInit() {
  return {
    /** The Rust-side document index; `null` while no document is open. */
    handle: null as number | null,
    width: 0,
    height: 0,
    activeLayer: 0,
    canUndo: false,
    canRedo: false,
    /** Entries on the undo stack (mirrored from the core). */
    undoDepth: 0,
    /** `undoDepth` at the last save / open: the document is dirty while
     *  the two differ. */
    savedUndoDepth: 0,
    /** Layers ordered bottom (index 0) to top, mirrored from the core. */
    layers: [] as LayerInfo[],
    /** Whether a selection is currently active (mirrored from the core). */
    hasSelection: false,
    /** The document's file name, as Save writes it (mirrored from
     *  `core/document-file.ts` for the editor title). */
    fileName: 'image.ora',
  };
}

/** One tab's document state. */
export type DocState = ReturnType<typeof docInit>;

/** A tab's canvas view transform (spec §6.4). `fit` re-centres/-scales to
 * the viewport until the user zooms or pans. Zoom is display px per
 * document px. */
export function viewInit() {
  return { zoom: 1, panX: 0, panY: 0, fit: true };
}

/** One tab's view state. */
export type ViewState = ReturnType<typeof viewInit>;

/** One open document (docs/specs/document-tabs.md): its core state, view
 *  and file. `id` is unique for the page load. */
export interface ImageTab {
  id: number;
  doc: DocState;
  view: ViewState;
  file: DocumentFile;
}

/** The open tabs in opening order, and the active one. */
export const tabs = $state({
  list: [] as ImageTab[],
  active: null as ImageTab | null,
});

// Stand-ins read while no tab is open (before the first document loads).
const idleDoc = $state(docInit());
const idleView = $state(viewInit());

const DOC_KEYS = Object.keys(docInit()) as (keyof DocState)[];
const VIEW_KEYS = Object.keys(viewInit()) as (keyof ViewState)[];

// The state all tabs share.
const sharedEditor = $state({
  /** Bumped after every mutation (and on a tab switch) so the canvas
   *  and thumbnails refresh. */
  revision: 0,
  /** A live effect preview composite that overrides the canvas while an
   * effect dialog is open; `null` shows the real composite (spec §11). */
  previewComposite: null as Uint8ClampedArray<ArrayBuffer> | null,
  /** One-line notice for the status bar (e.g. painting on a locked layer). */
  notice: null as string | null,
});

/** The active tab's document and its derived state, plus the state all
 *  tabs share. */
export const editor = tabStore(sharedEditor, DOC_KEYS, () => tabs.active?.doc ?? idleDoc);

/** Transient UI chrome state (not document state). */
export const ui = $state({
  /** True while a modal dialog is open; global shortcuts are suppressed. */
  modalOpen: false,
  /** The active tab is this editor's: it takes keys and pointer input
   *  (document-tabs AC-21). */
  active: true,
});

/** A question the Save flow is waiting on (save-original-format); the
 *  dialog that answers it calls `resolve`. */
export type SavePrompt =
  | { kind: 'jpeg'; resolve: (quality: number | null) => void }
  | {
      kind: 'flatten';
      /** The format is JPEG and the document has no remembered quality:
       *  the prompt asks it too (AC-17). */
      askQuality: boolean;
      resolve: (choice: FlattenChoice | null) => void;
    }
  | {
      /** Save As: the format, its JPEG quality and the merge confirmation
       *  in one dialog (AC-10, AC-15, AC-16). */
      kind: 'format';
      preselect: DocSaveFormat;
      /** The document's own format when it cannot be written back (e.g.
       *  `GIF`): the dialog says so. */
      unwritable: string | null;
      /** The JPEG quality the dialog starts with. */
      quality: number;
      /** A format other than `.ora` loses layers, visibility, opacity or
       *  blend modes and must be confirmed. */
      confirmMerge: boolean;
      resolve: (choice: SaveAsChoice | null) => void;
    };

/** The answer to "Flatten image?": write the flattened composite, or save
 *  the layers as `.ora` instead; `dontAsk` remembers Flatten for the
 *  document; `quality` is the JPEG quality when the prompt asked it.
 *  `null` (Cancel) writes nothing. */
export interface FlattenChoice {
  action: 'flatten' | 'ora';
  dontAsk: boolean;
  quality?: number;
}

/** The answer to the Save As dialog: the format, and the JPEG quality
 *  (used only for JPEG). */
export interface SaveAsChoice {
  format: DocSaveFormat;
  quality: number;
}

/** The Save flow's open prompt, `null` while none is shown. */
export const savePrompt = $state({ value: null as SavePrompt | null });

/** The active tab's canvas view transform. */
const sharedView = $state({});
export const view = tabStore(sharedView, VIEW_KEYS, () => tabs.active?.view ?? idleView);

// Canvas limits live in a rune-free module so node-side specs can test them.
export { MAX_CANVAS_AREA, MAX_CANVAS_DIM, clampCanvasSize } from '../canvas-limits';

/** Zoom limits (spec §6.4: fit to 6400 %-ish). */
export const MIN_ZOOM = 0.02;
export const MAX_ZOOM = 64;

/** The selectable tools (spec §9.2 / §16.2). */
export type ToolKind =
  | 'pencil'
  | 'eraser'
  | 'fill'
  | 'eyedropper'
  | 'move'
  | 'move_selection'
  | 'move_mask'
  | 'rect_select'
  | 'ellipse_select'
  | 'lasso'
  | 'polygon_lasso'
  | 'magic_wand'
  | 'hand'
  | 'zoom'
  | 'gradient'
  | 'recolor'
  | 'smudge'
  | 'clone'
  | 'line'
  | 'rectangle'
  | 'rounded_rect'
  | 'ellipse'
  | 'polygon'
  | 'text';

/** The geometry each shape tool draws. The rail shows them as variants of
 *  one button, with Line on its own key, as Pixel mode does (spec §9.2);
 *  outline / fill stays an option rather than a sixth variant, because the
 *  image core styles a shape instead of baking fill into its kind. */
const SHAPE_OF: Partial<Record<ToolKind, ShapeKind>> = {
  line: 'line',
  rectangle: 'rectangle',
  rounded_rect: 'rounded_rectangle',
  ellipse: 'ellipse',
  polygon: 'polygon',
};

/** The shape `kind` draws, or `null` if it is not a shape tool. */
export function shapeOf(kind: ToolKind): ShapeKind | null {
  return SHAPE_OF[kind] ?? null;
}

export function isShapeTool(kind: ToolKind): boolean {
  return shapeOf(kind) !== null;
}

/** Shape of an in-progress selection gesture, drawn by the overlay (spec §8.5). */
export type SelectionPreview = {
  shape: 'rect' | 'ellipse' | 'lasso' | 'polygon';
  /** Canvas-space points: two corners for rect/ellipse, a path otherwise. */
  points: Array<[number, number]>;
};

/** The live selection gesture preview, or `null` when not selecting. */
export const selectionPreview = $state({ value: null as SelectionPreview | null });

/** In-progress Shapes-tool gesture (drag rectangle), drawn by the overlay. */
export type ShapePreview = {
  kind: ShapeKind;
  /** Drag start and current point (canvas space): endpoints or box corners. */
  a: [number, number];
  b: [number, number];
  /** Polygon side count, mirrored so the preview matches the committed shape. */
  sides: number;
  /** Rounded-rectangle corner radius, mirrored so the preview matches commit. */
  cornerRadius: number;
};

/** The live Shapes-tool preview, or `null` when not drawing a shape. */
export const shapePreview = $state({ value: null as ShapePreview | null });

/** The active document's live object (docs/specs/image-live-objects-ui.md),
 *  mirrored from the core on every sync, and the pointer state over it. */
export const live = $state({
  /** The core's live object, `null` when none is live. */
  object: null as LiveObjectSpec | null,
  /** The cursor over a handle or the body; `null` keeps the tool's. */
  cursor: null as string | null,
});

/** Tool options across the M6 tool suite (spec §9.2, §16.3). */
export const tool = $state({
  /** The active tool. */
  kind: 'pencil' as ToolKind,
  /** Brush diameter in pixels, 1–500 (Pencil/Eraser). */
  size: 8,
  /** Stroke opacity, 1–100 (%). */
  opacity: 100,
  /** Brush tip shape (Pencil/Eraser). */
  shape: 'hard_round' as BrushShape,
  /** Edge hardness, 0–100 (%); applies to soft/flat tips. */
  hardness: 100,
  /** Eraser behavior. */
  eraserMode: 'to_transparent' as EraserMode,
  /** Pen pressure varies the brush size (Pencil/Eraser, spec §17.4). */
  pressureSize: true,
  /** Pen pressure varies the brush opacity. */
  pressureOpacity: false,
  /** Flat-nib angle in degrees, 0–180 (Pencil/Eraser). */
  angle: 45,
  /** A flat nib follows the stylus azimuth where the pen reports tilt. */
  tiltAngle: true,
  /** Fill color-similarity threshold, 0–255. */
  tolerance: 32,
  /** Fill connected region only (vs every matching pixel). */
  contiguous: true,
  /** Fill color sample source. */
  fillSample: 'current_layer' as SampleSource,
  /** Eyedropper sample source ('all_layers' = composite). */
  eyedropperSample: 'all_layers' as SampleSource,
  /** Eyedropper averaging edge length (1/3/5/11/31). */
  sampleSize: 1,
  /** Magic-wand color sample source. */
  wandSample: 'current_layer' as SampleSource,
  /** Feather radius (px) applied to rectangle/ellipse/lasso selections. */
  selectionFeather: 0,
  /** Foreground color as #RRGGBB (spec §4.2 default black). */
  foreground: '#000000',
  /** Background color as #RRGGBB (spec §4.2 default white). */
  background: '#ffffff',
  /** Foreground alpha 0–255 (color-picker spec). */
  foregroundAlpha: 255,
  /** Background alpha 0–255 (color-picker spec). */
  backgroundAlpha: 255,
  /** Recolor: how far a pixel may differ from the background color, 0–255. */
  recolorTolerance: 32,
  /** Smudge: how far a color is dragged, 1–100 (%). */
  smudgeStrength: 60,
  /** Clone Stamp: copy from the active layer or the visible composite. */
  cloneSample: 'current_layer' as SampleSource,
  /** Clone Stamp: the next press on the canvas sets the source point. */
  cloneSetSource: false,
  /** Gradient: which ramp the drag describes (spec §5.4). */
  gradientKind: 'linear' as GradientKind,
  /** Gradient: paint color, or fade the alpha that is already there. */
  gradientChannel: 'color' as GradientChannel,
  /** Shapes: outline / fill / fill+outline. */
  shapeMode: 'outline' as ShapeMode,
  /** Shapes: centered stroke width in pixels, 1–500. */
  strokeWidth: 3,
  /** Shapes: outline dash pattern. */
  shapeDash: 'solid' as DashPattern,
  /** Shapes: polygon side count, 3–100. */
  shapeSides: 5,
  /** Shapes: rounded-rectangle corner radius in pixels. */
  cornerRadius: 16,
  /** Shapes: anti-alias shape edges. */
  shapeAntiAlias: true,
  /** Text: font size in pixels, 6–999. */
  fontSize: 48,
  /** Text: synthetic bold. */
  textBold: false,
  /** Text: synthetic italic. */
  textItalic: false,
  /** Text: anti-alias glyph edges. */
  textAntiAlias: true,
  /** Text: per-line horizontal alignment. */
  textAlign: 'left' as TextAlign,
  /** Text: a stroke below the baseline (docs/specs/image-live-text.md AC-10). */
  textUnderline: false,
  /** Text: a stroke through the lowercase letters (AC-10). */
  textStrikethrough: false,
});

/** Swaps foreground and background colors (spec §4.2, `X`). */
export function swapColors(): void {
  const fg = tool.foreground;
  tool.foreground = tool.background;
  tool.background = fg;
  const fgAlpha = tool.foregroundAlpha;
  tool.foregroundAlpha = tool.backgroundAlpha;
  tool.backgroundAlpha = fgAlpha;
}

/** Resets colors to black/white defaults (spec §4.2, `D`). */
export function resetColors(): void {
  tool.foreground = '#000000';
  tool.background = '#ffffff';
  tool.foregroundAlpha = 255;
  tool.backgroundAlpha = 255;
}

/** View commands the menus and shortcuts call; `MainCanvas` installs the
 *  real implementations on mount (it owns the viewport size). */
export const viewCommands = {
  zoomIn: (): void => {},
  zoomOut: (): void => {},
  fit: (): void => {},
  actual: (): void => {},
};
