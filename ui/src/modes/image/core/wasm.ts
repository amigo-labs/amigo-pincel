// The single entry point to the Image-mode WASM core (pincel-image-wasm). No
// other module imports the generated bindings directly (CLAUDE.md §5.4). Document state lives in Rust;
// JS only receives composited pixels and exported bytes (ADR-001).
import initWasm, {
  create_document,
  open_image,
  open_project,
  open_psd,
  paste_layer,
  copy_selection,
  cut_selection,
  image_key,
  get_selection_bounds,
  save_project,
  snapshot_document,
  open_snapshot,
  close_document,
  document_count,
  composite,
  composite_region,
  apply_command,
  apply_effect,
  apply_effect_region,
  effect_region_bytes,
  preview_effect,
  pick_color,
  set_active_layer,
  get_layer_thumbnail,
  undo,
  redo,
  agent_call,
  undo_kinds,
  redo_kinds,
  history_dropped,
  jump_history,
  shape_outline,
  export_png,
  export_jpeg,
  export_webp,
  export_bmp,
  export_tiff,
  export_tga,
  export_ico,
  image_to_png,
  get_document_info,
  document_dpi,
  get_selection_mask,
  live_object,
  text_layout,
  register_font,
  read_palette_file,
  write_palette_file,
  vectorizer,
  type Vectorizer,
} from 'pincel-image-wasm';
import wasmUrl from 'pincel-image-wasm/pincel_image_wasm_bg.wasm?url';
import { publishDocumentCount } from '../../../lib/diagnostics';
import type { AgentFile } from '../../../lib/shell/types';
import type { LiveObjectSpec } from './generated/LiveObjectSpec';
import type { TextLayout } from './generated/TextLayout';
import type { TextLayoutRequest } from './generated/TextLayoutRequest';

export type { LiveObjectSpec, TextLayout, TextLayoutRequest };

export type { Vectorizer };

/** Why a PSD opened as its flattened composite (`PsdFallback` in Rust). */
export type PsdFallback = 'no_layers' | 'compressed' | 'unreadable' | 'unrepresentable';

/** One thing a PSD import left out or changed (`PsdNotice` in
 *  pincel-image-core, docs/specs/psd-import.md). */
export type PsdNotice =
  | { kind: 'fallback'; reason: PsdFallback }
  | {
      kind: 'adjustment' | 'text' | 'smart_object' | 'effects' | 'mask' | 'clipping';
      layer: string;
    }
  | { kind: 'blend'; layer: string; key: string }
  | { kind: 'pass_through'; group: string };

/** What `openPsd` returns: the document handle and the import's notices. */
export interface PsdOpened {
  handle: number;
  notices: PsdNotice[];
}

/** Per-layer state for the layers panel (spec §16.5). */
export interface LayerInfo {
  id: string;
  name: string;
  opacity: number;
  blend_mode: BlendMode;
  visible: boolean;
  locked: boolean;
  /** A group has no pixels; its children follow it in `layers`. */
  kind: 'raster' | 'group';
  /** Index of the group the layer sits in; absent at the root (serde-wasm-bindgen
   *  turns `None` into `undefined`). */
  parent?: number;
  /** Number of groups the layer is nested in. */
  depth: number;
  /** Whether a group the layer sits in is locked. */
  locked_by_parent: boolean;
}

export interface DocumentInfo {
  width: number;
  height: number;
  layer_count: number;
  active_layer: number;
  can_undo: boolean;
  can_redo: boolean;
  /** Entries on the undo stack. */
  undo_depth: number;
  /** Whether a selection is currently active. */
  has_selection: boolean;
  /** Layers ordered bottom (index 0) to top, matching core storage order. */
  layers: LayerInfo[];
}

export type Rgba = [number, number, number, number];

/** One entry of a palette file: packed `0xRRGGBBAA` and its name, if any. */
export interface PaletteFileEntry {
  rgba: number;
  name?: string;
}

