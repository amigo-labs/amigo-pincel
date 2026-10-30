// Application logic bridging the WASM core and reactive state. Components call
// these functions on events; no business logic lives in components (CLAUDE.md §5.4).
import {
  core,
  initCore,
  type PencilStrokeCommand,
  type EraserStrokeCommand,
  type RecolorStrokeCommand,
  type SmudgeStrokeCommand,
  type FillBucketCommand,
  type TranslateLayerCommand,
  type TranslateSelectionCommand,
  type TranslateSelectionMaskCommand,
  type DrawGradientCommand,
  type LayerCommand,
  type SelectionCommand,
  type SelectionMode,
  type TransformCommand,
  type Interpolation,
  type ResizeAnchor,
  type BlendMode,
  type Rgba,
  type DrawShapeCommand,
  type ShapeKind,
  type DrawTextCommand,
} from './wasm';
import { editor, tool } from '../stores/editor.svelte';
import {
  formatFromName,
  saveExport,
  sniffFormat,
  type ImageExportFormat as ExportFormat,
  type SaveTarget,
} from '../../../lib/fs';
import {
  docFormat,
  fileStem,
  markSaved,
  resetFile,
  setOpenedFile,
  type DocFormat,
} from './document-file';

/** Parses a #RRGGBB string into RGB bytes, defaulting to black on bad input. */
function hexToRgb(hex: string): [number, number, number] {
  const m = /^#?([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex.trim());
  const [, r, g, b] = m ?? [];
  if (r === undefined || g === undefined || b === undefined) {
    return [0, 0, 0];
  }
  return [parseInt(r, 16), parseInt(g, 16), parseInt(b, 16)];
}

