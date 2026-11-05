<script lang="ts">
  // Preview panel (docs/specs/animation-preview.md): the animation at 1×,
  // 2×, 4× or Fit, played on its own loop while the main canvas stays on
  // the current frame. The loop and compositing live in `../preview`.
  import { onDestroy } from 'svelte';
  import { t } from '../../../lib/i18n/index.svelte';
  import Panel from '../../../lib/components/layout/Panel.svelte';
  import IconButton from '../../../lib/components/layout/IconButton.svelte';
  import { panelSlot } from '../../../lib/components/layout/panels.svelte';
  import { drawPreview, previewShownFrame, stopPreview, togglePreview } from '../preview';
  import { ed, preview, type PreviewZoom } from '../stores/editor.svelte';

  const ZOOMS: PreviewZoom[] = [1, 2, 4, 'fit'];

  const slot = panelSlot();
  const collapsed = $derived(slot?.host !== 'sheet' && (slot?.collapsed ?? false));

  let canvas = $state<HTMLCanvasElement | null>(null);
  let stageW = $state(0);
  let stageH = $state(0);

  const shown = $derived.by(() => {
    void ed.frameCount;
    return previewShownFrame();
  });

  // Display scale: the zoom factor, or the largest that fits the stage.
  const scale = $derived(
    preview.zoom === 'fit'
      ? Math.max(0, Math.min(stageW / ed.canvasW, stageH / ed.canvasH))
      : preview.zoom,
  );

  // Collapsing the panel stops preview playback (AC-13).
  $effect(() => {
    if (collapsed && preview.playing) stopPreview();
  });

  // Draw when the shown frame or the document changes.
  $effect(() => {
    const frame = shown;
    void preview.contentRev;
    void ed.docRev;
    if (canvas && ed.doc && !collapsed) drawPreview(canvas, frame);
  });

  onDestroy(stopPreview);
</script>

<Panel>
  {#snippet actions()}
    <IconButton
      icon={preview.playing ? 'pause' : 'play'}
      label={preview.playing ? t('preview.pause') : t('preview.play')}
      size="sm"
      disabled={!ed.doc}
      onclick={togglePreview}
    />
  {/snippet}

  <div class="flex items-center gap-1" role="group" aria-label={t('preview.zoom')}>
    {#each ZOOMS as z (z)}
      <button
        type="button"
        class="zoom-btn"
        aria-pressed={preview.zoom === z}
        onclick={() => (preview.zoom = z)}
      >
        {z === 'fit' ? t('preview.fit') : `${z}×`}
      </button>
    {/each}
  </div>
  <div
    class="preview-stage"
    data-testid="preview-stage"
    bind:clientWidth={stageW}
    bind:clientHeight={stageH}
  >
    <canvas
      bind:this={canvas}
      class="preview-canvas"
      aria-label={t('preview.canvas')}
      data-frame={shown}
      style:width="{ed.canvasW * scale}px"
      style:height="{ed.canvasH * scale}px"
    ></canvas>
  </div>
</Panel>

<style>
  .zoom-btn {
    border-radius: 0.25rem;
    border: 1px solid var(--color-pc-border);
    padding: 0 0.4rem;
    font-size: 0.75rem;
    color: var(--color-pc-muted);
  }
  .zoom-btn[aria-pressed='true'] {
    border-color: var(--color-pc-accent);
    color: var(--color-pc-text);
  }
  .preview-stage {
    display: flex;
    height: 12rem;
    overflow: auto;
    border-radius: 0.25rem;
    background-color: var(--color-pc-bg);
  }
  .preview-canvas {
    flex: none;
    margin: auto;
    image-rendering: pixelated;
  }
</style>
