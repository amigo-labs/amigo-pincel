// Application logic bridging the WASM core and reactive state. Components call
// these functions on events; no business logic lives in components (CLAUDE.md §5.4).
import type { Rgba8 } from '../../../lib/color';
import { noteUsed } from '../../../lib/colors/recent.svelte';
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
  type DropPlace,
  type Rgba,
  type DrawShapeCommand,
  type ShapeKind,
  type Vectorizer,
} from './wasm';
import type { SvgTrace } from '../../../lib/components/SvgExportDialog.svelte';
import { editor, live, tool, type ImageTab } from '../stores/editor.svelte';
import {
  formatFromName,
  saveExport,
  sniffFormat,
  type ImageExportFormat as ExportFormat,
  type SaveTarget,
} from '../../../lib/fs';
import {
  blankFile,
  docFormat,
  fileStem,
  markSaved,
  openedFile,
  type DocFormat,
} from './document-file';
import { psdNotice, takePsd } from './psd-open';
import { openTab } from './tabs.svelte';
import { encodeInBackground, type EffectJob } from '../jobs';
import { gestureControl } from '../tools/gesture-control';
import { sameHistory } from '../../../lib/history/names';
import { t } from '../../../lib/i18n/index.svelte';

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
export function opacity01(): number {
  return Math.min(1, Math.max(0, tool.opacity / 100));
}

/** The active foreground (or background) color with its own alpha; the
 *  core combines it with the tool opacity it gets separately. */
export function activeColor(useBackground: boolean): Rgba {
  const [r, g, b] = hexToRgb(useBackground ? tool.background : tool.foreground);
  return [r, g, b, useBackground ? tool.backgroundAlpha : tool.foregroundAlpha];
}

/** The foreground (or background) color as RGBA bytes with its alpha. */
function usedColor(useBackground: boolean): Rgba8 {
  const [r, g, b, a] = activeColor(useBackground);
  return { r, g, b, a };
}

/** Records `used` (foreground first) as painted with when the document
 *  gained an undo step since `depthBefore` (color-picker AC-24). A merged
 *  stroke adds its step once, so it is recorded once. */
export function notePainted(depthBefore: number, used: readonly Rgba8[]): void {
  if (editor.undoDepth > depthBefore) noteUsed('image', used);
}

/** The colors a Line or box shape paints with under the current options: a
 *  line strokes only; a shape strokes with the foreground and fills with the
 *  background, as its mode says (color-picker AC-24). */
export function shapeColors(shape: ShapeKind): Rgba8[] {
  const mode = shape === 'line' ? 'outline' : tool.shapeMode;
  if (mode === 'outline') return [usedColor(false)];
  if (mode === 'fill') return [usedColor(true)];
  return [usedColor(false), usedColor(true)];
}

/** The colors a gradient paints with: from and to in the Color channel,
 *  none in Transparency mode, which fades what is there (AC-24). */
export function gradientColors(): Rgba8[] {
  return tool.gradientChannel === 'color' ? [usedColor(false), usedColor(true)] : [];
}

/** A color with the tool opacity baked into its alpha: round(alpha ×
 *  opacity / 100) (color-picker Decisions 2026-10-29). */
export function colorWithOpacity(hex: string, alpha: number): Rgba {
  const [r, g, b] = hexToRgb(hex);
  return [r, g, b, Math.round((alpha * Math.min(100, Math.max(0, tool.opacity))) / 100)];
}

