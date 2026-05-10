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
  // The wasm `ComposeFrame::pixels` getter already hands back a fresh
  // `Uint8Array` copy, so we wrap it in a `Uint8ClampedArray` view
  // over the same buffer rather than re-copying with the
  // `Uint8ClampedArray(source)` constructor.
  const pixels = frame.pixels;
  // `pixels.buffer` is typed as `ArrayBufferLike` (could be a
  // `SharedArrayBuffer` in principle); the wasm-pack runtime always
  // hands back a plain `ArrayBuffer`, so the cast is safe and lets
  // `ImageData` accept the view.
  const buffer = pixels.buffer as ArrayBuffer;
  const clamped = new Uint8ClampedArray(buffer, pixels.byteOffset, pixels.byteLength);
  const image = new ImageData(clamped, width, height);
  ctx.putImageData(image, 0, 0);
}
