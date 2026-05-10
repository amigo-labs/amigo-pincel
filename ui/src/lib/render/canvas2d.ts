import type { ComposeFrame } from '../core';

/**
 * Paint a [`ComposeFrame`] into the given 2D context at the origin.
 *
 * The frame is the final RGBA8 buffer from `Document.compose()` —
 * width / height already reflect any zoom applied in the wasm layer.
 * The canvas backing store is resized to match before painting.
 *
 * Spec §9.2 calls for a WebGPU adapter as the long-term renderer;
 * Canvas2D is the M6 fallback so the UI works on any browser today.
 * The WebGPU path lands in M12.
 */
export function blitFrame(canvas: HTMLCanvasElement, frame: ComposeFrame): void {
  const { width, height } = frame;
  if (canvas.width !== width) canvas.width = width;
  if (canvas.height !== height) canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('failed to acquire 2D context');
  // `pixels` is non-premultiplied RGBA8; ImageData expects the same.
  const pixels = frame.pixels;
  const image = new ImageData(new Uint8ClampedArray(pixels), width, height);
  ctx.putImageData(image, 0, 0);
}
