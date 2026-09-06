<script lang="ts">
  // Selection overlay (spec §8.5): draws the committed selection's boundary as
  // animated marching ants, plus the in-progress selection gesture. Rendered on
  // a separate, non-interactive canvas layered exactly over the main canvas, so
  // the document's pixels are never touched (ADR-001).
  import { onMount } from 'svelte';
  import { editor, selectionPreview, shapePreview } from '../../stores/editor.svelte';
  import { selectionMask } from '../../core/controller';

  let canvas: HTMLCanvasElement;
  // Boundary of the committed selection, rebuilt only when the document
  // changes ($state so the animation effect reacts to it).
  let selectionPath = $state<Path2D | null>(null);
  let raf = 0;
  let running = false;
  let dashOffset = 0;

  /** Traces the boundary edges of the committed selection mask. */
  function buildSelectionPath(): Path2D | null {
    const mask = selectionMask();
    if (!mask) {
      return null;
    }
    const w = editor.width;
    const h = editor.height;
    const sel = (x: number, y: number): boolean =>
      x >= 0 && y >= 0 && x < w && y < h && mask[y * w + x] >= 128;
    const path = new Path2D();
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        if (!sel(x, y)) {
          continue;
        }
        if (!sel(x - 1, y)) {
          path.moveTo(x, y);
          path.lineTo(x, y + 1);
        }
        if (!sel(x + 1, y)) {
          path.moveTo(x + 1, y);
          path.lineTo(x + 1, y + 1);
        }
        if (!sel(x, y - 1)) {
          path.moveTo(x, y);
          path.lineTo(x + 1, y);
        }
        if (!sel(x, y + 1)) {
          path.moveTo(x, y + 1);
          path.lineTo(x + 1, y + 1);
        }
      }
    }
    return path;
  }

  /** Builds the path for the in-progress selection gesture, if any. */
  function buildPreviewPath(): Path2D | null {
    const preview = selectionPreview.value;
    if (!preview || preview.points.length < 2) {
      return null;
    }
    const pts = preview.points;
    const path = new Path2D();
    if ((preview.shape === 'rect' || preview.shape === 'ellipse') && pts.length === 2) {
      const [a, b] = pts;
      const x = Math.min(a[0], b[0]);
      const y = Math.min(a[1], b[1]);
      const w = Math.abs(b[0] - a[0]);
      const hh = Math.abs(b[1] - a[1]);
      if (preview.shape === 'rect') {
        path.rect(x, y, w, hh);
      } else {
        path.ellipse(x + w / 2, y + hh / 2, w / 2, hh / 2, 0, 0, Math.PI * 2);
      }
    } else {
      path.moveTo(pts[0][0], pts[0][1]);
      for (let i = 1; i < pts.length; i++) {
        path.lineTo(pts[i][0], pts[i][1]);
      }
      if (preview.shape === 'lasso') {
        path.closePath();
      }
    }
    return path;
  }

  /** Builds the outline of the in-progress Shapes-tool gesture, if any. */
  function buildShapePath(): Path2D | null {
    const sp = shapePreview.value;
    if (!sp) {
      return null;
    }
    const { a, b, kind } = sp;
    const x = Math.min(a[0], b[0]);
    const y = Math.min(a[1], b[1]);
    const w = Math.abs(b[0] - a[0]);
    const h = Math.abs(b[1] - a[1]);
    const path = new Path2D();
    if (kind === 'line') {
      path.moveTo(a[0], a[1]);
      path.lineTo(b[0], b[1]);
    } else if (kind === 'ellipse') {
      path.ellipse(x + w / 2, y + h / 2, w / 2, h / 2, 0, 0, Math.PI * 2);
    } else if (kind === 'polygon') {
      const cx = x + w / 2;
      const cy = y + h / 2;
      const r = Math.min(w, h) / 2;
      const n = Math.max(3, sp.sides);
      for (let i = 0; i < n; i++) {
        const ang = -Math.PI / 2 + (Math.PI * 2 * i) / n;
        const px = cx + r * Math.cos(ang);
        const py = cy + r * Math.sin(ang);
        if (i === 0) path.moveTo(px, py);
        else path.lineTo(px, py);
      }
      path.closePath();
    } else if (kind === 'rounded_rectangle') {
      const r = Math.min(sp.cornerRadius, w / 2, h / 2);
      path.roundRect(x, y, w, h, r);
    } else {
      path.rect(x, y, w, h);
    }
    return path;
  }

  /** Strokes a path as marching ants: solid black under an offset white dash. */
  function marchingAnts(ctx: CanvasRenderingContext2D, path: Path2D): void {
    ctx.lineWidth = 1;
    ctx.setLineDash([]);
    ctx.strokeStyle = 'rgba(0, 0, 0, 0.8)';
    ctx.stroke(path);
    ctx.setLineDash([4, 4]);
    ctx.lineDashOffset = -dashOffset;
    ctx.strokeStyle = '#ffffff';
    ctx.stroke(path);
    ctx.setLineDash([]);
  }

  function draw(): void {
    if (!canvas) {
      return;
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (selectionPath) {
      marchingAnts(ctx, selectionPath);
    }
    const preview = buildPreviewPath();
    if (preview) {
      marchingAnts(ctx, preview);
    }
    const shape = buildShapePath();
    if (shape) {
      marchingAnts(ctx, shape);
    }
  }

  /** Whether anything is on the overlay (else the animation loop idles). */
  function hasContent(): boolean {
    return selectionPath !== null || selectionPreview.value !== null || shapePreview.value !== null;
  }

  // The dash animation only runs while there is something to animate; an idle
  // editor burns no frames.
  function loop(): void {
    dashOffset = (dashOffset + 0.5) % 8;
    draw();
    if (hasContent()) {
      raf = requestAnimationFrame(loop);
    } else {
      running = false;
    }
  }

  onMount(() => () => cancelAnimationFrame(raf));

  // Start the animation when overlay content appears; clear once when it goes.
  $effect(() => {
    if (hasContent()) {
      if (!running) {
        running = true;
        raf = requestAnimationFrame(loop);
      }
    } else {
      draw();
    }
  });

  // Match the backing store to the document size.
  $effect(() => {
    if (canvas) {
      if (canvas.width !== editor.width) {
        canvas.width = editor.width;
      }
      if (canvas.height !== editor.height) {
        canvas.height = editor.height;
      }
    }
  });

  // Rebuild the committed selection boundary whenever the document mutates.
  $effect(() => {
    void editor.revision;
    selectionPath = editor.handle !== null ? buildSelectionPath() : null;
  });
</script>

<canvas
  bind:this={canvas}
  class="pointer-events-none absolute left-0 top-0 h-full w-full"
  style="image-rendering: pixelated;"
></canvas>
