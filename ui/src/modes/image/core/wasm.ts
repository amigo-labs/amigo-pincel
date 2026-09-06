// The single entry point to the Image-mode WASM core (fineliner-wasm). No
// other module imports the generated bindings directly (CLAUDE.md §5.4). Document state lives in Rust;
// JS only receives composited pixels and exported bytes (ADR-001).
import initWasm, {
  create_document,
  open_image,
  close_document,
  composite,
  apply_command,
  apply_effect,
  preview_effect,
  pick_color,
  set_active_layer,
  get_layer_thumbnail,
  undo,
  redo,
  export_png,
  export_jpeg,
  export_webp,
  get_document_info,
  get_selection_mask,
  register_font,
} from 'fineliner-wasm';
import wasmUrl from 'fineliner-wasm/fineliner_wasm_bg.wasm?url';

/** Per-layer state for the layers panel (spec §16.5). */
export interface LayerInfo {
  id: string;
  name: string;
  opacity: number;
  blend_mode: BlendMode;
  visible: boolean;
  locked: boolean;
}

export interface DocumentInfo {
  width: number;
  height: number;
  layer_count: number;
  active_layer: number;
  can_undo: boolean;
  can_redo: boolean;
  /** Whether a selection is currently active. */
  has_selection: boolean;
  /** Layers ordered bottom (index 0) to top, matching core storage order. */
  layers: LayerInfo[];
}

export type Rgba = [number, number, number, number];
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

export interface PencilStrokeCommand {
  type: 'pencil_stroke';
  layer: number;
  size: number;
  color: Rgba;
  opacity: number;
  shape: BrushShape;
  hardness: number;
  points: Array<[number, number]>;
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
  mode: EraserMode;
  background: Rgba;
  points: Array<[number, number]>;
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
}

export interface TranslateLayerCommand {
  type: 'translate_layer';
  layer: number;
  dx: number;
  dy: number;
}

/** Erase the selected pixels of a layer — the whole layer when no selection
 * is active (Edit ▸ Clear / Delete key). */
export interface DeleteSelectionCommand {
  type: 'delete_selection';
  layer: number;
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
  | { type: 'flatten_image' };

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
    }
  | { type: 'select_polygon'; points: Array<[number, number]>; mode: SelectionMode; feather: number }
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
export type Interpolation = 'nearest' | 'bilinear' | 'bicubic';

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
  | { type: 'crop_to_selection' };

/** Geometric shape kinds the Shapes tool can draw (spec §9.2 Shapes). */
export type ShapeKind = 'line' | 'rectangle' | 'rounded_rectangle' | 'ellipse' | 'polygon';

/** How a shape's interior and border are painted (spec §9.2 Shapes). */
export type ShapeMode = 'outline' | 'fill' | 'fill_and_outline';

/** Outline dash pattern (spec §9.2 Shapes). */
export type DashPattern = 'solid' | 'dashed' | 'dotted';

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
}

/** Radial-blur mode (spec §11.1). */
export type RadialKind = 'spin' | 'zoom';
/** Edge-detect kernel family (spec §11.3). */
export type EdgeAlgorithm = 'sobel' | 'prewitt' | 'laplacian';
/** Noise distribution (spec §11.4). */
export type NoiseType = 'uniform' | 'gaussian';
/** Whether noise is chromatic or shared across channels (spec §11.4). */
export type NoiseChannels = 'rgb' | 'monochromatic';
/** Curve target channel (spec §12.3). */
export type CurveChannel = 'composite' | 'red' | 'green' | 'blue' | 'alpha';
/** Levels target channel (spec §12.4). */
export type LevelsChannel = 'composite' | 'red' | 'green' | 'blue';
/** Grayscale weighting (spec §12.7). */
export type GrayscaleMethod = 'luminosity' | 'average' | 'bt709' | 'channel_mixer';

/** An effect applied destructively to a layer (spec §11). Mirrors the Rust
 * `EffectSpec`; the drift check in generated-check.ts guards the match. */
