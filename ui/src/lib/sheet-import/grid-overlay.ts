// The grid of the Import sprite sheet dialog, drawn over the preview
// (docs/specs/sprite-sheet-import.md AC-01, AC-03). Write-only: the canvas
// is never read back (AGENTS.md §9).

import type { GridValues } from './values';

/** Complete cells of `grid` along an axis of `len` px (spec AC-02). */
function count(len: number, offset: number, cell: number, spacing: number): number {
  if (cell <= 0 || offset + cell > len) return 0;
  return Math.floor((len - offset - cell) / (cell + spacing)) + 1;
}

/** Complete cells of `grid` on a `w × h` sheet: `[columns, rows]`. */
export function gridCells(grid: GridValues, w: number, h: number): [number, number] {
  const [cw, ch, ox, oy, sx, sy] = grid;
  return [count(w, ox, cw, sx), count(h, oy, ch, sy)];
}

/** The accent colour of the theme, for the grid lines. */
function accent(canvas: HTMLCanvasElement): string {
  return getComputedStyle(canvas).getPropertyValue('--color-pc-accent').trim() || 'currentColor';
}

/**
 * Draws the complete cells of `grid` (or nothing for `null`) over a
 * preview of a `w × h` sheet scaled to the canvas size; returns the
 * number of cells drawn.
 */
export function drawGrid(
  canvas: HTMLCanvasElement,
  grid: GridValues | null,
  w: number,
  h: number,
): number {
  const ctx = canvas.getContext('2d');
  if (!ctx) return 0;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (grid === null) return 0;
  const [cols, rows] = gridCells(grid, w, h);
  const [cw, ch, ox, oy, sx, sy] = grid;
  const kx = canvas.width / w;
  const ky = canvas.height / h;
  ctx.strokeStyle = accent(canvas);
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const x = (ox + c * (cw + sx)) * kx;
      const y = (oy + r * (ch + sy)) * ky;
      ctx.rect(
        Math.round(x) + 0.5,
        Math.round(y) + 0.5,
        Math.max(1, Math.round(cw * kx) - 1),
        Math.max(1, Math.round(ch * ky) - 1),
      );
    }
  }
  ctx.stroke();
  return cols * rows;
}