/** A Save palette format (docs/specs/image-palette-gradient.md AC-06). */
export type PaletteFileFormat = 'gpl' | 'hex' | 'pal' | 'png';
/** Brush tip shapes (spec §9.2 Pencil). */
export type BrushShape = 'hard_round' | 'soft_round' | 'flat';
/** Color sampling source shared by Fill and Eyedropper (spec §9.2). */
export type SampleSource = 'current_layer' | 'all_layers';
/** Eraser behavior (spec §9.2 Eraser). */
export type EraserMode = 'to_transparent' | 'to_background';
/** Layer blend modes as stable snake_case strings (spec §6.1, 12 modes). */
export type BlendMode =
  | 'normal'
  | 'multiply'
  | 'screen'
  | 'overlay'
  | 'darken'
  | 'lighten'
  | 'color_dodge'
  | 'color_burn'
  | 'hard_light'
  | 'soft_light'
  | 'difference'
  | 'exclusion';

/** Normal composites the paint; Overwrite replaces the pixels, alpha
 *  included (docs/specs/image-edit-extras-tool-options.md). */
export type ToolBlend = 'normal' | 'overwrite';

export interface PencilStrokeCommand {
  type: 'pencil_stroke';
  layer: number;
  size: number;
  color: Rgba;
  opacity: number;
  shape: BrushShape;
  hardness: number;
  /** Smooth the hard round tip's edge (docs/specs/image-edit-extras-tool-options.md). */
  anti_alias: boolean;
  blend: ToolBlend;
  points: Array<[number, number]>;
  /** Pen pressure in `[0, 1]` per point (spec §17.4); empty = full. */
  pressure: number[];
  /** Pressure scales the dab size. */
  pressure_size: boolean;
  /** Pressure scales the dab opacity. */
  pressure_opacity: boolean;
  /** Pen azimuth in radians per point; `null` where the pen reported no tilt. */
  azimuth: Array<number | null>;
  /** Flat-nib angle in degrees. */
  angle: number;
  /** The flat nib follows the pen azimuth. */
  tilt_angle: boolean;
  /** Identifies the pointer drag; segments sharing it merge into one undo step. */
  stroke_id: number;
}

export interface EraserStrokeCommand {
  type: 'eraser_stroke';
  layer: number;
  size: number;
  opacity: number;
  shape: BrushShape;
  hardness: number;
  /** Smooth the hard round tip's edge (docs/specs/image-edit-extras-tool-options.md). */
  anti_alias: boolean;
  mode: EraserMode;
  background: Rgba;
  points: Array<[number, number]>;
  pressure: number[];
  pressure_size: boolean;
  pressure_opacity: boolean;
  azimuth: Array<number | null>;
  angle: number;
  tilt_angle: boolean;
  stroke_id: number;
}

export interface FillBucketCommand {
  type: 'fill_bucket';
  layer: number;
  color: Rgba;
  opacity: number;
  tolerance: number;
  contiguous: boolean;
  sample: SampleSource;
  x: number;
  y: number;
  blend: ToolBlend;
}

export interface TranslateLayerCommand {
  type: 'translate_layer';
  layer: number;
  dx: number;
  dy: number;
}

/** Moves the selected pixels; the whole layer when nothing is selected. */
export interface TranslateSelectionCommand {
  type: 'translate_selection';
  layer: number;
  dx: number;
  dy: number;
}

/** A recolor stroke: writes `to` only where it finds `from`. */
export interface RecolorStrokeCommand {
  type: 'recolor_stroke';
  layer: number;
  size: number;
  opacity: number;
  shape: BrushShape;
  hardness: number;
  /** Smooth the hard round tip's edge (docs/specs/image-edit-extras-tool-options.md). */
  anti_alias: boolean;
  from: [number, number, number, number];
  to: [number, number, number, number];
  tolerance: number;
  points: Array<[number, number]>;
  pressure: number[];
  pressure_size: boolean;
  pressure_opacity: boolean;
  azimuth: Array<number | null>;
  angle: number;
  tilt_angle: boolean;
  stroke_id: number;
}

