import type { ComposeFrame } from '../core';

/** Which low-level surface a [`CanvasRenderer`] drives. */
export type RenderBackend = 'webgpu' | 'canvas2d';

/**
 * Backend-agnostic blit surface for the composed sprite (spec §4.4).
 *
 * The base layer shows the composed RGBA only — transient overlays
 * (selection marquee, drag previews, tile grid, slice accents) live on a
 * separate stacked Canvas2D layer so the base can be WebGPU without
 * having to rasterize 1-pixel UI furniture on the GPU.
 *
 * `pincel-core` composes at 1× (`compose(frame, 1)`); the display zoom is
 * pure CSS upscaling (`image-rendering: pixelated`). A renderer therefore
 * only ever uploads a sprite-sized buffer and lets the element scale —
 * there is no GPU-side zoom to manage.
 */
export interface CanvasRenderer {
  /** Which backend this instance drives — surfaced in the footer. */
  readonly backend: RenderBackend;

  /**
   * Paint a full composed frame, resizing the backing store to
   * `frame.width × frame.height` first. The frame is read synchronously;
   * the caller is free to `frame.free()` once this returns.
   */
  draw(frame: ComposeFrame): void;

  /**
   * Paint a sub-region (the M12.4 dirty fast path) at the frame's
   * `dirtyX` / `dirtyY` origin without resizing the backing store. Empty
   * frames are a no-op. The caller routes here only when no overlay is
   * live (`canRecomposeDirty()`), so the base layer never strands a
   * stale overlay.
   */
  drawDirty(frame: ComposeFrame): void;

  /** Release any GPU / context resources. Idempotent. */
  destroy(): void;
}
