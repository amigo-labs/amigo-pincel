// The drag preview of Move Layer (docs/specs/image-move-layer-preview.md
// AC-01 – AC-03): the composites below, of and above the moved layer,
// handed over once by the core when the drag's first frame needs them,
// drawn every frame with the moved one shifted by the drag's offset. The
// browser blends them in sRGB, so semi-transparent pixels are approximate
// until release; opaque pixels match the core. Render-only: the canvas is
// never read (AGENTS.md §9).
import type { MovePreviewData } from '../core/wasm';
import { checkerPattern, imageCanvas, prepareViewport, type ViewTransform } from './canvas2d';

/** One drag's preview images, once fetched. */
export interface MovePreview {
  /** Asks the core for the three composites; `null` when it cannot. */
  readonly fetch: () => MovePreviewData | null;
  images: {
    readonly below: HTMLCanvasElement;
    readonly moved: HTMLCanvasElement;
    readonly above: HTMLCanvasElement;
    /** The images' size: the canvas, or less for a large canvas. */
    readonly size: readonly [number, number];
  } | null;
}

/** A preview whose images are fetched on its first frame. */
export function movePreview(fetch: MovePreview['fetch']): MovePreview {
  return { fetch, images: null };
}

/**
 * Draws `p` into the viewport-sized `canvas` of a `docW × docH` document at
 * `view`, the moved layer shifted by `offset` document pixels and clipped
 * to the document. Returns `false` when the core could not hand over the
 * images, so the caller draws the document instead.
 */
export function drawMovePreview(
  canvas: HTMLCanvasElement,
  docW: number,
  docH: number,
  view: ViewTransform,
  p: MovePreview,
  offset: readonly [number, number],
): boolean {
  if (p.images === null) {
    const got = p.fetch();
    if (got === null) return false;
    const [w, h] = got.size;
    const below = imageCanvas(got.below, w, h);
    const moved = imageCanvas(got.moved, w, h);
    const above = imageCanvas(got.above, w, h);
    if (!below || !moved || !above) return false;
    p.images = { below, moved, above, size: [w, h] };
  }
  const prepared = prepareViewport(canvas, docW, docH, view);
  if (!prepared) return true;
  const { ctx, region } = prepared;
  if (!region) return true;
  const { x0, y0, rw, rh, dpr } = region;
  const pattern = checkerPattern(ctx);
  if (pattern) {
    ctx.fillStyle = pattern;
    ctx.fillRect(
      (view.panX + x0 * view.zoom) * dpr,
      (view.panY + y0 * view.zoom) * dpr,
      rw * view.zoom * dpr,
      rh * view.zoom * dpr,
    );
  }
  const { below, moved, above, size } = p.images;
  const scale = view.zoom * dpr;
  ctx.save();
  ctx.setTransform(scale, 0, 0, scale, view.panX * dpr, view.panY * dpr);
  ctx.beginPath();
  ctx.rect(0, 0, docW, docH);
  ctx.clip();
  // Zoomed in on full-size images the pixels stay sharp, as the canvas
  // draws them outside a drag; scaled down they are smoothed.
  ctx.imageSmoothingEnabled = scale * (size[0] / docW) < 1;
  ctx.drawImage(below, 0, 0, docW, docH);
  ctx.drawImage(moved, offset[0], offset[1], docW, docH);
  ctx.drawImage(above, 0, 0, docW, docH);
  ctx.restore();
  return true;
}