/** A smudge stroke: carries color along the stroke. */
export interface SmudgeStrokeCommand {
  type: 'smudge_stroke';
  layer: number;
  size: number;
  opacity: number;
  shape: BrushShape;
  hardness: number;
  /** Smooth the hard round tip's edge (docs/specs/image-edit-extras-tool-options.md). */
  anti_alias: boolean;
  strength: number;
  points: Array<[number, number]>;
  pressure: number[];
  pressure_size: boolean;
  pressure_opacity: boolean;
  azimuth: Array<number | null>;
  angle: number;
  tilt_angle: boolean;
  stroke_id: number;
}

/** A Clone Stamp stroke: copies the pixels `offset` away through the tip
 *  (docs/specs/clone-stamp.md). */
export interface CloneStrokeCommand {
  type: 'clone_stroke';
  layer: number;
  size: number;
  opacity: number;
  shape: BrushShape;
  hardness: number;
  anti_alias?: boolean;
  /** Source position minus destination position, in whole pixels. */
  offset: [number, number];
  sample: SampleSource;
  points: Array<[number, number]>;
  pressure: number[];
  pressure_size: boolean;
  pressure_opacity: boolean;
  azimuth: Array<number | null>;
  angle: number;
  tilt_angle: boolean;
  stroke_id: number;
}

/** The seven ramps a gradient drag can describe. */
export type GradientKind =
  'linear' | 'reflected' | 'radial' | 'conical' | 'diamond' | 'spiral_cw' | 'spiral_ccw';

/** Whether the ramp paints color or fades alpha. */
export type GradientChannel = 'color' | 'transparency';

export interface DrawGradientCommand {
  type: 'draw_gradient';
  layer: number;
  kind: GradientKind;
  channel: GradientChannel;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  from: [number, number, number, number];
  to: [number, number, number, number];
  opacity: number;
  blend: ToolBlend;
}

/** Moves the selection outline, leaving the pixels under it alone. */
export interface TranslateSelectionMaskCommand {
  type: 'translate_selection_mask';
  dx: number;
  dy: number;
}

/** Erase the selected pixels of a layer — the whole layer when no selection
 * is active (Edit ▸ Clear / Delete key). */
export interface DeleteSelectionCommand {
  type: 'delete_selection';
  layer: number;
}

/** Fill the selected pixels of a layer with `color` — the whole layer when
 * no selection is active (Edit ▸ Fill Selection). */
export interface FillSelectionCommand {
  type: 'fill_selection';
  layer: number;
  color: Rgba;
}

/** Layer-structure and -property commands (spec §5.2 / §7.3). */
export type LayerCommand =
  | { type: 'add_layer'; active: number }
  | { type: 'remove_layer'; index: number }
  | { type: 'duplicate_layer'; index: number }
  | { type: 'rename_layer'; index: number; name: string }
  | { type: 'set_layer_opacity'; index: number; opacity: number }
  | { type: 'set_layer_blend_mode'; index: number; mode: BlendMode }
  | { type: 'set_layer_visible'; index: number; visible: boolean }
  | { type: 'set_layer_locked'; index: number; locked: boolean }
  | { type: 'move_layer'; from: number; to: number }
  | { type: 'merge_down'; index: number }
  | { type: 'merge_visible' }
  | { type: 'flatten_image' }
  | { type: 'add_group' }
  | { type: 'move_layer_to'; index: number; target: number; place: DropPlace }
  | { type: 'merge_group'; index: number };

/** Where a moved layer lands relative to its target (docs/specs/image-layer-groups.md). */
export type DropPlace = 'above' | 'below' | 'into';

/** How a drawn selection combines with the existing one (spec §8.2). */
export type SelectionMode = 'replace' | 'add' | 'subtract' | 'intersect';

