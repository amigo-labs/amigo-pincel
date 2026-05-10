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

/**
 * Overlay a 1-pixel-wide Bresenham line on top of the current canvas
 * contents. Coordinates are in the canvas's pixel space (i.e. sprite
 * coords for the M6 single-frame, 1× compose path).
 *
 * Used by the Line tool to preview the rasterized pixel set during a
 * press-drag before `Document.applyLine` commits the actual command on
 * release. The preview matches the committed pixels exactly because it
 * uses the same Bresenham algorithm as `pincel_core::DrawLine`.
 */
export function paintLinePreview(
  canvas: HTMLCanvasElement,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  color: string,
): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.fillStyle = color;
  for (const [x, y] of bresenham(x0, y0, x1, y1)) {
    if (x < 0 || y < 0 || x >= canvas.width || y >= canvas.height) continue;
    ctx.fillRect(x, y, 1, 1);
  }
}

/**
 * Overlay an axis-aligned rectangle (outline or filled) on top of the
 * current canvas contents. Coordinates are in the canvas's pixel space
 * (i.e. sprite coords for the M6 single-frame, 1× compose path).
 *
 * Endpoint order is irrelevant — the helper normalizes to min / max
 * corners before rasterizing, matching the Rust `DrawRectangle`
 * behavior so the preview is pixel-exact with what `applyRectangle`
 * commits on release.
 */
export function paintRectanglePreview(
  canvas: HTMLCanvasElement,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  color: string,
  fill: boolean,
): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.fillStyle = color;
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  // Pointer-capture drags can place the live endpoint far outside the
  // canvas. Clip the iteration ranges to the canvas up front and emit
  // each edge / fill as a single span `fillRect` so the preview stays
  // O(1) regardless of how far the cursor strays.
  const loX = Math.max(minX, 0);
  const hiX = Math.min(maxX, canvas.width - 1);
  const xInView = loX <= hiX;
  if (fill) {
    const loY = Math.max(minY, 0);
    const hiY = Math.min(maxY, canvas.height - 1);
    if (xInView && loY <= hiY) {
      ctx.fillRect(loX, loY, hiX - loX + 1, hiY - loY + 1);
    }
    return;
  }
  const topInView = xInView && minY >= 0 && minY < canvas.height;
  const bottomDistinct = maxY > minY;
  const bottomInView =
    bottomDistinct && xInView && maxY >= 0 && maxY < canvas.height;
  if (topInView) {
    ctx.fillRect(loX, minY, hiX - loX + 1, 1);
  }
  if (bottomInView) {
    ctx.fillRect(loX, maxY, hiX - loX + 1, 1);
  }
  if (bottomDistinct && maxY > minY + 1) {
    const sideLoY = Math.max(minY + 1, 0);
    const sideHiY = Math.min(maxY - 1, canvas.height - 1);
    if (sideLoY <= sideHiY) {
      if (minX >= 0 && minX < canvas.width) {
        ctx.fillRect(minX, sideLoY, 1, sideHiY - sideLoY + 1);
      }
      if (maxX > minX && maxX >= 0 && maxX < canvas.width) {
        ctx.fillRect(maxX, sideLoY, 1, sideHiY - sideLoY + 1);
      }
    }
  }
}

/**
 * Overlay the ellipse inscribed in the bbox of two sprite-space corners
 * (outline or filled) on top of the current canvas contents.
 * Coordinates are in canvas pixel space (i.e. sprite coords for the M6
 * single-frame, 1× compose path).
 *
 * Mirrors `pincel_core::DrawEllipse` (Alois Zingl's integer midpoint
 * algorithm) so the preview matches the pixels `applyEllipse` commits
 * on release. Degenerate (zero-width / zero-height) bboxes collapse to
 * a single-axis line; out-of-canvas pixels are skipped.
 */
