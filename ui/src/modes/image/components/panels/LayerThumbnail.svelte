<script lang="ts">
  // A 32×32 layer thumbnail (spec §5.3). Renders a checkerboard so transparency
  // is visible, then the layer's downscaled pixels on top. Redraws whenever the
  // document mutates (`editor.revision`).
  import { editor } from '../../stores/editor.svelte';
  import { layerThumbnail } from '../../core/controller';

  interface Props {
    layerId: string;
  }
  const { layerId }: Props = $props();

  const DIM = 32;
  const CHECK = 4;
  let canvas: HTMLCanvasElement;
  let scratch: HTMLCanvasElement | null = null;

  function draw(): void {
    if (!canvas) {
      return;
    }
    const ctx = canvas.getContext('2d');
    if (!ctx) {
      return;
    }
    // Checkerboard backdrop.
    for (let y = 0; y < DIM; y += CHECK) {
      for (let x = 0; x < DIM; x += CHECK) {
        const dark = (x / CHECK + y / CHECK) % 2 === 0;
        ctx.fillStyle = dark ? '#808080' : '#a0a0a0';
        ctx.fillRect(x, y, CHECK, CHECK);
      }
    }
    const rgba = layerThumbnail(layerId);
    if (!rgba) {
      return;
    }
    if (!scratch) {
      scratch = document.createElement('canvas');
      scratch.width = DIM;
      scratch.height = DIM;
    }
    const sctx = scratch.getContext('2d');
    if (!sctx) {
      return;
    }
    // Copy into an ArrayBuffer-backed view so ImageData accepts it.
    sctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), DIM, DIM), 0, 0);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(scratch, 0, 0);
  }

  $effect(() => {
    void editor.revision;
    void layerId;
    draw();
  });
</script>

<canvas
  bind:this={canvas}
  width={DIM}
  height={DIM}
  class="h-8 w-8 shrink-0 rounded-sm border border-[var(--fl-panel-border)]"
  style="image-rendering: pixelated;"
></canvas>
