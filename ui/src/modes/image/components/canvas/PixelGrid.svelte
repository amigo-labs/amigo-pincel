<script lang="ts">
  // View ▸ Pixel Grid (docs/specs/image-view-aids.md AC-06 – AC-09): 1-device-
  // pixel lines between image pixels from 400 % zoom up, inside the image's
  // bounds only. A viewport-sized canvas below the marching ants; it draws
  // the visible lines only, so its cost follows the view, not the image.
  import { onMount } from 'svelte';
  import { editor, view } from '../../stores/editor.svelte';
  import { viewAids } from '../../stores/view-aids.svelte';
  import { PIXEL_GRID_MIN_ZOOM } from '../../view-units';

  let canvas: HTMLCanvasElement;
  let size = $state({ w: 0, h: 0, dpr: 1 });

  function draw(): void {
    if (!canvas) return;
    const { w, h, dpr } = size;
    const cw = Math.max(1, Math.round(w * dpr));
    const ch = Math.max(1, Math.round(h * dpr));
    if (canvas.width !== cw || canvas.height !== ch) {
      canvas.width = cw;
      canvas.height = ch;
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, cw, ch);
    if (!viewAids.showPixelGrid || view.zoom < PIXEL_GRID_MIN_ZOOM || editor.handle === null) {
      return;
    }
    const z = view.zoom;
    // Visible image columns / rows, clamped to the image.
    const x0 = Math.max(0, Math.floor(-view.panX / z));
    const x1 = Math.min(editor.width, Math.ceil((w - view.panX) / z));
    const y0 = Math.max(0, Math.floor(-view.panY / z));
    const y1 = Math.min(editor.height, Math.ceil((h - view.panY) / z));
    if (x0 >= x1 || y0 >= y1) return;
    // Device-pixel edges of the visible part of the image.
    const dev = (v: number): number => Math.round(v * dpr);
    const left = dev(view.panX + x0 * z);
    const right = dev(view.panX + x1 * z);
    const top = dev(view.panY + y0 * z);
    const bottom = dev(view.panY + y1 * z);
    ctx.beginPath();
    for (let x = x0 + 1; x < x1; x++) {
      const dx = dev(view.panX + x * z) + 0.5;
      ctx.moveTo(dx, top);
      ctx.lineTo(dx, bottom);
    }
    for (let y = y0 + 1; y < y1; y++) {
      const dy = dev(view.panY + y * z) + 0.5;
      ctx.moveTo(left, dy);
      ctx.lineTo(right, dy);
    }
    ctx.lineWidth = 1;
    // Mid grey at partial opacity shows over black and white pixels alike.
    ctx.strokeStyle = 'rgba(128, 128, 128, 0.7)';
    ctx.stroke();
  }

  onMount(() => {
    const measure = (): void => {
      size = { w: canvas.clientWidth, h: canvas.clientHeight, dpr: window.devicePixelRatio || 1 };
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(canvas);
    return () => ro.disconnect();
  });

  $effect(() => {
    void size;
    void viewAids.showPixelGrid;
    void view.zoom;
    void view.panX;
    void view.panY;
    void editor.width;
    void editor.height;
    void editor.handle;
    draw();
  });
</script>

<canvas
  bind:this={canvas}
  class="pointer-events-none absolute inset-0 h-full w-full"
  data-testid="pixel-grid"
></canvas>