/** Formats RGB bytes as a #RRGGBB string. */
function rgbToHex(r: number, g: number, b: number): string {
  return '#' + [r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('');
}

/** Opacity as a 0–1 fraction, clamped. */
function opacity01(): number {
  return Math.min(1, Math.max(0, tool.opacity / 100));
}

/** The active foreground (or background) color as opaque RGBA. */
function activeColor(useBackground: boolean): Rgba {
  const [r, g, b] = hexToRgb(useBackground ? tool.background : tool.foreground);
  return [r, g, b, 255];
}

/** A #RRGGBB color with the current tool opacity baked into its alpha. */
function colorWithOpacity(hex: string): Rgba {
  const [r, g, b] = hexToRgb(hex);
  return [r, g, b, Math.round(opacity01() * 255)];
}

/** Pen input and the brush dynamics it drives (spec §17.4). */
function penFields(
  pressure: number[],
  azimuth: Array<number | null>,
): {
  pressure: number[];
  pressure_size: boolean;
  pressure_opacity: boolean;
  azimuth: Array<number | null>;
  angle: number;
  tilt_angle: boolean;
} {
  return {
    pressure,
    pressure_size: tool.pressureSize,
    pressure_opacity: tool.pressureOpacity,
    azimuth,
    angle: tool.angle,
    tilt_angle: tool.tiltAngle,
  };
}

/** Refreshes derived document state from the core after a mutation. */
function syncInfo(): void {
  if (editor.handle === null) {
    return;
  }
  const info = core.documentInfo(editor.handle);
  editor.width = info.width;
  editor.height = info.height;
  editor.activeLayer = info.active_layer;
  editor.canUndo = info.can_undo;
  editor.canRedo = info.can_redo;
  editor.undoDepth = info.undo_depth;
  editor.layers = info.layers;
  editor.hasSelection = info.has_selection;
  editor.revision += 1;
}

/** Applies a layer command, then refreshes derived state. */
function applyLayer(cmd: LayerCommand): void {
  if (editor.handle === null) {
    return;
  }
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Adds a transparent layer above the active layer. */
export function addLayer(): void {
  applyLayer({ type: 'add_layer', active: editor.activeLayer });
}

/** Deletes the layer at `index` (blocked by the core if it is the last layer). */
export function deleteLayer(index: number): void {
  applyLayer({ type: 'remove_layer', index });
}

/** Duplicates the layer at `index`. */
export function duplicateLayer(index: number): void {
  applyLayer({ type: 'duplicate_layer', index });
}

/** Reorders the layer at `from` to position `to`. */
export function reorderLayer(from: number, to: number): void {
  if (from !== to) {
    applyLayer({ type: 'move_layer', from, to });
  }
}

/** Renames the layer at `index`. */
export function renameLayer(index: number, name: string): void {
  applyLayer({ type: 'rename_layer', index, name });
}

/** Sets the opacity (0–100 %) of the layer at `index`. */
export function setLayerOpacity(index: number, percent: number): void {
  const opacity = Math.min(1, Math.max(0, percent / 100));
  applyLayer({ type: 'set_layer_opacity', index, opacity });
}

/** Sets the blend mode of the layer at `index`. */
export function setLayerBlendMode(index: number, mode: BlendMode): void {
  applyLayer({ type: 'set_layer_blend_mode', index, mode });
}

/** Shows or hides the layer at `index`. */
export function setLayerVisible(index: number, visible: boolean): void {
  applyLayer({ type: 'set_layer_visible', index, visible });
}

/** Locks or unlocks pixel edits on the layer at `index`. */
export function setLayerLocked(index: number, locked: boolean): void {
  applyLayer({ type: 'set_layer_locked', index, locked });
}

/** Merges the layer at `index` onto the layer below it. */
export function mergeDown(index: number): void {
  applyLayer({ type: 'merge_down', index });
}

/** Flattens all visible layers into one. */
export function mergeVisible(): void {
  applyLayer({ type: 'merge_visible' });
}

/** Flattens every layer onto an opaque white background. */
export function flattenImage(): void {
  applyLayer({ type: 'flatten_image' });
}

/** Selects the active layer (UI state; not an undoable command). */
export function selectLayer(index: number): void {
  if (editor.handle === null) {
    return;
  }
  core.setActiveLayer(editor.handle, index);
  syncInfo();
}

/** Returns a 32×32 RGBA8 thumbnail for the layer with `layerId`, or null.
 *
 * Returns null (rather than throwing) if the id no longer exists — a thumbnail
 * component can still be mounted for a layer that was just deleted or reordered.
 */
export function layerThumbnail(layerId: string): Uint8ClampedArray<ArrayBuffer> | null {
  if (editor.handle === null) {
    return null;
  }
  try {
    return core.layerThumbnail(editor.handle, layerId);
  } catch {
    return null;
  }
}

/** Applies a selection command, then refreshes derived state. */
function applySelection(cmd: SelectionCommand): void {
  if (editor.handle === null) {
    return;
  }
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Selects a rectangle (canvas space) combined per `mode`. */
export function selectRectangle(
  x: number,
  y: number,
  w: number,
  h: number,
  mode: SelectionMode,
): void {
  applySelection({ type: 'select_rectangle', x, y, w, h, mode, feather: tool.selectionFeather });
}

/** Selects an ellipse inscribed in the rectangle (canvas space), per `mode`. */
export function selectEllipse(
  x: number,
  y: number,
  w: number,
  h: number,
  mode: SelectionMode,
): void {
  applySelection({ type: 'select_ellipse', x, y, w, h, mode, feather: tool.selectionFeather });
}

/** Selects a polygon over the given canvas-space points, per `mode`. */
export function selectPolygon(points: Array<[number, number]>, mode: SelectionMode): void {
  applySelection({ type: 'select_polygon', points, mode, feather: tool.selectionFeather });
}

/** Magic-wand selection seeded at the given canvas-space point, per `mode`. */
export function selectWand(x: number, y: number, mode: SelectionMode): void {
  applySelection({
    type: 'select_wand',
    layer: editor.activeLayer,
    x,
    y,
    tolerance: tool.tolerance,
    contiguous: tool.contiguous,
    sample: tool.wandSample,
    mode,
  });
}

/** Selects the whole canvas (Ctrl+A). */
export function selectAll(): void {
  applySelection({ type: 'select_all' });
}

/** Clears the selection (Ctrl+D). */
export function deselect(): void {
  applySelection({ type: 'deselect' });
}

/** Inverts the selection (Ctrl+Shift+I). */
export function invertSelection(): void {
  applySelection({ type: 'invert_selection' });
}

/** Grows the selection by `radius` pixels. */
export function expandSelection(radius: number): void {
  applySelection({ type: 'expand_selection', radius });
}

/** Shrinks the selection by `radius` pixels. */
export function contractSelection(radius: number): void {
  applySelection({ type: 'contract_selection', radius });
}

/** Softens the selection edges by `radius` pixels. */
export function featherSelection(radius: number): void {
  applySelection({ type: 'feather_selection', radius });
}

/** The active selection's coverage mask (canvas-sized), or null if none. */
export function selectionMask(): Uint8ClampedArray | null {
  if (editor.handle === null) {
    return null;
  }
  const mask = core.selectionMask(editor.handle);
  return mask.length > 0 ? mask : null;
}

/** Applies a transform command, then refreshes derived state. */
function applyTransform(cmd: TransformCommand): void {
  if (editor.handle === null) {
    return;
  }
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Flips or 180°-rotates the active layer (spec §10.2/§10.3). */
export function transformLayer(op: 'flip_h' | 'flip_v' | 'rotate_180'): void {
  applyTransform({ type: 'transform_layer', layer: editor.activeLayer, op });
}

/** Rotates the active layer 90° (counter-clockwise when `ccw`). */
export function rotateLayer90(ccw: boolean): void {
  applyTransform({ type: 'rotate_layer_90', layer: editor.activeLayer, ccw });
}

/** Flips the whole canvas horizontally or vertically (spec §10.2). */
export function flipCanvas(horizontal: boolean): void {
  applyTransform({ type: 'flip_canvas', horizontal });
}

/** Rotates the whole canvas (spec §10.3). */
export function rotateCanvas(rotation: 'cw90' | 'ccw90' | 'rotate_180'): void {
  applyTransform({ type: 'rotate_canvas', rotation });
}

/** Scales the whole image to a new size (spec §10.5). */
export function scaleImage(width: number, height: number, interpolation: Interpolation): void {
  applyTransform({ type: 'scale_image', width, height, interpolation });
}

/** Resizes the canvas, anchoring existing content (spec §10.4). */
export function resizeCanvas(width: number, height: number, anchor: ResizeAnchor): void {
  applyTransform({ type: 'resize_canvas', width, height, anchor });
}

/**
 * Pastes an encoded image as a new layer above the active one, its top-left
 * at canvas `(x, y)`; `expand` first grows the canvas to cover it there
 * (spec §5.4). Throws when the core can't decode the image.
 */
export function pasteImage(bytes: Uint8Array, x: number, y: number, expand: boolean): void {
  if (editor.handle === null) {
    return;
  }
  core.pasteLayer(editor.handle, bytes, x, y, expand);
  syncInfo();
}

/**
 * Copy / Copy Merged / Cut (spec §5.4): the PNG for the clipboard and the
 * canvas position it came from; `null` without a document. Cut also clears
 * the copied pixels as one undo step. Throws on an empty selection.
 */
export function copyImage(
  kind: 'copy' | 'merged' | 'cut',
): { png: Uint8Array; x: number; y: number } | null {
  const handle = editor.handle;
  if (handle === null) {
    return null;
  }
  const [x, y] = core.selectionBounds(handle) ?? [0, 0];
  const png = kind === 'cut' ? core.cutSelection(handle) : core.copySelection(handle, kind === 'merged');
  if (kind === 'cut') syncInfo();
  return { png, x, y };
}

/** Crops the canvas to the current selection's bounding box (spec §10.6). */
export function cropToSelection(): void {
  applyTransform({ type: 'crop_to_selection' });
}

/** Applies an effect to the active layer as one undoable step (spec §11). */
/** Applies an effect to the active layer, inside the selection. Returns
 *  `false` when the selection is empty and nothing changed. */
export function applyEffect(name: string, params: number[]): boolean {
  if (editor.handle === null) {
    return false;
  }
  const changed = core.applyEffect(editor.handle, editor.activeLayer, name, params);
  editor.previewComposite = null;
  syncInfo();
  return changed;
}

/** Sets the live effect preview over the canvas without mutating the document
 * (spec §11 live preview). Call from an open effect dialog. */
export function previewEffect(name: string, params: number[]): void {
  if (editor.handle === null) {
    return;
  }
  editor.previewComposite = core.previewEffect(editor.handle, editor.activeLayer, name, params);
  editor.revision += 1;
}

/** Clears any live effect preview and restores the real composite. */
export function clearEffectPreview(): void {
  if (editor.previewComposite !== null) {
    editor.previewComposite = null;
    editor.revision += 1;
  }
}

/** Draws the active shape between two canvas-space points (spec §9.2 Shapes).
 *
 * `a`/`b` are the drag endpoints (line) or opposite corners of the drag box
 * (rectangle/rounded/ellipse); a polygon is inscribed in that box. Stroke uses
 * the foreground color, fill the background color, both at the tool opacity. */
export function drawShape(
  a: [number, number],
  b: [number, number],
  shape: ShapeKind,
): void {
  if (editor.handle === null) {
    return;
  }
  const cmd: DrawShapeCommand = {
    type: 'draw_shape',
    layer: editor.activeLayer,
    shape: shape,
    mode: tool.shapeMode,
    stroke_width: tool.strokeWidth,
    stroke_color: colorWithOpacity(tool.foreground),
    fill_color: colorWithOpacity(tool.background),
    anti_alias: tool.shapeAntiAlias,
    dash: tool.shapeDash,
  };
  if (shape === 'polygon') {
    const w = Math.abs(b[0] - a[0]);
    const h = Math.abs(b[1] - a[1]);
    cmd.center = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
    cmd.radius = Math.min(w, h) / 2;
    cmd.sides = tool.shapeSides;
    cmd.rotation = 0;
  } else {
    cmd.points = [a, b];
    if (shape === 'rounded_rectangle') {
      cmd.corner_radius = tool.cornerRadius;
    }
  }
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

// The default font is fetched once and registered with the core (ADR-012,
// Option B). `draw_text` references it by id. The promise is cached so
// concurrent text commits await a single load/registration.
let fontLoad: Promise<number> | null = null;

/** Loads and registers the bundled default font, returning its core id.
 *
 * The successful promise is cached so repeat commits reuse one registration; a
 * failed load resets the cache so a later attempt can retry (offline/404). */
async function ensureFont(): Promise<number> {
  if (!fontLoad) {
    fontLoad = (async () => {
      await initCore();
      const res = await fetch(`${import.meta.env.BASE_URL}fonts/LiberationSans-Regular.ttf`);
      if (!res.ok) {
        throw new Error(`font fetch failed: ${res.status}`);
      }
      const bytes = new Uint8Array(await res.arrayBuffer());
      return core.registerFont(bytes);
    })().catch((err) => {
      fontLoad = null; // allow a later commit to retry
      throw err;
    });
  }
  return fontLoad;
}

/** Rasterizes `text` at a canvas-space point with the active text style.
 *
 * Empty/whitespace text is a no-op. The font is loaded lazily on first use; if
 * it cannot be loaded the commit is silently skipped rather than rejecting. */
export async function renderText(x: number, y: number, text: string): Promise<void> {
  if (editor.handle === null || text.trim().length === 0) {
    return;
  }
  let fontId: number;
  try {
    fontId = await ensureFont();
  } catch {
    return; // font unavailable — skip the commit
  }
  if (editor.handle === null) {
    return; // document closed while the font was loading
  }
  const cmd: DrawTextCommand = {
    type: 'draw_text',
    layer: editor.activeLayer,
    font_id: fontId,
    text,
    x,
    y,
    size: tool.fontSize,
    color: colorWithOpacity(tool.foreground),
    bold: tool.textBold,
    italic: tool.textItalic,
    anti_alias: tool.textAntiAlias,
    align: tool.textAlign,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Marks the document as having unsaved changes (e.g. after recovering an
 *  autosave snapshot, which is newer than any file on disk). */
export function markUnsaved(): void {
  editor.savedUndoDepth = -1;
}

/** OpenRaster bytes of the current document; `fast` trades size for speed
 *  (autosave). `null` when no document is open. */
export function projectBytes(fast = false): Uint8Array | null {
  if (editor.handle === null) return null;
  return core.saveProject(editor.handle, fast ? 1 : 6);
}

/** Creates a blank document and makes it the active one. */
export async function newDocument(width: number, height: number): Promise<void> {
  await initCore();
  // Create the new document first; only replace (and close) the current one on
  // success, so a failure leaves the open document untouched.
  const handle = core.createDocument(width, height);
  if (editor.handle !== null) {
    core.closeDocument(editor.handle);
  }
  editor.handle = handle;
  // A blank document must not inherit a previously opened file's name.
  resetFile();
  syncInfo();
  markSaved();
}

/** Opens already-read bytes: an OpenRaster project with all its layers, or
 * a flat image as a single-layer document. `name` is the file name Save
 * writes back to; `mime` is a decoder hint. `target` carries the file's FSA
 * handle / Tauri path so Save writes it in place, and optionally the format
 * Save writes (a recovered snapshot is OpenRaster bytes of, say, a PNG);
 * by default the format detected from the bytes. */
export async function openBytes(
  bytes: Uint8Array,
  name: string,
  mime: string,
  target: Pick<SaveTarget, 'handle' | 'path'> & { format?: DocFormat | null } = {
    handle: null,
    path: null,
  },
): Promise<void> {
  await initCore();
  const sniffed = sniffFormat(bytes);
  const ora = sniffed === 'ora';
  // Decode before closing the current document: a corrupt file must not
  // destroy the open document or leave `editor.handle` pointing at a closed slot.
  const handle = ora ? core.openProject(bytes) : core.openImage(bytes, mime || 'image/png');
  if (editor.handle !== null) {
    core.closeDocument(editor.handle);
  }
  editor.handle = handle;
  const format =
    target.format !== undefined ? target.format : docFormat(sniffed ?? formatFromName(name));
  setOpenedFile(name, format, target);
  syncInfo();
  markSaved();
}

/** Opens an encoded image file as a new single-layer document. */
export async function openFile(file: File): Promise<void> {
  await openBytes(new Uint8Array(await file.arrayBuffer()), file.name, file.type);
}

/** Applies a pencil stroke over the given canvas-space points.
 *
 * `strokeId` ties segments of one pointer drag together so they collapse into a
 * single undo step; a fresh id per drag keeps distinct strokes separate.
 * `useBackground` paints the background color (right-button, spec §9.2). */
export function paintStroke(
  points: Array<[number, number]>,
  strokeId: number,
  useBackground = false,
  pressure: number[] = [],
  azimuth: Array<number | null> = [],
): void {
  if (editor.handle === null || points.length === 0) {
    return;
  }
  const cmd: PencilStrokeCommand = {
    type: 'pencil_stroke',
    layer: editor.activeLayer,
    size: tool.size,
    color: activeColor(useBackground),
    opacity: opacity01(),
    shape: tool.shape,
    hardness: Math.min(1, Math.max(0, tool.hardness / 100)),
    points,
    ...penFields(pressure, azimuth),
    stroke_id: strokeId,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Applies an eraser stroke over the given canvas-space points. */
export function eraseStroke(
  points: Array<[number, number]>,
  strokeId: number,
  pressure: number[] = [],
  azimuth: Array<number | null> = [],
): void {
  if (editor.handle === null || points.length === 0) {
    return;
  }
  const cmd: EraserStrokeCommand = {
    type: 'eraser_stroke',
    layer: editor.activeLayer,
    size: tool.size,
    opacity: opacity01(),
    shape: tool.shape,
    hardness: Math.min(1, Math.max(0, tool.hardness / 100)),
    mode: tool.eraserMode,
    background: activeColor(true), // 'To Background' erases to the bg swatch
    points,
    ...penFields(pressure, azimuth),
    stroke_id: strokeId,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Recolors along the given points: the foreground replaces the background
 *  color wherever the stroke finds it, within the tool's tolerance. */
export function recolorStroke(
  points: Array<[number, number]>,
  strokeId: number,
  pressure: number[] = [],
  azimuth: Array<number | null> = [],
): void {
  if (editor.handle === null || points.length === 0) {
    return;
  }
  const cmd: RecolorStrokeCommand = {
    type: 'recolor_stroke',
    layer: editor.activeLayer,
    size: tool.size,
    opacity: opacity01(),
    shape: tool.shape,
    hardness: Math.min(1, Math.max(0, tool.hardness / 100)),
    from: activeColor(true),
    to: activeColor(false),
    tolerance: tool.recolorTolerance,
    points,
    ...penFields(pressure, azimuth),
    stroke_id: strokeId,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Smudges along the given points. */
export function smudgeStroke(
  points: Array<[number, number]>,
  strokeId: number,
  pressure: number[] = [],
  azimuth: Array<number | null> = [],
): void {
  if (editor.handle === null || points.length === 0) {
    return;
  }
  const cmd: SmudgeStrokeCommand = {
    type: 'smudge_stroke',
    layer: editor.activeLayer,
    size: tool.size,
    opacity: opacity01(),
    shape: tool.shape,
    hardness: Math.min(1, Math.max(0, tool.hardness / 100)),
    strength: Math.min(1, Math.max(0, tool.smudgeStrength / 100)),
    points,
    ...penFields(pressure, azimuth),
    stroke_id: strokeId,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Flood-fills from the given canvas-space point with the active color. */
export function fillAt(x: number, y: number, useBackground = false): void {
  if (editor.handle === null) {
    return;
  }
  const cmd: FillBucketCommand = {
    type: 'fill_bucket',
    layer: editor.activeLayer,
    color: activeColor(useBackground),
    opacity: opacity01(),
    tolerance: tool.tolerance,
    contiguous: tool.contiguous,
    sample: tool.fillSample,
    x,
    y,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Erases the selected pixels of the active layer (Delete / Edit ▸ Clear).
 *
 * With no active selection the whole layer is cleared. */
export function deleteSelection(): void {
  if (editor.handle === null) {
    return;
  }
  core.applyCommand(editor.handle, { type: 'delete_selection', layer: editor.activeLayer });
  syncInfo();
}

/** Translates the active layer's contents by `(dx, dy)` pixels. */
export function moveLayer(dx: number, dy: number): void {
  if (editor.handle === null || (dx === 0 && dy === 0)) {
    return;
  }
  const cmd: TranslateLayerCommand = {
    type: 'translate_layer',
    layer: editor.activeLayer,
    dx,
    dy,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Move Selected Pixels: drags the selection's content, leaving
 *  transparency behind. Without a selection this moves the whole layer. */
export function moveSelectionContent(dx: number, dy: number): void {
  if (editor.handle === null || (dx === 0 && dy === 0)) {
    return;
  }
  const cmd: TranslateSelectionCommand = {
    type: 'translate_selection',
    layer: editor.activeLayer,
    dx,
    dy,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Gradient: the drag a → b is the ramp (spec §5.4). The foreground is
 *  the start color, the background the end; Transparency mode ignores both
 *  and fades what is already there. */
export function drawGradient(a: [number, number], b: [number, number]): void {
  if (editor.handle === null) {
    return;
  }
  const cmd: DrawGradientCommand = {
    type: 'draw_gradient',
    layer: editor.activeLayer,
    kind: tool.gradientKind,
    channel: tool.gradientChannel,
    x0: a[0],
    y0: a[1],
    x1: b[0],
    y1: b[1],
    // Opaque endpoints: `Gradient::sample` applies `opacity` itself, and
    // `colorWithOpacity` would square it — a 50 % Colour ramp painted at 25 %
    // while the Transparency channel used 50 %.
    from: activeColor(false),
    to: activeColor(true),
    opacity: opacity01(),
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Move Selection: drags the outline only. */
export function moveSelectionMask(dx: number, dy: number): void {
  if (editor.handle === null || (dx === 0 && dy === 0)) {
    return;
  }
  const cmd: TranslateSelectionMaskCommand = {
    type: 'translate_selection_mask',
    dx,
    dy,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
}

/** Samples a color at the given point and sets it as the active color.
 *
 * Left button sets the foreground, right button the background (spec §9.2). */
export function sampleColor(x: number, y: number, toBackground = false): void {
  if (editor.handle === null) {
    return;
  }
  const rgba = core.pickColor(editor.handle, x, y, tool.eyedropperSample, tool.sampleSize);
  const [r, g, b] = rgba;
  if (r === undefined || g === undefined || b === undefined) {
    return; // off-canvas
  }
  const hex = rgbToHex(r, g, b);
  if (toBackground) {
    tool.background = hex;
  } else {
    tool.foreground = hex;
  }
}

/** Reads the current composite as RGBA8 for rendering. */
/** Composites the document rect `(x, y, w, h)` at `outW × outH` for the
 *  viewport renderer; `null` when no document is open. */
export function readCompositeRegion(
  x: number,
  y: number,
  w: number,
  h: number,
  outW: number,
  outH: number,
): Uint8ClampedArray<ArrayBuffer> | null {
  if (editor.handle === null) {
    return null;
  }
  return core.compositeRegion(editor.handle, x, y, w, h, outW, outH);
}

export function readComposite(): Uint8ClampedArray<ArrayBuffer> | null {
  if (editor.handle === null) {
    return null;
  }
  return core.composite(editor.handle);
}

/** Undoes the last command. */
export function undo(): void {
  if (editor.handle !== null && core.undo(editor.handle)) {
    syncInfo();
  }
}

/** Redoes the last undone command. */
export function redo(): void {
  if (editor.handle !== null && core.redo(editor.handle)) {
    syncInfo();
  }
}

export type { ExportFormat };

/** Exports the composite in `format` through the shared file layer
 * (FSA save picker / Tauri dialog / download). Resolves `false` when the
 * user cancelled the picker.
 *
 * `quality` (1–100) applies to JPEG only; PNG and WebP are lossless. */
export async function exportImage(format: ExportFormat = 'png', quality = 90): Promise<boolean> {
  if (editor.handle === null) {
    return false;
  }
  let bytes: Uint8Array;
  switch (format) {
    case 'jpeg':
      bytes = core.exportJpeg(editor.handle, Math.min(100, Math.max(1, Math.round(quality))));
      break;
    case 'webp':
      bytes = core.exportWebp(editor.handle);
      break;
    case 'png':
      bytes = core.exportPng(editor.handle, 6);
      break;
  }
  // Copy into a fresh ArrayBuffer so the file layer owns standalone memory.
  const name = `${fileStem()}.${format === 'jpeg' ? 'jpg' : format}`;
  return saveExport(new Uint8Array(bytes), name, format);
}
