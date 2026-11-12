<script lang="ts">
  // The selected generated variant over its target box (comfyui-generate
  // AC-10, AC-11): a canvas overlay inside the view-transformed stage, in
  // document pixels, never document state. The variant is fitted to the
  // box and its alpha multiplied by the mask, so outside the mask the
  // layer below shows through (AC-07).
  import { tabs } from '../../stores/editor.svelte';
  import { genTabs } from '../../../../lib/comfyui/generation.svelte';
  import { fittedVariant, genKey } from '../../generate/target';

  let canvas: HTMLCanvasElement | undefined = $state();

  const gen = $derived(tabs.active ? genTabs[genKey(tabs.active)] : undefined);
  const shown = $derived(gen !== undefined && gen.phase === 'done' && gen.variants.length > 0 ? gen : null);

  $effect(() => {
    const g = shown;
    if (!canvas || g === null) return;
    const rgba = fittedVariant(g, g.selected);
    const ctx = canvas.getContext('2d');
    if (!ctx || rgba === null) return;
    canvas.width = g.box.w;
    canvas.height = g.box.h;
    const pixels = new Uint8ClampedArray(rgba.length);
    pixels.set(rgba);
    ctx.putImageData(new ImageData(pixels, g.box.w, g.box.h), 0, 0);
  });
</script>

{#if shown}
  <canvas
    bind:this={canvas}
    class="pointer-events-none absolute"
    data-testid="generate-preview"
    data-variant={shown.selected}
    style="left: {shown.box.x}px; top: {shown.box.y}px; width: {shown.box.w}px; height: {shown.box.h}px; image-rendering: pixelated;"
  ></canvas>
{/if}
