// Canvas2D rendering adapter (spec §16.4). The canvas covers the viewport,
// not the document: each frame composites only the visible document rect
// (resampled to screen resolution when zoomed out) and draws it over a
// checkerboard at the view transform. A document larger than any browser
// canvas therefore still displays — the backing store is bounded by the
// viewport. The canvas is render-only — pixels are never read back
// (CLAUDE.md §9, ADR-001).

const CHECKER_SIZE = 8;
const CHECKER_LIGHT = '#a0a0a0';
const CHECKER_DARK = '#808080';

/** Pan / zoom of the document on screen, in CSS pixels. */
export interface ViewTransform {
  zoom: number;
  panX: number;
  panY: number;
}

/** Composites document rect `(x, y, w, h)` resampled to `outW × outH`. */
export type RegionSampler = (
  x: number,
  y: number,
  w: number,
  h: number,
  outW: number,
  outH: number,
) => Uint8ClampedArray<ArrayBuffer> | null;

// Offscreen scratch canvas used to turn raw RGBA into a drawable image so the
// checkerboard shows through transparent regions (putImageData would erase it).
let scratch: HTMLCanvasElement | null = null;
let checker: CanvasPattern | null = null;

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

function checkerPattern(ctx: CanvasRenderingContext2D): CanvasPattern | null {
  if (checker) return checker;
  const tile = document.createElement('canvas');
  tile.width = tile.height = CHECKER_SIZE * 2;
  const t = tile.getContext('2d');
  if (!t) return null;
  t.fillStyle = CHECKER_LIGHT;
  t.fillRect(0, 0, tile.width, tile.height);
  t.fillStyle = CHECKER_DARK;
  t.fillRect(0, 0, CHECKER_SIZE, CHECKER_SIZE);
  t.fillRect(CHECKER_SIZE, CHECKER_SIZE, CHECKER_SIZE, CHECKER_SIZE);
  checker = ctx.createPattern(tile, 'repeat');
  return checker;
}

/**
 * Renders the visible part of a `docW × docH` document into the
 * viewport-sized `canvas` at view transform `view`. `sample` supplies the
 * pixels of the visible rect (see `RegionSampler`).
 */
export function drawViewport(
  canvas: HTMLCanvasElement,
  docW: number,
  docH: number,
  view: ViewTransform,
  sample: RegionSampler,
): void {
  const dpr = window.devicePixelRatio || 1;
  const cssW = canvas.clientWidth;
  const cssH = canvas.clientHeight;
  const cw = Math.max(1, Math.round(cssW * dpr));
  const ch = Math.max(1, Math.round(cssH * dpr));
  if (canvas.width !== cw || canvas.height !== ch) {
    canvas.width = cw;
    canvas.height = ch;
  }
  const ctx = canvas.getContext('2d');
  if (!ctx) {
    return;
  }
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, cw, ch);

  // Visible document rect, in whole document pixels.
  const x0 = Math.max(0, Math.floor(-view.panX / view.zoom));
  const y0 = Math.max(0, Math.floor(-view.panY / view.zoom));
  const x1 = Math.min(docW, Math.ceil((cssW - view.panX) / view.zoom));
  const y1 = Math.min(docH, Math.ceil((cssH - view.panY) / view.zoom));
  if (x1 <= x0 || y1 <= y0) {
    return;
  }
  const rw = x1 - x0;
  const rh = y1 - y0;
  // Zoomed out, the core downsamples to (about) device resolution so the
  // buffer stays viewport-sized; zoomed in, it returns 1:1 pixels that are
  // upscaled here without smoothing.
  const deviceScale = view.zoom * dpr;
  const ow = Math.min(rw, Math.max(1, Math.ceil(rw * deviceScale)));
  const oh = Math.min(rh, Math.max(1, Math.ceil(rh * deviceScale)));
  const rgba = sample(x0, y0, rw, rh, ow, oh);
  if (!rgba || rgba.length !== ow * oh * 4) {
    return;
  }
  const off = scratchCanvas(ow, oh);
  const offCtx = off.getContext('2d');
  if (!offCtx) {
    return;
  }
  // `rgba` is already an ArrayBuffer-backed copy (the wasm adapter narrows the
  // type), so ImageData takes it directly — no per-frame copy.
  offCtx.putImageData(new ImageData(rgba, ow, oh), 0, 0);

  const dx = (view.panX + x0 * view.zoom) * dpr;
  const dy = (view.panY + y0 * view.zoom) * dpr;
  const dw = rw * view.zoom * dpr;
  const dh = rh * view.zoom * dpr;
  const pattern = checkerPattern(ctx);
  if (pattern) {
    ctx.fillStyle = pattern;
    ctx.fillRect(dx, dy, dw, dh);
  }
  ctx.imageSmoothingEnabled = ow < rw || oh < rh;
  ctx.drawImage(off, dx, dy, dw, dh);
}

/**
 * Nearest-neighbour crop + resample of a full-document RGBA buffer — the
 * `RegionSampler` for a live effect preview, which arrives as one
 * document-sized composite.
 */
export function sampleFullBuffer(src: Uint8ClampedArray<ArrayBuffer>, docW: number): RegionSampler {
  return (x, y, w, h, outW, outH) => {
    const out = new Uint8ClampedArray(outW * outH * 4);
    for (let oy = 0; oy < outH; oy++) {
      const sy = y + Math.floor(((oy + 0.5) * h) / outH);
      for (let ox = 0; ox < outW; ox++) {
        const sx = x + Math.floor(((ox + 0.5) * w) / outW);
        const s = (sy * docW + sx) * 4;
        const d = (oy * outW + ox) * 4;
        out[d] = src[s] ?? 0;
        out[d + 1] = src[s + 1] ?? 0;
        out[d + 2] = src[s + 2] ?? 0;
        out[d + 3] = src[s + 3] ?? 0;
      }
    }
    return out;
  };
}
