// The live preview of a Move Layer drag (docs/specs/image-move-layer-preview.md):
// while the pointer is held, the editor draws the moved layer at the drag's
// offset between the layers below and above it — three composites the core
// hands over once, when the first frame needs them — and the core moves the
// layer once, on release. A drag is tied to the document, its revision, the
// active layer and the tool it began with: when any of them changes (Undo,
// a menu command, a tab or layer switch, another tool) the drag is dropped
// and nothing is moved (AC-06).
import { countCall } from '../../../lib/diagnostics';
import { core } from '../core/wasm';
import { movePreview, type MovePreview } from '../render/move-preview';
import { editor, live, tool, type ToolKind } from '../stores/editor.svelte';

interface MoveDrag {
  readonly handle: number;
  readonly revision: number;
  readonly layer: number;
  readonly kind: ToolKind;
  readonly preview: MovePreview;
}

let drag: MoveDrag | null = null;

/** Starts the preview of a drag with the Move tool `kind`. */
export function beginMoveDrag(kind: ToolKind): void {
  const handle = editor.handle;
  if (handle === null) return;
  const layer = editor.activeLayer;
  drag = {
    handle,
    revision: editor.revision,
    layer,
    kind,
    preview: movePreview(() => {
      // e2e specs pin that the core hands the images over once (AC-04).
      countCall('image.movePreview');
      try {
        return core.movePreview(handle, layer);
      } catch {
        return null;
      }
    }),
  };
  live.moveOffset = [0, 0];
}

/** Shows the drag at the whole-pixel offset `(dx, dy)`. */
export function setMoveOffset(dx: number, dy: number): void {
  const at = live.moveOffset;
  if (drag === null || (at !== null && at[0] === dx && at[1] === dy)) return;
  live.moveOffset = [dx, dy];
}

/** `true` while a drag is in flight and still matches the document, the
 *  active layer and the tool it began with; a drag that no longer does is
 *  dropped here. Reads reactive state, so a caller in an effect reruns
 *  when any of it changes. */
export function moveDragCurrent(): boolean {
  if (drag === null) return false;
  const same =
    drag.handle === editor.handle &&
    drag.revision === editor.revision &&
    drag.layer === editor.activeLayer &&
    drag.kind === tool.kind;
  if (!same) endMoveDrag();
  return same;
}

/** The preview images of the drag in flight, `null` without one. */
export function movePreviewOf(): MovePreview | null {
  return drag?.preview ?? null;
}

/** Ends the drag's preview; `true` when one was in flight. */
export function endMoveDrag(): boolean {
  const was = drag !== null;
  drag = null;
  live.moveOffset = null;
  return was;
}
