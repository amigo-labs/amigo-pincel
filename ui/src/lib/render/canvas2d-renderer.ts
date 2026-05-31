import type { ComposeFrame } from '../core';
import { blitDirtyFrame, blitFrame } from './canvas2d';
import type { CanvasRenderer } from './types';

/**
 * Canvas2D implementation of [`CanvasRenderer`] — the universal fallback
 * (spec §4.4). Thin wrapper over the existing [`blitFrame`] /
 * [`blitDirtyFrame`] helpers so the put-`ImageData` path is shared with
 * the pre-M12.5 code.
 *
 * Holds no state beyond the target canvas; `destroy()` is a no-op because
 * a 2D context owns nothing that needs explicit release.
 */
export class Canvas2DRenderer implements CanvasRenderer {
  readonly backend = 'canvas2d' as const;
  readonly #canvas: HTMLCanvasElement;

  constructor(canvas: HTMLCanvasElement) {
    this.#canvas = canvas;
  }

  draw(frame: ComposeFrame): void {
    blitFrame(this.#canvas, frame);
  }

  drawDirty(frame: ComposeFrame): void {
    blitDirtyFrame(this.#canvas, frame);
  }

  destroy(): void {
    // A 2D context holds no resources that need explicit teardown.
  }
}
