<script lang="ts">
  let canvas = $state<HTMLCanvasElement | null>(null);

  $effect(() => {
    const el = canvas;
    if (!el) return;
    const ctx = el.getContext('2d');
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = '#1f2937';
    ctx.fillRect(0, 0, el.width, el.height);
  });
</script>

<main class="flex h-full flex-col">
  <header class="border-b border-neutral-800 px-4 py-2 text-sm font-semibold tracking-wide">
    Pincel
  </header>

  <section class="flex flex-1 items-center justify-center overflow-hidden p-6">
    <canvas
      bind:this={canvas}
      width="64"
      height="64"
      class="canvas-pixelated h-[512px] w-[512px] border border-neutral-700 bg-neutral-900 shadow-lg"
      aria-label="Pincel canvas"
    ></canvas>
  </section>

  <footer class="border-t border-neutral-800 px-4 py-2 text-xs text-neutral-500">
    Phase 1 · M6.5 scaffold
  </footer>
</main>

<style>
  .canvas-pixelated {
    image-rendering: pixelated;
    image-rendering: crisp-edges;
  }
</style>
