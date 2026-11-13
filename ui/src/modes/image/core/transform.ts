// The Move tools' live transform in the editor
// (docs/specs/image-transform-handles.md): the core keeps the transform and
// its undo steps; this module sends the box a finished drag produced and
// reads the core's report (`live.transform`, refreshed by every
// `syncInfo`). During a drag the editor previews the box itself
// (`render/transform-preview.ts`), from what `previewLift` fetches once.
import {
  core,
  type SelectionTransformCommand,
  type TransformFields,
  type TransformPreviewData,
} from './wasm';
import { syncInfo } from './controller';
import { countCall } from '../../../lib/diagnostics';
import { editor, live, tool } from '../stores/editor.svelte';
import { gestureControl } from '../tools/gesture-control';
import { boxOfBounds, type BoxState } from '../tools/transform-box';

function send(cmd: SelectionTransformCommand): boolean {
  if (editor.handle === null) return false;
  // e2e specs pin how often the core applies a transform (AC-23).
  if (cmd.type === 'begin_transform' || cmd.type === 'edit_transform') {
    countCall('image.applyTransform');
  }
  try {
    core.applyCommand(editor.handle, cmd);
    return true;
  } catch {
    return false;
  } finally {
    syncInfo();
  }
}

/** The box the handles show: a drag's, else the live transform's, else
 *  the selection's bounds; `null` without a selection that selects a pixel
 *  (AC-01, AC-17). */
export function transformBox(): BoxState | null {
  const drag = live.transformDrag;
  if (drag !== null) return drag.box;
  const t = live.transform;
  if (t !== null) {
    return {
      center: [t.center[0], t.center[1]],
      size: [t.size[0], t.size[1]],
      angle: t.angle,
      pivot: [t.pivot[0], t.pivot[1]],
    };
  }
  const b = live.selectionBox;
  return b === null ? null : boxOfBounds(b);
}

function fields(b: BoxState, gesture: number | null, quality: number): TransformFields {
  return {
    center_x: b.center[0],
    center_y: b.center[1],
    width: b.size[0],
    height: b.size[1],
    angle: b.angle,
    pivot_x: b.pivot[0],
    pivot_y: b.pivot[1],
    quality,
    gesture,
  };
}

/**
 * Shows the selection's pixels (`pixels`, Move Selected Pixels) or outline
 * (Move Selection) in box `b`: begins the transform on the active layer
 * when none is live, else changes it. A drag sends its final box once,
 * when it ends, as one undo step (AC-07, AC-23).
 */
export function applyTransform(b: BoxState, pixels: boolean, gesture: number | null): boolean {
  const f = fields(b, gesture, tool.moveQuality);
  return live.transform === null
    ? send({ type: 'begin_transform', layer: editor.activeLayer, pixels, ...f })
    : send({ type: 'edit_transform', ...f });
}

/**
 * The lift a drag preview draws from (AC-20, AC-23): asks the core for a
 * view rect with the selection's content of the active layer — or the
 * live transform's — lifted out, and the patch. Changes nothing in the
 * document; `null` when the core cannot.
 */
export function previewLift(
  pixels: boolean,
): (view: readonly number[]) => TransformPreviewData | null {
  const handle = editor.handle;
  const layer = live.transform?.layer ?? editor.activeLayer;
  return (view) => {
    if (handle === null) return null;
    countCall('image.transformPreview');
    try {
      return core.transformPreview(handle, layer, pixels, view);
    } catch {
      return null;
    }
  };
}

/** The quality option (AC-12): a live transform is re-applied with it. */
export function setTransformQuality(quality: number): void {
  tool.moveQuality = quality;
  const b = transformBox();
  if (live.transform !== null && b !== null) {
    send({ type: 'edit_transform', ...fields(b, null, quality) });
  }
}

/** Commits the live transform, if any (AC-08, AC-09). */
export function commitTransform(): void {
  if (live.transform !== null) send({ type: 'commit_transform' });
}

/** Cancels the live transform, if any (AC-10). `true` when one was live. */
export function cancelTransform(): boolean {
  if (live.transform === null) return false;
  send({ type: 'cancel_transform' });
  return true;
}

gestureControl.commitTransform = () => commitTransform();