export function paintEllipsePreview(
  canvas: HTMLCanvasElement,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  color: string,
  fill: boolean,
): void {
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.fillStyle = color;
  const minX = Math.min(x0, x1);
  const maxX = Math.max(x0, x1);
  const minY = Math.min(y0, y1);
  const maxY = Math.max(y0, y1);
  if (fill) {
    // Per-row x extents over the bbox's y range, restricted to canvas
    // rows so a far-out drag does not allocate per-pixel-row.
    const rowLoY = Math.max(minY, 0);
    const rowHiY = Math.min(maxY, canvas.height - 1);
    const rowCount = rowHiY - rowLoY + 1;
    if (rowCount <= 0) return;
    // `noUncheckedIndexedAccess` types every index read as `T |
    // undefined`. The bounds checks below (`idx < rowCount`,
    // `sy in [rowLoY..rowHiY]`) are sufficient to guarantee the slot
    // exists; the non-null assertions document that invariant.
    const rowMin = new Float64Array(rowCount).fill(Number.POSITIVE_INFINITY);
    const rowMax = new Float64Array(rowCount).fill(Number.NEGATIVE_INFINITY);
    midpointEllipse(minX, minY, maxX, maxY, (sx, sy) => {
      if (sy < rowLoY || sy > rowHiY) return;
      const idx = sy - rowLoY;
      if (sx < rowMin[idx]!) rowMin[idx] = sx;
      if (sx > rowMax[idx]!) rowMax[idx] = sx;
    });
    for (let idx = 0; idx < rowCount; idx++) {
      const lx = rowMin[idx]!;
      const rx = rowMax[idx]!;
      if (lx > rx) continue;
      const loX = Math.max(lx, 0);
      const hiX = Math.min(rx, canvas.width - 1);
      if (loX > hiX) continue;
      ctx.fillRect(loX, rowLoY + idx, hiX - loX + 1, 1);
    }
    return;
  }
  midpointEllipse(minX, minY, maxX, maxY, (sx, sy) => {
    if (sx < 0 || sy < 0 || sx >= canvas.width || sy >= canvas.height) return;
    ctx.fillRect(sx, sy, 1, 1);
  });
}

function midpointEllipse(
  minX: number,
  minY: number,
  maxX: number,
  maxY: number,
  plot: (sx: number, sy: number) => void,
): void {
  const a = maxX - minX;
  const b = maxY - minY;
  if (a === 0 && b === 0) {
    plot(minX, minY);
    return;
  }
  if (a === 0) {
    for (let sy = minY; sy <= maxY; sy++) plot(minX, sy);
    return;
  }
  if (b === 0) {
    for (let sx = minX; sx <= maxX; sx++) plot(sx, minY);
    return;
  }
  const b1 = b & 1;
  let xLo = minX;
  let xHi = maxX;
  let yTop = minY + ((b + 1) >> 1);
  let yBot = yTop - b1;
  let dx = 4 * (1 - a) * b * b;
  let dy = 4 * (b1 + 1) * a * a;
  let err = dx + dy + b1 * a * a;
  const stepY = 8 * a * a;
  const stepX = 8 * b * b;
  for (;;) {
    plot(xHi, yTop);
    plot(xLo, yTop);
    plot(xLo, yBot);
    plot(xHi, yBot);
    const e2 = 2 * err;
    if (e2 <= dy) {
      yTop += 1;
      yBot -= 1;
      dy += stepY;
      err += dy;
    }
    if (e2 >= dx || 2 * err > dy) {
      xLo += 1;
      xHi -= 1;
      dx += stepX;
      err += dx;
    }
    if (xLo > xHi) break;
  }
  // Flat ellipses (a == 1) finish their y tips one pixel wide.
  while (yTop - yBot < b) {
    plot(xLo - 1, yTop);
    plot(xHi + 1, yTop);
    yTop += 1;
    plot(xLo - 1, yBot);
    plot(xHi + 1, yBot);
    yBot -= 1;
  }
}

function* bresenham(
  x0: number,
  y0: number,
  x1: number,
  y1: number,
): IterableIterator<[number, number]> {
  const dx = Math.abs(x1 - x0);
  const dy = -Math.abs(y1 - y0);
  const sx = x0 < x1 ? 1 : -1;
  const sy = y0 < y1 ? 1 : -1;
  let err = dx + dy;
  let x = x0;
  let y = y0;
  for (;;) {
    yield [x, y];
    if (x === x1 && y === y1) return;
    const e2 = 2 * err;
    if (e2 >= dy) {
      err += dy;
      x += sx;
    }
    if (e2 <= dx) {
      err += dx;
      y += sy;
    }
  }
}
