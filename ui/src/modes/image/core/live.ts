// Live objects in the editor (docs/specs/image-live-objects-ui.md): begin,
// edit, commit and cancel through the core, which keeps the object and its
// undo steps (docs/specs/image-live-objects.md). The handles come from the
// core's report (`live.object`), refreshed by every `syncInfo`.
import { core, type EditLiveCommand, type LiveCommand, type ShapeKind } from './wasm';
import {
  activeColor,
  colorWithOpacity,
  gradientColors,
  notePainted,
  noteTextPainted,
  opacity01,
  shapeColors,
  syncInfo,
} from './controller';
import { editor, live, tool, type ToolKind } from '../stores/editor.svelte';
import { gestureControl } from '../tools/gesture-control';
import type { UsedFace } from '../../../lib/fonts/text-font.svelte';

/** Sends a live command; `false` when the core refused it (no object live
 *  any more, AC-19): the editor then shows the core's state, silently. */
function send(cmd: LiveCommand): boolean {
  if (editor.handle === null) return false;
  try {
    core.applyCommand(editor.handle, cmd);
    return true;
  } catch {
    return false;
  } finally {
    syncInfo();
  }
}

/** Begins a live Line or box shape between `a` and `b` with the current
 *  options and colors (outline = foreground, fill = background). The drag
 *  that creates it counts as painting with the colors it uses (color-picker
 *  AC-24, Decisions 2026-11-06). */
export function beginShape(a: [number, number], b: [number, number], shape: ShapeKind): void {
  const before = editor.undoDepth;
  send({
    type: 'begin_live_shape',
    layer: editor.activeLayer,
    shape,
    points: [a, b],
    corner_radius: tool.cornerRadius,
    sides: tool.shapeSides,
    star_points: tool.starPoints,
    inner_ratio: tool.starInner / 100,
    mode: tool.shapeMode,
    stroke_width: tool.strokeWidth,
    stroke_color: colorWithOpacity(tool.foreground, tool.foregroundAlpha),
    fill_color: colorWithOpacity(tool.background, tool.backgroundAlpha),
    anti_alias: tool.antiAlias,
    dash: tool.shapeDash,
    blend: tool.blend,
    ...(shape === 'line' ? { start_cap: tool.lineStartCap, end_cap: tool.lineEndCap } : {}),
  });
  notePainted(before, shapeColors(shape));
}

/** Begins a live gradient along `a` → `b` (start = foreground, end =
 *  background, as `drawGradient` paints it); like a shape, it counts as
 *  painted with on creation. */
export function beginGradient(a: [number, number], b: [number, number]): void {
  const before = editor.undoDepth;
  send({
    type: 'begin_live_gradient',
    layer: editor.activeLayer,
    kind: tool.gradientKind,
    channel: tool.gradientChannel,
    x0: a[0],
    y0: a[1],
    x1: b[0],
    y1: b[1],
    from: activeColor(false),
    to: activeColor(true),
    opacity: opacity01(),
    blend: tool.blend,
  });
  notePainted(before, gradientColors());
}

/** Begins a live text `text` whose box has its top-left at `at`, with the
 *  Text tool's options and the foreground color (docs/specs/image-live-text.md
 *  AC-01). `true` when a live text began. */
export function beginText(at: [number, number], text: string, face: UsedFace): boolean {
  const before = editor.undoDepth;
  const ok = send({
    type: 'begin_live_text',
    layer: editor.activeLayer,
    font_id: face.id,
    text,
    x: at[0],
    y: at[1],
    size: tool.fontSize,
    color: colorWithOpacity(tool.foreground, tool.foregroundAlpha),
    // The face carries the real Bold / Italic; only what it lacks is
    // synthesized (docs/specs/system-fonts.md AC-07).
    bold: face.synthBold,
    italic: face.synthItalic,
    anti_alias: tool.antiAlias,
    align: tool.textAlign,
    underline: tool.textUnderline,
    strikethrough: tool.textStrikethrough,
    blend: tool.blend,
  });
  noteTextPainted(before);
  return ok && live.object?.type === 'text';
}

/** Edits the live object; `false` when the core refused (AC-19). */
export function editLive(fields: Omit<EditLiveCommand, 'type'>): boolean {
  if (live.object === null) return false;
  return send({ type: 'edit_live', ...fields });
}

/** Commits the live object, if any (AC-09); an empty text box just
 *  closes (image-live-text AC-01). */
export function commitLive(): void {
  gestureControl.closeTextBox();
  if (live.object !== null) send({ type: 'commit_live' });
}

/** Cancels the live object, if any (AC-11). `true` when one was live. */
export function cancelLive(): boolean {
  if (live.object === null) return false;
  send({ type: 'cancel_live' });
  return true;
}

// The tab store and the save flow commit through the gesture hooks, which
// avoid an import cycle with the controller.
gestureControl.commitLive = () => commitLive();

/** Drops the step of the handle drag `gesture`, without a redo step
 *  (AC-11: Escape or Undo during a drag). */
export function cancelLiveGesture(gesture: number): void {
  send({ type: 'cancel_live_gesture', gesture });
}

/** The rail tool that draws the live object. */
export function liveToolOf(): ToolKind | null {
  const obj = live.object;
  if (obj === null) return null;
  if (obj.type === 'gradient') return 'gradient';
  if (obj.type === 'text') return 'text';
  switch (obj.shape) {
    case 'line':
      return 'line';
    case 'rounded_rectangle':
      return 'rounded_rect';
    case 'ellipse':
      return 'ellipse';
    case 'polygon':
      return 'polygon';
    case 'triangle':
    case 'diamond':
    case 'star':
    case 'block_arrow':
    case 'heart':
    case 'callout':
      return obj.shape;
    default:
      return 'rectangle';
  }
}