export type EffectCommand =
  | { type: 'gaussian_blur'; radius: number }
  | { type: 'box_blur'; width: number; height: number }
  | { type: 'motion_blur'; distance: number; angle: number }
  | { type: 'radial_blur'; amount: number; center_x: number; center_y: number; kind: RadialKind }
  | { type: 'sharpen' }
  | { type: 'unsharp_mask'; amount: number; radius: number; threshold: number }
  | { type: 'emboss'; angle: number; elevation: number; relief: number }
  | { type: 'edge_detect'; algorithm: EdgeAlgorithm; amount: number }
  | { type: 'relief'; angle: number; amount: number }
  | { type: 'add_noise'; amount: number; noise_type: NoiseType; channels: NoiseChannels; seed: number }
  | { type: 'reduce_noise'; radius: number }
  | { type: 'brightness_contrast'; brightness: number; contrast: number; enhanced: boolean }
  | { type: 'hue_saturation'; hue: number; saturation: number; lightness: number; colorize: boolean }
  | { type: 'curves'; channel: CurveChannel; points: Array<[number, number]> }
  | {
      type: 'levels';
      channel: LevelsChannel;
      in_black: number;
      in_white: number;
      gamma: number;
      out_black: number;
      out_white: number;
    }
  | {
      type: 'color_balance';
      shadows: [number, number, number];
      midtones: [number, number, number];
      highlights: [number, number, number];
      preserve_luminosity: boolean;
    }
  | { type: 'invert' }
  | { type: 'grayscale'; method: GrayscaleMethod; mixer: [number, number, number] }
  | { type: 'posterize'; levels: number }
  | { type: 'threshold'; threshold: number };

/** Any command emitted to the core. */
export type ToolCommand =
  | PencilStrokeCommand
  | EraserStrokeCommand
  | FillBucketCommand
  | TranslateLayerCommand
  | DeleteSelectionCommand
  | LayerCommand
  | SelectionCommand
  | TransformCommand
  | DrawShapeCommand
  | DrawTextCommand;

let initialized: Promise<unknown> | null = null;

/** Initializes the WASM module exactly once. Safe to call repeatedly. */
export async function initCore(): Promise<void> {
  if (!initialized) {
    // Same pattern as the Pixel-mode adapter (src/lib/core): pass the asset
    // URL explicitly so Vite fingerprints the .wasm and the precache sees it.
    initialized = initWasm({ module_or_path: wasmUrl });
  }
  await initialized;
}

export const core = {
  createDocument: (width: number, height: number): number => create_document(width, height),
  openImage: (data: Uint8Array, mime: string): number => open_image(data, mime),
  closeDocument: (handle: number): void => close_document(handle),
  composite: (handle: number): Uint8ClampedArray => composite(handle),
  applyCommand: (handle: number, command: ToolCommand): void =>
    apply_command(handle, JSON.stringify(command)),
  /** Applies an effect to a layer's pixels as one undoable step (spec §11). */
  applyEffect: (handle: number, layer: number, effect: EffectCommand): void =>
    apply_effect(handle, layer, JSON.stringify(effect)),
  /** Composites the document with `effect` previewed on `layer`; no mutation. */
  previewEffect: (handle: number, layer: number, effect: EffectCommand): Uint8ClampedArray =>
    preview_effect(handle, layer, JSON.stringify(effect)),
  /** Samples a color; returns RGBA bytes, or an empty array if off-canvas. */
  pickColor: (handle: number, x: number, y: number, sample: SampleSource, size: number): Uint8Array =>
    pick_color(handle, x, y, sample, size),
  /** Selects the active layer (UI state, not undoable). */
  setActiveLayer: (handle: number, index: number): void => set_active_layer(handle, index),
  /** Selection coverage bytes (canvas-sized, row-major), or empty if none. */
  selectionMask: (handle: number): Uint8ClampedArray => get_selection_mask(handle),
  /** Returns a 32×32 RGBA8 thumbnail of the layer with the given id. */
  layerThumbnail: (handle: number, layerId: string): Uint8ClampedArray =>
    get_layer_thumbnail(handle, layerId),
  undo: (handle: number): boolean => undo(handle),
  redo: (handle: number): boolean => redo(handle),
  exportPng: (handle: number, compression: number): Uint8Array => export_png(handle, compression),
  exportJpeg: (handle: number, quality: number): Uint8Array => export_jpeg(handle, quality),
  exportWebp: (handle: number): Uint8Array => export_webp(handle),
  documentInfo: (handle: number): DocumentInfo => get_document_info(handle) as DocumentInfo,
  /** Registers a font's bytes and returns its id for `draw_text` commands. */
  registerFont: (data: Uint8Array): number => register_font(data),
};
