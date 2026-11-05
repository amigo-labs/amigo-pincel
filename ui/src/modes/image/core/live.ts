// Live objects in the editor (docs/specs/image-live-objects-ui.md): begin,
// edit, commit and cancel through the core, which keeps the object and its
// undo steps (docs/specs/image-live-objects.md). The handles come from the
// core's report (`live.object`), refreshed by every `syncInfo`.
import { core, type EditLiveCommand, type LiveCommand, type ShapeKind } from './wasm';
import { activeColor, colorWithOpacity, opacity01, syncInfo } from './controller';
import { editor, live, tool, type ToolKind } from '../stores/editor.svelte';

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
 *  options and colors (outline = foreground, fill = background). */
export function beginShape(a: [number, number], b: [number, number], shape: ShapeKind): void {
  send({
    type: 'begin_live_shape',
    layer: editor.activeLayer,
    shape,
    points: [a, b],
    corner_radius: tool.cornerRadius,
    sides: tool.shapeSides,
    mode: tool.shapeMode,
    stroke_width: tool.strokeWidth,
    stroke_color: colorWithOpacity(tool.foreground),
    fill_color: colorWithOpacity(tool.background),
    anti_alias: tool.shapeAntiAlias,
    dash: tool.shapeDash,
  });
}

/** Begins a live gradient along `a` → `b` (start = foreground, end =
 *  background, as `drawGradient` paints it). */
export function beginGradient(a: [number, number], b: [number, number]): void {
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
  });
}

/** Edits the live object; `false` when the core refused (AC-19). */
export function editLive(fields: Omit<EditLiveCommand, 'type'>): boolean {
  if (live.object === null) return false;
  return send({ type: 'edit_live', ...fields });
}

/** Commits the live object, if any (AC-09). */
export function commitLive(): void {
  if (live.object !== null) send({ type: 'commit_live' });
}

/** Cancels the live object, if any (AC-11). `true` when one was live. */
export function cancelLive(): boolean {
  if (live.object === null) return false;
  send({ type: 'cancel_live' });
  return true;
}

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
  switch (obj.shape) {
    case 'line':
      return 'line';
    case 'rounded_rectangle':
      return 'rounded_rect';
    case 'ellipse':
      return 'ellipse';
    case 'polygon':
      return 'polygon';
    default:
      return 'rectangle';
  }
}
