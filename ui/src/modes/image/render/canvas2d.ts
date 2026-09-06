// Canvas2D rendering adapter (spec §16.4). Draws the checkerboard transparency
// background, then the composited image on top. The canvas is render-only —
// pixels are never read back (CLAUDE.md §9, ADR-001).

const CHECKER_SIZE = 8;
const CHECKER_LIGHT = '#a0a0a0';
const CHECKER_DARK = '#808080';

// Offscreen scratch canvas used to turn raw RGBA into a drawable image so the
// checkerboard shows through transparent regions (putImageData would erase it).
let scratch: HTMLCanvasElement | null = null;

function scratchCanvas(width: number, height: number): HTMLCanvasElement {
  if (!scratch) {
    scratch = document.createElement('canvas');
  }
  if (scratch.width !== width || scratch.height !== height) {
    scratch.width = width;
    scratch.height = height;
  }
  return scratch;
}

function paintCheckerboard(ctx: CanvasRenderingContext2D, width: number, height: number): void {
  for (let y = 0; y < height; y += CHECKER_SIZE) {
    for (let x = 0; x < width; x += CHECKER_SIZE) {
      const dark = (x / CHECKER_SIZE + y / CHECKER_SIZE) % 2 === 0;
      ctx.fillStyle = dark ? CHECKER_DARK : CHECKER_LIGHT;
      ctx.fillRect(x, y, CHECKER_SIZE, CHECKER_SIZE);
    }
  }
}

/** Renders an RGBA8 composite into `canvas`, sizing it to the document. */
export function drawComposite(
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
  rgba: Uint8ClampedArray,
): void {
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  const ctx = canvas.getContext('2d');
  if (!ctx) {
    return;
  }

  const off = scratchCanvas(width, height);
  const offCtx = off.getContext('2d');
  if (!offCtx) {
    return;
  }
  // Copy into an ArrayBuffer-backed view so ImageData accepts it (the WASM
  // return type is backed by ArrayBufferLike).
  const pixels = new Uint8ClampedArray(rgba);
  offCtx.putImageData(new ImageData(pixels, width, height), 0, 0);

  ctx.imageSmoothingEnabled = false;
  paintCheckerboard(ctx, width, height);
  ctx.drawImage(off, 0, 0);
}