/** Selection draws and modifiers (spec §8.3, §8.4 / §9.3). */
export type SelectionCommand =
  | {
      type: 'select_rectangle';
      x: number;
      y: number;
      w: number;
      h: number;
      mode: SelectionMode;
      feather: number;
    }
  | {
      type: 'select_ellipse';
      x: number;
      y: number;
      w: number;
      h: number;
      mode: SelectionMode;
      feather: number;
      anti_alias: boolean;
    }
  | {
      type: 'select_polygon';
      points: Array<[number, number]>;
      mode: SelectionMode;
      feather: number;
      anti_alias: boolean;
    }
  | {
      type: 'select_wand';
      layer: number;
      x: number;
      y: number;
      tolerance: number;
      contiguous: boolean;
      sample: SampleSource;
      mode: SelectionMode;
    }
  | { type: 'select_all' }
  | { type: 'deselect' }
  | { type: 'invert_selection' }
  | { type: 'expand_selection'; radius: number }
  | { type: 'contract_selection'; radius: number }
  | { type: 'feather_selection'; radius: number };

/** Resampling quality for Scale Image (spec §10.5). */
export type Interpolation = 'supersampling' | 'nearest' | 'bilinear' | 'bicubic';

/** The 9-grid anchor for Resize Canvas (spec §10.4). */
export type ResizeAnchor =
  | 'top_left'
  | 'top_center'
  | 'top_right'
  | 'center_left'
  | 'center'
  | 'center_right'
  | 'bottom_left'
  | 'bottom_center'
  | 'bottom_right';

/** Transform commands (spec §10). */
export type TransformCommand =
  | { type: 'transform_layer'; layer: number; op: 'flip_h' | 'flip_v' | 'rotate_180' }
  | { type: 'rotate_layer_90'; layer: number; ccw: boolean }
  | { type: 'flip_canvas'; horizontal: boolean }
  | { type: 'rotate_canvas'; rotation: 'cw90' | 'ccw90' | 'rotate_180' }
  | { type: 'scale_image'; width: number; height: number; interpolation: Interpolation }
  | { type: 'resize_canvas'; width: number; height: number; anchor: ResizeAnchor }
  | { type: 'crop_to_selection' }
  | { type: 'auto_crop' };

/** Geometric shape kinds the Shapes tool can draw (spec §9.2 Shapes). */
export type ShapeKind =
  | 'line'
  | 'rectangle'
  | 'rounded_rectangle'
  | 'ellipse'
  | 'polygon'
  | 'triangle'
  | 'diamond'
  | 'star'
  | 'block_arrow'
  | 'heart'
  | 'callout';

/** How a shape's interior and border are painted (spec §9.2 Shapes). */
export type ShapeMode = 'outline' | 'fill' | 'fill_and_outline';

/** Outline dash pattern (spec §9.2 Shapes). */
export type DashPattern = 'solid' | 'dashed' | 'dotted' | 'dash_dot' | 'dash_dot_dot';

/** How a line ends (docs/specs/image-shapes-curves.md). */
export type LineCap = 'round' | 'flat' | 'arrow';

/** Horizontal text alignment about the placement point (spec §9.2 Text). */
export type TextAlign = 'left' | 'center' | 'right';

/** Draw a geometric shape onto a layer (spec §9.2 Shapes).
 *
 * `points` carries `[a, b]` (endpoints or opposite corners) for line/rectangle/
 * rounded_rectangle/ellipse; `polygon` uses `center`/`radius`/`sides`/`rotation`.
 */
export interface DrawShapeCommand {
  type: 'draw_shape';
  layer: number;
  shape: ShapeKind;
  points?: Array<[number, number]>;
  corner_radius?: number;
  center?: [number, number];
  radius?: number;
  sides?: number;
  rotation?: number;
  mode: ShapeMode;
  stroke_width: number;
  stroke_color: Rgba;
  fill_color: Rgba;
  anti_alias: boolean;
  dash: DashPattern;
  blend: ToolBlend;
  start_cap?: LineCap;
  end_cap?: LineCap;
  star_points?: number;
  inner_ratio?: number;
}

/** Rasterize text onto a layer using a `registerFont` id (spec §9.2 Text). */
export interface DrawTextCommand {
  type: 'draw_text';
  layer: number;
  font_id: number;
  text: string;
  x: number;
  y: number;
  size: number;
  color: Rgba;
  bold: boolean;
  italic: boolean;
  anti_alias: boolean;
  align: TextAlign;
  underline?: boolean;
  strikethrough?: boolean;
  blend?: ToolBlend;
}