/** Pen input and the brush dynamics it drives (spec §17.4). */
export function penFields(
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

/** Refreshes the active tab's derived document state from the core after a
 *  mutation (and when a tab becomes active). */
export function syncInfo(): void {
  if (editor.handle === null) {
    return;
  }
  const info = core.documentInfo(editor.handle);
  // A document command or an active-layer change is a commit trigger the
  // core runs on its own: it closes an empty text box at once, before a
  // Text press that follows the commit opens the next one
  // (docs/specs/image-live-text.md AC-01, AC-11).
  if (info.undo_depth !== editor.undoDepth || info.active_layer !== editor.activeLayer) {
    gestureControl.closeTextBox();
  }
  editor.width = info.width;
  editor.height = info.height;
  editor.activeLayer = info.active_layer;
  editor.canUndo = info.can_undo;
  editor.canRedo = info.can_redo;
  editor.undoDepth = info.undo_depth;
  const history = core.history(editor.handle);
  if (!sameHistory(history, editor.history)) editor.history = history;
  editor.layers = info.layers;
  editor.hasSelection = info.has_selection;
  live.object = core.liveObject(editor.handle);
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

/** Adds an empty group named `Group N` on top of the stack and makes it active. */
export function addGroup(): void {
  applyLayer({ type: 'add_group' });
}

/** Moves the layer at `index` — a group with its subtree — above, below or
 *  into the layer at `target`, as one undo step. A move the core refuses
 *  (a group into itself) leaves the document as it was. */
export function moveLayerTo(index: number, target: number, place: DropPlace): void {
  if (editor.handle === null || index === target) {
    return;
  }
  try {
    core.applyCommand(editor.handle, { type: 'move_layer_to', index, target, place });
  } catch {
    // Refused: nothing changed.
  }
  syncInfo();
}

/** Replaces the group at `index` and its subtree with one raster layer. */
export function mergeGroup(index: number): void {
  applyLayer({ type: 'merge_group', index });
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
  applySelection({
    type: 'select_ellipse',
    x,
    y,
    w,
    h,
    mode,
    feather: tool.selectionFeather,
    anti_alias: tool.antiAlias,
  });
}

/** Selects a polygon over the given canvas-space points, per `mode`. */
export function selectPolygon(points: Array<[number, number]>, mode: SelectionMode): void {
  applySelection({
    type: 'select_polygon',
    points,
    mode,
    feather: tool.selectionFeather,
    anti_alias: tool.antiAlias,
  });
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
  const png =
    kind === 'cut' ? core.cutSelection(handle) : core.copySelection(handle, kind === 'merged');
  if (kind === 'cut') syncInfo();
  return { png, x, y };
}

/** Crops the canvas to the current selection's bounding box (spec §10.6). */
export function cropToSelection(): void {
  applyTransform({ type: 'crop_to_selection' });
}

/** Image ▸ Auto Crop (docs/specs/image-resize-autocrop.md AC-13 – AC-15):
 *  `false` when there was nothing to crop, which adds no undo step. */
export function autoCrop(): boolean {
  const [w, h] = [editor.width, editor.height];
  applyTransform({ type: 'auto_crop' });
  return editor.width !== w || editor.height !== h;
}

/** The active document's stored resolution in pixels per inch, or 0
 *  without a document (docs/specs/image-view-aids.md AC-23). */
export function documentDpi(): number {
  return editor.handle === null ? 0 : core.documentDpi(editor.handle);
}

/** Effect `name` / `params` on the active layer, as a job
 *  (docs/specs/performance-background-jobs.md). */
export function effectJobFor(name: string, params: number[]): EffectJob {
  return { layer: editor.activeLayer, name, params };
}

/** Applies an effect to the active layer, inside the selection, as one
 *  undoable step (spec §11): commits the `region` the job worker computed
 *  for it. Returns `false` when the selection is empty and nothing changed. */
export function applyEffectRegion(name: string, params: number[], region: Uint8Array): boolean {
  if (editor.handle === null) {
    return false;
  }
  const changed = core.applyEffectRegion(editor.handle, editor.activeLayer, name, params, region);
  editor.previewComposite = null;
  syncInfo();
  return changed;
}

/** Shows a computed effect preview over the canvas without mutating the
 *  document (spec §11 live preview). Call from an open effect dialog. */
export function showEffectPreview(composite: Uint8ClampedArray<ArrayBuffer>): void {
  if (editor.handle === null) {
    return;
  }
  editor.previewComposite = composite;
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
export function drawShape(a: [number, number], b: [number, number], shape: ShapeKind): void {
  if (editor.handle === null) {
    return;
  }
  const cmd: DrawShapeCommand = {
    type: 'draw_shape',
    layer: editor.activeLayer,
    shape: shape,
    mode: tool.shapeMode,
    stroke_width: tool.strokeWidth,
    stroke_color: colorWithOpacity(tool.foreground, tool.foregroundAlpha),
    fill_color: colorWithOpacity(tool.background, tool.backgroundAlpha),
    anti_alias: tool.antiAlias,
    dash: tool.shapeDash,
    blend: tool.blend,
  };
  if (shape === 'star') {
    cmd.star_points = tool.starPoints;
    cmd.inner_ratio = tool.starInner / 100;
  }
  if (shape === 'line') {
    cmd.start_cap = tool.lineStartCap;
    cmd.end_cap = tool.lineEndCap;
  }
  if (shape === 'polygon' || shape === 'star') {
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
  const before = editor.undoDepth;
  core.applyCommand(editor.handle, cmd);
  syncInfo();
  notePainted(before, shapeColors(shape));
}

/** The foreground color as RGBA bytes, recorded as painted with when the
 *  document gained a step since `depthBefore` (color-picker AC-24). */
export function noteTextPainted(depthBefore: number): void {
  notePainted(depthBefore, [usedColor(false)]);
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

/** Creates a blank document as a new tab and makes it the active one
 *  (docs/specs/document-tabs.md AC-02). */
export async function newDocument(width: number, height: number): Promise<ImageTab> {
  await initCore();
  const tab = openTab(core.createDocument(width, height), blankFile());
  tab.doc.origin = 'new';
  syncInfo();
  markSaved();
  return tab;
}

/** Opens already-read bytes: an OpenRaster project with all its layers, a
 * PSD with its layers and groups (its import notice goes to the status
 * bar), or a flat image as a single-layer document. `name` is the file name Save
 * writes back to; `mime` is a decoder hint. `target` carries the file's FSA
 * handle / Tauri path so Save writes it in place, and optionally the format
 * Save writes (a recovered snapshot is OpenRaster bytes of, say, a PNG);
 * by default the format detected from the bytes. */
export async function openBytes(
  bytes: Uint8Array,
  name: string,
  mime: string,
  target: Pick<SaveTarget, 'handle' | 'path'> & {
    format?: DocFormat | null;
    origin?: 'paste';
  } = {
    handle: null,
    path: null,
  },
): Promise<ImageTab> {
  await initCore();
  const sniffed = sniffFormat(bytes);
  // A corrupt file throws here, before a tab exists: no tab opens and the
  // active tab stays (document-tabs edge cases).
  let handle: number;
  let notice: string | null = null;
  if (sniffed === 'ora') {
    handle = core.openProject(bytes);
  } else if (sniffed === 'psd') {
    const opened = takePsd(bytes);
    handle = opened.handle;
    notice = psdNotice(opened.notices);
  } else {
    // The file name is the hint for TGA, which has no signature and often
    // no MIME type (docs/specs/image-formats-more.md AC-04).
    handle = core.openImage(bytes, formatFromName(name) === 'tga' ? name : mime || 'image/png');
  }
  const format =
    target.format !== undefined ? target.format : docFormat(sniffed ?? formatFromName(name));
  const tab = openTab(handle, openedFile(name, format, target));
  tab.doc.origin = target.origin ?? 'open';
  syncInfo();
  markSaved();
  editor.notice = notice;
  return tab;
}

/** Opens an encoded image file as a new single-layer document. */
export async function openFile(file: File): Promise<ImageTab> {
  return openBytes(new Uint8Array(await file.arrayBuffer()), file.name, file.type);
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
  const before = editor.undoDepth;
  const cmd: PencilStrokeCommand = {
    type: 'pencil_stroke',
    layer: editor.activeLayer,
    size: tool.size,
    color: activeColor(useBackground),
    opacity: opacity01(),
    shape: tool.shape,
    hardness: Math.min(1, Math.max(0, tool.hardness / 100)),
    anti_alias: tool.antiAlias,
    blend: tool.blend,
    points,
    ...penFields(pressure, azimuth),
    stroke_id: strokeId,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
  notePainted(before, [usedColor(useBackground)]);
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
    anti_alias: tool.antiAlias,
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
    anti_alias: tool.antiAlias,
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
    anti_alias: tool.antiAlias,
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
  const before = editor.undoDepth;
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
    blend: tool.blend,
  };
  core.applyCommand(editor.handle, cmd);
  syncInfo();
  notePainted(before, [usedColor(useBackground)]);
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

/** Fills the selected pixels of the active layer with the foreground color
 *  (Edit ▸ Fill Selection); the whole layer without a selection. The core
 *  adds no undo step when nothing changes (image-edit-extras AC-06). */
export function fillSelection(): void {
  if (editor.handle === null) {
    return;
  }
  const before = editor.undoDepth;
  core.applyCommand(editor.handle, {
    type: 'fill_selection',
    layer: editor.activeLayer,
    color: activeColor(false),
  });
  syncInfo();
  notePainted(before, [usedColor(false)]);
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
    blend: tool.blend,
  };
  const before = editor.undoDepth;
  core.applyCommand(editor.handle, cmd);
  syncInfo();
  notePainted(before, gradientColors());
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
  const [r, g, b, a] = rgba;
  if (r === undefined || g === undefined || b === undefined) {
    return; // off-canvas
  }
  // Hex and alpha both (color-picker AC-17, AC-22).
  const hex = rgbToHex(r, g, b);
  const alpha = a ?? 255;
  if (toBackground) {
    tool.background = hex;
    tool.backgroundAlpha = alpha;
  } else {
    tool.foreground = hex;
    tool.foregroundAlpha = alpha;
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

/** Undoes the last command. During a live-object handle drag it only drops
 *  that drag (docs/specs/image-live-objects-ui.md AC-11). */
export function undo(): void {
  if (gestureControl.cancelLiveDrag()) return;
  gestureControl.historyMoved();
  if (editor.handle !== null && core.undo(editor.handle)) {
    syncInfo();
  }
}

/** Undoes (`delta < 0`) or redoes `|delta|` steps at once for the History
 *  panel (history-panel AC-05): one core call, one redraw (AC-13). A failed
 *  step stops the jump and shows a notice (AC-12). */
export function jumpHistory(delta: number): void {
  if (editor.handle === null || delta === 0) return;
  // What Undo / Redo do first: drop a live-object handle drag, and reset a
  // live text's caret, selection and typing burst (image-live-text AC-13;
  // history-panel AC-11).
  if (gestureControl.cancelLiveDrag()) return;
  gestureControl.historyMoved();
  try {
    core.jumpHistory(editor.handle, delta);
  } catch (err) {
    editor.notice = t('historyPanel.jumpFailed', {
      error: err instanceof Error ? err.message : String(err),
    });
  }
  syncInfo();
}

/** Redoes the last undone command. */
export function redo(): void {
  gestureControl.historyMoved();
  if (editor.handle !== null && core.redo(editor.handle)) {
    syncInfo();
  }
}

export type { ExportFormat };

// File ▸ Export ▸ SVG… (docs/specs/vectorize-dialog.md). The vectorizer
// lives while the dialog is open, so re-tracing reuses its caches. It is
// tied to the document handle it was made from: a document replaced while
// the dialog is open (native File ▸ New / Open) gets a fresh one.
let svgVectorizer: { handle: number; tracer: Vectorizer } | null = null;

/** Traces the composite with the dialog's mode and option vector. */
export function traceSvg(mode: number, params: number[]): SvgTrace {
  if (editor.handle === null) return { error: 'no document' };
  try {
    if (svgVectorizer?.handle !== editor.handle) {
      closeSvgExport();
      svgVectorizer = { handle: editor.handle, tracer: core.vectorizer(editor.handle) };
    }
    const r = svgVectorizer.tracer.trace(mode, new Float64Array(params));
    try {
      return { svg: r.svg, layers: r.layers, outlines: r.outlines, threshold: r.threshold };
    } finally {
      r.free();
    }
  } catch (err) {
    return { error: err instanceof Error ? err.message : String(err) };
  }
}

/** Frees the dialog's vectorizer (on close, and before the dialog opens). */
export function closeSvgExport(): void {
  svgVectorizer?.tracer.free();
  svgVectorizer = null;
}

/** Saves `svg` as `<stem>.svg`; `false` when the user cancelled. */
export async function exportSvg(svg: string): Promise<boolean> {
  if (editor.handle === null) return false;
  return saveExport(new Uint8Array(new TextEncoder().encode(svg)), `${fileStem()}.svg`, 'svg');
}

/** Exports the composite in `format` through the shared file layer
 * (FSA save picker / Tauri dialog / download). Resolves `false` when the
 * user cancelled the picker.
 *
 * `quality` (1–100) applies to JPEG only; PNG and WebP are lossless. */
export async function exportImage(format: ExportFormat = 'png', quality = 90): Promise<boolean> {
  if (editor.handle === null) {
    return false;
  }
  // Encoded off the main thread from the document as it is now
  // (performance-background-jobs).
  const bytes = await encodeInBackground(editor.handle, format, { level: 6, quality });
  const name = `${fileStem()}.${format === 'jpeg' ? 'jpg' : format}`;
  return saveExport(new Uint8Array(bytes), name, format);
}
