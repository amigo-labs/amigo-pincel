// Reactive editor state (Svelte 5 runes). One store per concern (CLAUDE.md §5.4).
import type {
  BrushShape,
  DashPattern,
  EraserMode,
  GradientChannel,
  GradientKind,
  LayerInfo,
  SampleSource,
  ShapeKind,
  ShapeMode,
  TextAlign,
} from '../core/wasm';

/** The open document and its derived state. `handle` is the Rust-side index. */
export const editor = $state({
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
  /** Bumped after every mutation so the canvas and thumbnails refresh. */
  revision: 0,
  /** A live effect preview composite that overrides the canvas while an effect
   * dialog is open; `null` shows the real composite (spec §11 live preview). */
  previewComposite: null as Uint8ClampedArray<ArrayBuffer> | null,
  /** The document's file name, as Save writes it (mirrored from
   *  `core/document-file.ts` for the editor title). */
  fileName: 'image.ora',
  /** One-line notice for the status bar (e.g. painting on a locked layer). */
  notice: null as string | null,
});

/** Transient UI chrome state (not document state). */
export const ui = $state({
  /** True while a modal dialog is open; global shortcuts are suppressed. */
  modalOpen: false,
});

/** A question the Save flow is waiting on (save-original-format); the
 *  dialog that answers it calls `resolve`. */
export type SavePrompt = { kind: 'jpeg'; resolve: (quality: number | null) => void };

/** The Save flow's open prompt, `null` while none is shown. */
export const savePrompt = $state({ value: null as SavePrompt | null });

/** Canvas view transform (spec §6.4). `fit` re-centres/-scales to the viewport
 * until the user zooms or pans. Zoom is display px per document px. */
export const view = $state({
  zoom: 1,
  panX: 0,
  panY: 0,
  fit: true,
});

/** Largest canvas edge the UI lets the user create / resize / scale to.
 * The renderer only ever draws the visible rect into a viewport-sized
 * canvas, so browser canvas limits no longer cap the document; memory
 * does. The core accepts 32767 per edge, but each layer is width × height
 * × 4 bytes of wasm memory (4 GiB address space in total), so the edge is
 * capped at 16384 and the area at `MAX_CANVAS_AREA`. */
export const MAX_CANVAS_DIM = 16384;

/** Largest canvas area (64 M px = 256 MiB per layer): room for a few
 *  layers, their undo snapshots and the composite inside wasm32 memory. */
export const MAX_CANVAS_AREA = 8192 * 8192;

/** Clamp a requested canvas size to the edge and area limits, scaling
 *  both edges down together when only the area is too large. */
export function clampCanvasSize(width: number, height: number): [number, number] {
  let w = Math.max(1, Math.min(MAX_CANVAS_DIM, Math.round(width)));
  let h = Math.max(1, Math.min(MAX_CANVAS_DIM, Math.round(height)));
  if (w * h > MAX_CANVAS_AREA) {
    const k = Math.sqrt(MAX_CANVAS_AREA / (w * h));
    w = Math.max(1, Math.floor(w * k));
    h = Math.max(1, Math.floor(h * k));
  }
  return [w, h];
}

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
  /** Recolor: how far a pixel may differ from the background color, 0–255. */
  recolorTolerance: 32,
  /** Smudge: how far a color is dragged, 1–100 (%). */
  smudgeStrength: 60,
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
});

/** Swaps foreground and background colors (spec §4.2, `X`). */
export function swapColors(): void {
  const fg = tool.foreground;
  tool.foreground = tool.background;
  tool.background = fg;
}

/** Resets colors to black/white defaults (spec §4.2, `D`). */
export function resetColors(): void {
  tool.foreground = '#000000';
  tool.background = '#ffffff';
}

/** View commands the menus and shortcuts call; `MainCanvas` installs the
 *  real implementations on mount (it owns the viewport size). */
export const viewCommands = {
  zoomIn: (): void => {},
  zoomOut: (): void => {},
  fit: (): void => {},
  actual: (): void => {},
};