/** Two canvas points: endpoints, box corners or a gradient's drag. */
export type PointPair = [[number, number], [number, number]];

/** Begins a live Line or box shape (docs/specs/image-live-objects.md). */
export interface BeginLiveShapeCommand {
  type: 'begin_live_shape';
  layer: number;
  shape: ShapeKind;
  points: PointPair;
  corner_radius?: number;
  sides?: number;
  star_points?: number;
  inner_ratio?: number;
  mode?: ShapeMode;
  stroke_width?: number;
  stroke_color?: Rgba;
  fill_color?: Rgba;
  anti_alias?: boolean;
  dash?: DashPattern;
  blend?: ToolBlend;
  /** A line's control points between the endpoints, in order. */
  control?: Array<[number, number]>;
  start_cap?: LineCap;
  end_cap?: LineCap;
}

/** Begins a live gradient (docs/specs/image-live-objects.md). */
export interface BeginLiveGradientCommand {
  type: 'begin_live_gradient';
  layer: number;
  kind?: GradientKind;
  channel?: GradientChannel;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  from?: Rgba;
  to?: Rgba;
  opacity?: number;
  blend?: ToolBlend;
}

/** Edits the live object; absent fields keep their value. Edits sharing a
 *  `gesture` form one undo step. */
export interface EditLiveCommand {
  type: 'edit_live';
  gesture?: number;
  points?: PointPair;
  shape?: ShapeKind;
  mode?: ShapeMode;
  stroke_width?: number;
  dash?: DashPattern;
  anti_alias?: boolean;
  corner_radius?: number;
  sides?: number;
  star_points?: number;
  inner_ratio?: number;
  stroke_color?: Rgba;
  fill_color?: Rgba;
  kind?: GradientKind;
  channel?: GradientChannel;
  from?: Rgba;
  to?: Rgba;
  opacity?: number;
  text?: string;
  size?: number;
  color?: Rgba;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strikethrough?: boolean;
  align?: TextAlign;
  blend?: ToolBlend;
  /** A line's control points, all of them, in order. */
  control?: Array<[number, number]>;
  start_cap?: LineCap;
  end_cap?: LineCap;
  /** A live text's new font, a `registerFont` id (docs/specs/system-fonts.md). */
  font_id?: number;
}

/** Begins a live text whose box has its top-left at `(x, y)`
 *  (docs/specs/image-live-text.md). */
export interface BeginLiveTextCommand {
  type: 'begin_live_text';
  layer: number;
  font_id: number;
  text: string;
  x: number;
  y: number;
  size?: number;
  color?: Rgba;
  bold?: boolean;
  italic?: boolean;
  anti_alias?: boolean;
  align?: TextAlign;
  underline?: boolean;
  strikethrough?: boolean;
  blend?: ToolBlend;
}

/** Begin, edit, commit or cancel the live object. */
export type LiveCommand =
  | BeginLiveShapeCommand
  | BeginLiveGradientCommand
  | BeginLiveTextCommand
  | EditLiveCommand
  | { type: 'commit_live' }
  | { type: 'cancel_live' }
  | { type: 'cancel_live_gesture'; gesture: number };

/** Any command emitted to the core. */
export type ToolCommand =
  | PencilStrokeCommand
  | EraserStrokeCommand
  | RecolorStrokeCommand
  | SmudgeStrokeCommand
  | CloneStrokeCommand
  | FillBucketCommand
  | TranslateLayerCommand
  | TranslateSelectionCommand
  | TranslateSelectionMaskCommand
  | DeleteSelectionCommand
  | FillSelectionCommand
  | LayerCommand
  | SelectionCommand
  | TransformCommand
  | DrawShapeCommand
  | DrawGradientCommand
  | DrawTextCommand
  | LiveCommand;

let initialized: Promise<unknown> | null = null;
let compiled: WebAssembly.Module | null = null;

/** Fetches and compiles the core. Like wasm-bindgen's own loader, it
 *  compiles the whole body when a host serves the .wasm without the
 *  `application/wasm` type that streaming compilation needs. */
