<script lang="ts">
  import { t } from '../../../lib/i18n/index.svelte';
  // Resize Canvas / Scale Image dialog. Presentational: the parent runs
  // the wasm command from `onSubmit`. `mode` picks which form is shown;
  // `resize` adds the 9-point anchor grid, `scale` the interpolation and
  // an aspect-ratio lock.
  export type ResizeMode = 'resize' | 'scale';
  export type ResizeResult =
    | { mode: 'resize'; width: number; height: number; anchor: string }
    | { mode: 'scale'; width: number; height: number; interpolation: string };

  let {
    mode,
    width: initialWidth,
    height: initialHeight,
    maxSize = 4096,
    onSubmit,
    onCancel,
  }: {
    mode: ResizeMode;
    width: number;
    height: number;
    maxSize?: number;
    onSubmit: (result: ResizeResult) => void;
    onCancel: () => void;
  } = $props();

  // Remounted per open (`{#key}` in the parent), so the initial props are
  // the only ones this instance sees.
  // svelte-ignore state_referenced_locally
  let width = $state(initialWidth);
  // svelte-ignore state_referenced_locally
  let height = $state(initialHeight);
  let anchor = $state('center');
  let interpolation = $state('nearest');
  let keepAspect = $state(true);
  // svelte-ignore state_referenced_locally
  const aspect = initialWidth / Math.max(1, initialHeight);

  const anchors: { id: string; glyph: string }[] = [
    { id: 'top_left', glyph: '↖' },
    { id: 'top', glyph: '↑' },
    { id: 'top_right', glyph: '↗' },
    { id: 'left', glyph: '←' },
    { id: 'center', glyph: '•' },
    { id: 'right', glyph: '→' },
    { id: 'bottom_left', glyph: '↙' },
    { id: 'bottom', glyph: '↓' },
    { id: 'bottom_right', glyph: '↘' },
  ];

  function clampDim(v: number): number {
    return Math.max(1, Math.min(maxSize, Math.floor(Number.isFinite(v) ? v : 1)));
  }

  function onWidthInput(v: number) {
    width = clampDim(v);
    if (mode === 'scale' && keepAspect) height = clampDim(Math.round(width / aspect));
  }

  function onHeightInput(v: number) {
    height = clampDim(v);
    if (mode === 'scale' && keepAspect) width = clampDim(Math.round(height * aspect));
  }

  function submit() {
    const w = clampDim(width);
    const h = clampDim(height);
    if (mode === 'resize') onSubmit({ mode, width: w, height: h, anchor });
    else onSubmit({ mode, width: w, height: h, interpolation });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={mode === 'resize' ? t('resize.resizeTitle') : t('resize.scaleTitle')}
>
  <form
    class="flex w-80 flex-col gap-3 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <h2 class="text-base font-semibold">
      {mode === 'resize' ? t('resize.resizeTitle') : t('resize.scaleTitle')}
    </h2>
    <p class="text-xs text-pc-muted">
      Current size: {initialWidth}×{initialHeight} px
    </p>
    <div class="flex gap-3">
      <label class="flex flex-1 flex-col gap-1 text-xs text-pc-text">
        <span>{t('common.width')}</span>
        <input
          type="number"
          min="1"
          max={maxSize}
          value={width}
          oninput={(e) => onWidthInput(Number(e.currentTarget.value))}
          class="rounded border border-pc-border bg-pc-bg px-2 py-1 tabular-nums"
          aria-label={t('common.width')}
        />
      </label>
      <label class="flex flex-1 flex-col gap-1 text-xs text-pc-text">
        <span>{t('common.height')}</span>
        <input
          type="number"
          min="1"
          max={maxSize}
          value={height}
          oninput={(e) => onHeightInput(Number(e.currentTarget.value))}
          class="rounded border border-pc-border bg-pc-bg px-2 py-1 tabular-nums"
          aria-label={t('common.height')}
        />
      </label>
    </div>

    {#if mode === 'resize'}
      <fieldset class="flex flex-col gap-1 text-xs text-pc-text">
        <legend class="mb-1">{t('resize.anchor')}</legend>
        <div class="grid w-24 grid-cols-3 gap-1" role="radiogroup" aria-label={t('resize.anchor')}>
          {#each anchors as a (a.id)}
            <button
              type="button"
              class="panel-btn h-7 w-7 px-0 text-center"
              class:panel-btn-primary={anchor === a.id}
              role="radio"
              aria-checked={anchor === a.id}
              aria-label={a.id.replace('_', ' ')}
              onclick={() => (anchor = a.id)}
            >
              {a.glyph}
            </button>
          {/each}
        </div>
      </fieldset>
    {:else}
      <label class="flex items-center justify-between gap-2 text-xs text-pc-text">
        <span>{t('resize.interpolation')}</span>
        <select
          bind:value={interpolation}
          class="rounded border border-pc-border bg-pc-bg px-1 py-0.5"
          aria-label={t('resize.interpolation')}
        >
          <option value="nearest">{t('resize.nearest')}</option>
          <option value="bilinear">{t('resize.bilinear')}</option>
          <option value="bicubic">{t('resize.bicubic')}</option>
        </select>
      </label>
      <label class="flex items-center gap-2 text-xs text-pc-text">
        <input type="checkbox" bind:checked={keepAspect} />
        <span>{t('resize.keepAspect')}</span>
      </label>
    {/if}

    <footer class="mt-1 flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary">
        {mode === 'resize' ? t('resize.resize') : t('resize.scale')}
      </button>
    </footer>
  </form>
</div>
