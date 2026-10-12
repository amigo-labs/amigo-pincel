<script lang="ts">
  // A layer thumbnail for the layers panel (spec §9.2): checkerboard so
  // transparency shows, the mode's thumbnail pixels centred on top.
  // `revision` changes whenever the document does, which redraws it.
  import type { Thumb } from './layers';

  interface Props {
    source: () => Thumb | null;
    revision: number;
  }
  const { source, revision }: Props = $props();

  const DIM = 32;
  const CHECK = 4;
  let canvas: HTMLCanvasElement | undefined = $state();
  let scratch: HTMLCanvasElement | null = null;

  function draw(): void {
    const ctx = canvas?.getContext('2d');
    if (!ctx) return;
    for (let y = 0; y < DIM; y += CHECK) {
      for (let x = 0; x < DIM; x += CHECK) {
        ctx.fillStyle = (x / CHECK + y / CHECK) % 2 === 0 ? '#808080' : '#a0a0a0';
        ctx.fillRect(x, y, CHECK, CHECK);
      }
    }
    const thumb = source();
    if (!thumb || thumb.width === 0 || thumb.height === 0) return;
    scratch ??= document.createElement('canvas');
    scratch.width = thumb.width;
    scratch.height = thumb.height;
    const sctx = scratch.getContext('2d');
    if (!sctx) return;
    sctx.putImageData(new ImageData(new Uint8ClampedArray(thumb.rgba), thumb.width, thumb.height), 0, 0);
    // Fit inside the square, keeping the aspect ratio and hard pixels.
    const scale = DIM / Math.max(thumb.width, thumb.height);
    const w = thumb.width * scale;
    const h = thumb.height * scale;
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(scratch, (DIM - w) / 2, (DIM - h) / 2, w, h);
  }

  // Edits bump `revision` in bursts (one per stroke segment on some
  // paths); redraw once the burst settles instead of composing each time.
  let timer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    void revision;
    void source;
    if (timer === null) {
      draw();
      timer = setTimeout(() => (timer = null), 120);
    } else {
      clearTimeout(timer);
      timer = setTimeout(() => {
        timer = null;
        draw();
      }, 120);
    }
  });
  $effect(() => () => {
    if (timer !== null) clearTimeout(timer);
  });
</script>

<canvas
  bind:this={canvas}
  width={DIM}
  height={DIM}
  class="h-8 w-8 shrink-0 rounded-sm border border-pc-border [image-rendering:pixelated]"
  aria-hidden="true"
></canvas>