async function compileCore(): Promise<WebAssembly.Module> {
  // Pass the asset URL explicitly (as the Pixel-mode adapter does) so Vite
  // fingerprints the .wasm and the precache sees it.
  const response = await fetch(wasmUrl);
  if (!response.ok) throw new Error(`could not load the Image core (${response.status})`);
  if (response.headers.get('Content-Type')?.startsWith('application/wasm')) {
    return WebAssembly.compileStreaming(response);
  }
  return WebAssembly.compile(await response.arrayBuffer());
}

/**
 * Initializes the WASM module exactly once. Safe to call repeatedly.
 * `module` is a core the page already compiled (a job worker gets it from
 * `compiledCore`): it is instantiated instead of fetched and compiled a
 * second time.
 */
export async function initCore(module?: WebAssembly.Module): Promise<void> {
  if (!initialized) {
    initialized = (module ? Promise.resolve(module) : compileCore())
      .then((m) => {
        compiled = m;
        return initWasm({ module_or_path: m });
      })
      .catch((err: unknown) => {
        // A failed download (offline) is retried by the next caller.
        initialized = null;
        compiled = null;
        throw err;
      });
  }
  await initialized;
  publishDocumentCount('image', document_count);
}

/** The compiled core once `initCore` has loaded it, for a job worker to
 *  instantiate (docs/specs/performance-background-jobs.md). */
export function compiledCore(): WebAssembly.Module | null {
  return compiled;
}

