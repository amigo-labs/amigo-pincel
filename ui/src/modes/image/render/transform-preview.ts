// The drag preview of the Move tools' transform box
// (docs/specs/image-transform-handles.md AC-20, AC-23). While a box drag is
// in flight the core does no work per pointer move: when the drag starts it
// hands over, once, the lifted patch and the visible composite with the
// hole the lift leaves (`TransformPreviewData`), and every frame draws that
// composite and the patch through the box's affine map with a canvas
// transform. Only a view change asks the core again. Render-only: the
// canvas is never read (CLAUDE.md §9).
import type { TransformPreviewData } from '../core/wasm';
import { patchMatrix, type BoxState } from '../tools/transform-box';
import { imageCanvas, prepareViewport, presentRegion, type ViewTransform } from './canvas2d';

/** One drag's preview: what the core handed over, and the box to show. */
export interface DragPreview {
  /** Asks the core for the view rect `[x, y, w, h, outW, outH]` with the
   *  content lifted out (the patch comes along); `null` when it cannot. */
  readonly lift: (view: readonly number[]) => TransformPreviewData | null;
  /** The box the drag shows now. */
  box: BoxState;
  /** Resampling quality: 0 Nearest, 1 Bilinear, 2 Bicubic. */
  quality: number;
  /** The lifted patch, once the first frame fetched it. */
  patch: {
    readonly canvas: HTMLCanvasElement;
    readonly bounds: readonly [number, number, number, number];
    readonly opacity: number;
  } | null;
  /** The composite with the hole for the view rect `key`. */
  hole: { readonly key: string; readonly canvas: HTMLCanvasElement } | null;
}

/** A preview of the drag that starts at `box`; nothing is fetched yet. */
export function dragPreview(
  lift: DragPreview['lift'],
  box: BoxState,
  quality: number,
): DragPreview {
  return { lift, box, quality, patch: null, hole: null };
}

/**
 * Draws `p` into the viewport-sized `canvas` of a `docW × docH` document at
 * `view`: the composite with the hole, then the patch at the box, clipped
 * to the document. Returns `false` when the core could not hand over the
 * view, so the caller draws the document instead.
 */
export function drawDragPreview(
  canvas: HTMLCanvasElement,
  docW: number,
  docH: number,
  view: ViewTransform,
  p: DragPreview,
): boolean {
  const prepared = prepareViewport(canvas, docW, docH, view);
  if (!prepared) return true;
  const { ctx, region } = prepared;
  if (!region) return true;
  const { x0, y0, rw, rh, ow, oh, dpr } = region;
  const key = [x0, y0, rw, rh, ow, oh].join(':');
  if (p.hole?.key !== key) {
    const got = p.lift([x0, y0, rw, rh, ow, oh]);
    const hole = got ? imageCanvas(got.hole, ow, oh, p.hole?.canvas) : null;
    if (!got || !hole) return false;
    p.hole = { key, canvas: hole };
    if (p.patch === null && got.patch !== null) {
      const patch = imageCanvas(got.patch, got.patchSize[0], got.patchSize[1]);
      if (patch) p.patch = { canvas: patch, bounds: got.bounds, opacity: got.opacity };
    }
  }
  presentRegion(ctx, region, view, p.hole.canvas);
  const patch = p.patch;
  if (patch === null || patch.opacity <= 0) return true;
  const [, , bw, bh] = patch.bounds;
  const [a, b, c, d, e, f] = patchMatrix(p.box, patch.bounds);
  const scale = view.zoom * dpr;
  ctx.save();
  ctx.setTransform(scale, 0, 0, scale, view.panX * dpr, view.panY * dpr);
  ctx.beginPath();
  ctx.rect(0, 0, docW, docH);
  ctx.clip();
  // Canvas order: (a, b, c, d, e, f) = (x←x, y←x, x←y, y←y, dx, dy).
  ctx.transform(a, d, b, e, c, f);
  ctx.globalAlpha = patch.opacity;
  // A plain move at 1:1 or closer keeps pixels sharp, as the core's copy
  // of whole pixels does; Nearest never smooths.
  const plain = p.box.angle === 0 && p.box.size[0] === bw && p.box.size[1] === bh;
  ctx.imageSmoothingEnabled = p.quality !== 0 && !(plain && view.zoom >= 1);
  ctx.imageSmoothingQuality = p.quality === 2 ? 'high' : 'low';
  ctx.drawImage(patch.canvas, 0, 0, bw, bh);
  ctx.restore();
  return true;
}