export const core = {
  createDocument: (width: number, height: number): number => create_document(width, height),
  openImage: (data: Uint8Array, mime: string): number => open_image(data, mime),
  /** Pastes an encoded image as a new layer with its top-left at `(x, y)`;
   *  `expand` grows the canvas to cover it there (spec §5.4). */
  pasteLayer: (handle: number, data: Uint8Array, x: number, y: number, expand: boolean): void =>
    paste_layer(handle, data, x, y, expand),
  /** PNG of the active layer (`merged`: the visible composite) inside the selection (spec §5.4). */
  copySelection: (handle: number, merged: boolean): Uint8Array => copy_selection(handle, merged),
  /** `copySelection` of the active layer, then clears it, as one undo step. */
  cutSelection: (handle: number): Uint8Array => cut_selection(handle),
  /** Size + pixel hash of an encoded image; `null` when it can't be decoded. */
  imageKey: (data: Uint8Array): string | null => {
    try {
      return image_key(data);
    } catch {
      return null;
    }
  },
  /** The selection's bounding box `[x, y, w, h]`, or `null` without one. */
  selectionBounds: (handle: number): [number, number, number, number] | null => {
    const [x, y, w, h] = get_selection_bounds(handle);
    return x === undefined || y === undefined || w === undefined || h === undefined
      ? null
      : [x, y, w, h];
  },
  /** Opens an OpenRaster (`.ora`) project with all its layers. */
  openProject: (data: Uint8Array): number => open_project(data),
  /** Opens a PSD (8-bit RGB) with its layers and groups, or its flattened
   *  composite; throws for an unsupported or unreadable file and for a
   *  canvas over `maxDim` per edge or `maxArea` pixels. */
  openPsd: (data: Uint8Array, maxDim: number, maxArea: number): PsdOpened =>
    open_psd(data, maxDim, maxArea) as PsdOpened,
  /** Serializes the document as OpenRaster; `compression` is the 0–9 PNG level. */
  saveProject: (handle: number, compression: number): Uint8Array =>
    save_project(handle, compression),
  /** An uncompressed copy of the whole document for the job worker
   *  (docs/specs/performance-background-jobs.md). */
  snapshotDocument: (handle: number): Uint8Array => snapshot_document(handle),
  /** Opens a `snapshotDocument` copy as a new document. */
  openSnapshot: (data: Uint8Array): number => open_snapshot(data),
  closeDocument: (handle: number): void => close_document(handle),
  // `Clamped<Vec<u8>>` returns cross as fresh `.slice()` copies of wasm memory,
  // so they are ArrayBuffer-backed; narrowing the type here lets ImageData take
  // them without a second copy per frame.
  /** Composites the canvas rect `(x, y, w, h)` resampled to `outW × outH`
   *  (viewport rendering; an empty array when the rect misses the canvas). */
  compositeRegion: (
    handle: number,
    x: number,
    y: number,
    w: number,
    h: number,
    outW: number,
    outH: number,
  ): Uint8ClampedArray<ArrayBuffer> =>
    composite_region(handle, x, y, w, h, outW, outH) as Uint8ClampedArray<ArrayBuffer>,
  composite: (handle: number): Uint8ClampedArray<ArrayBuffer> =>
    composite(handle) as Uint8ClampedArray<ArrayBuffer>,
  /** A tracer over the composite for File ▸ Export ▸ SVG… (vectorize-dialog); free it when done. */
  vectorizer: (handle: number): Vectorizer => vectorizer(handle),
  applyCommand: (handle: number, command: ToolCommand): void =>
    apply_command(handle, JSON.stringify(command)),
  /** Applies effect `name` with positional `params` (the shared
   *  `lib/effects/catalog.ts` wire format) to a layer, inside the selection,
   *  as one undoable step. `false`: the selection is empty, nothing changed. */
  applyEffect: (handle: number, layer: number, name: string, params: number[]): boolean =>
    apply_effect(handle, layer, name, Float64Array.from(params)),
  /** The region effect `name` / `params` would change on `layer` (empty:
   *  nothing, the selection is empty), computed but not committed: the
   *  job worker's half of `applyEffect`. */
  effectRegionBytes: (handle: number, layer: number, name: string, params: number[]): Uint8Array =>
    effect_region_bytes(handle, layer, name, Float64Array.from(params)),
  /** Commits `effectRegionBytes` of the same effect as the undo step
   *  `applyEffect` makes; `false`: the region is empty, nothing changed. */
  applyEffectRegion: (
    handle: number,
    layer: number,
    name: string,
    params: number[],
    region: Uint8Array,
  ): boolean => apply_effect_region(handle, layer, name, Float64Array.from(params), region),
  /** Composites the document with the effect previewed on `layer`; no mutation. */
  previewEffect: (
    handle: number,
    layer: number,
    name: string,
    params: number[],
  ): Uint8ClampedArray<ArrayBuffer> =>
    preview_effect(
      handle,
      layer,
      name,
      Float64Array.from(params),
    ) as Uint8ClampedArray<ArrayBuffer>,
  /** Samples a color; returns RGBA bytes, or an empty array if off-canvas. */
  pickColor: (
    handle: number,
    x: number,
    y: number,
    sample: SampleSource,
    size: number,
  ): Uint8Array => pick_color(handle, x, y, sample, size),
  /** Selects the active layer (UI state, not undoable). */
  setActiveLayer: (handle: number, index: number): void => set_active_layer(handle, index),
  /** Selection coverage bytes (canvas-sized, row-major), or empty if none. */
  selectionMask: (handle: number): Uint8ClampedArray => get_selection_mask(handle),
  /** Returns a 32×32 RGBA8 thumbnail of the layer with the given id. */
  layerThumbnail: (handle: number, layerId: string): Uint8ClampedArray<ArrayBuffer> =>
    get_layer_thumbnail(handle, layerId) as Uint8ClampedArray<ArrayBuffer>,
  undo: (handle: number): boolean => undo(handle),
  /** Runs a `pincel-agent` tool on the document as one undo step and
   *  returns its JSON reply (docs/specs/mcp-live-desktop.md); `file` is
   *  the file the user picked for a reading tool. */
  agentCall: (
    handle: number,
    docId: string,
    name: string,
    argsJson: string,
    file: AgentFile | null,
  ): string => agent_call(handle, docId, name, argsJson, file?.path, file?.bytes),
  redo: (handle: number): boolean => redo(handle),
  /** Kind ids of the undo steps (oldest first) and the redo steps (next
   *  redo first), and whether a step was dropped (history-panel AC-02). */
  history: (handle: number): { undo: string[]; redo: string[]; dropped: boolean } => ({
    undo: undo_kinds(handle),
    redo: redo_kinds(handle),
    dropped: history_dropped(handle),
  }),
  /** Undoes (`delta < 0`) or redoes `|delta|` steps in one call; throws
   *  when a step fails, after the steps before it (history-panel AC-12). */
  jumpHistory: (handle: number, delta: number): number => jump_history(handle, delta),
  exportPng: (handle: number, compression: number): Uint8Array => export_png(handle, compression),
  exportJpeg: (handle: number, quality: number): Uint8Array => export_jpeg(handle, quality),
  exportWebp: (handle: number): Uint8Array => export_webp(handle),
  exportBmp: (handle: number): Uint8Array => export_bmp(handle),
  exportTiff: (handle: number): Uint8Array => export_tiff(handle),
  /** RLE-compressed 32-bit TGA of the composite (image-formats-more). */
  exportTga: (handle: number): Uint8Array => export_tga(handle),
  /** A one-image ICO of the composite; throws above 256 × 256 px. */
  exportIco: (handle: number): Uint8Array => export_ico(handle),
  /** Decodes `data` (MIME type or name as the TGA hint) and re-encodes it
   *  as PNG, so a dropped TGA / DDS / ICO pastes like a browser format. */
  imageToPng: (data: Uint8Array, hint: string): Uint8Array => image_to_png(data, hint),
  documentInfo: (handle: number): DocumentInfo => get_document_info(handle) as DocumentInfo,
  /** Line boxes and caret positions of a text as a live text draws it
   *  with font `fontId` (docs/specs/image-live-text.md AC-22). */
  textLayout: (fontId: number, request: TextLayoutRequest): TextLayout =>
    text_layout(fontId, JSON.stringify(request)) as TextLayout,
  /** The document's stored resolution in pixels per inch (docs/specs/image-view-aids.md AC-23). */
  documentDpi: (handle: number): number => document_dpi(handle),
  /** The live object (docs/specs/image-live-objects.md), or `null`. */
  liveObject: (handle: number): LiveObjectSpec | null =>
    live_object(handle) as LiveObjectSpec | null,
  /** The outline polygon of a box shape over a drag, as the core draws it
   *  (docs/specs/image-shapes-curves-shapes.md AC-12); empty for other
   *  kinds or a box without area. */
  shapeOutline: (
    kind: ShapeKind,
    a: [number, number],
    b: [number, number],
    starPoints: number,
    innerRatio: number,
  ): Array<[number, number]> => {
    const flat = shape_outline(kind, a[0], a[1], b[0], b[1], starPoints, innerRatio);
    const out: Array<[number, number]> = [];
    for (let i = 0; i + 1 < flat.length; i += 2) out.push([flat[i] ?? 0, flat[i + 1] ?? 0]);
    return out;
  },
  /** Registers a font's bytes and returns its id for `draw_text` commands. */
  registerFont: (data: Uint8Array, faceIndex?: number): number => register_font(data, faceIndex),
  /** The entries of a palette file — `.gpl`, `.hex`, `.pal` or a PNG
   *  palette image, detected from the content; throws the reader's reason
   *  (docs/specs/image-palette-gradient.md AC-03, AC-04). */
  readPaletteFile: (data: Uint8Array): PaletteFileEntry[] => {
    const file = read_palette_file(data);
    try {
      const names = file.names;
      return Array.from(file.colors, (rgba, i) => {
        const name = names[i];
        return name ? { rgba, name } : { rgba };
      });
    } finally {
      file.free();
    }
  },
  /** `entries` as a palette file: a GIMP palette called `name`, Lospec
   *  `.hex`, JASC `.pal` or a PNG strip of `cell` px swatches (AC-06). */
  writePaletteFile: (
    entries: readonly PaletteFileEntry[],
    format: PaletteFileFormat,
    name: string,
    cell: number,
  ): Uint8Array<ArrayBuffer> =>
    new Uint8Array(
      write_palette_file(
        Uint32Array.from(entries, (e) => e.rgba),
        entries.map((e) => e.name ?? ''),
        format,
        name,
        cell,
      ),
    ),
};
