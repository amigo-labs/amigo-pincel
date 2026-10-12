<script lang="ts">
  import { t } from '../../../lib/i18n/index.svelte';
  // Text tool dialog. The parent owns the wasm `Document`, runs
  // `previewText` on every `onPreview(params)` and commits via
  // `onApply(params)`; `onCancel` restores the un-previewed composite.
  export type TextParams = {
    text: string;
    size: number;
    bold: boolean;
    italic: boolean;
    antiAlias: boolean;
    align: 'left' | 'center' | 'right';
  };

  let {
    x,
    y,
    onPreview,
    onApply,
    onCancel,
  }: {
    x: number;
    y: number;
    onPreview: (params: TextParams) => void;
    onApply: (params: TextParams) => void;
    onCancel: () => void;
  } = $props();

  let text = $state('');
  let size = $state(12);
  let bold = $state(false);
  let italic = $state(false);
  let antiAlias = $state(false);
  let align = $state<'left' | 'center' | 'right'>('left');

  const params = $derived<TextParams>({
    text,
    size: Math.max(1, Math.min(2000, Math.round(Number.isFinite(size) ? size : 12))),
    bold,
    italic,
    antiAlias,
    align,
  });

  let rafId = 0;
  $effect(() => {
    const p = params;
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      onPreview(p);
    });
    return () => {
      if (rafId) cancelAnimationFrame(rafId);
    };
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (params.text.trim().length > 0) onApply(params);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="pointer-events-none fixed inset-0 z-20 flex items-start justify-end p-4"
  role="dialog"
  aria-modal="false"
  aria-labelledby="text-title"
>
  <form
    class="pointer-events-auto flex w-80 flex-col gap-3 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-2xl"
    onsubmit={(e) => {
      e.preventDefault();
      if (params.text.trim().length > 0) onApply(params);
    }}
  >
    <header class="flex items-baseline justify-between">
      <h2 id="text-title" class="text-base font-semibold">{t('tool.text')}</h2>
      <span class="text-xs text-pc-muted">at {x}, {y}</span>
    </header>
    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      bind:value={text}
      rows="3"
      autofocus
      placeholder={t('text.placeholder')}
      class="rounded border border-pc-border bg-pc-bg px-2 py-1 font-mono text-sm"
      aria-label={t('text.content')}
    ></textarea>
    <div class="flex items-center gap-3 text-xs text-pc-text">
      <label class="flex items-center gap-1">
        <span>{t('opt.size')}</span>
        <input
          type="number"
          min="1"
          max="2000"
          bind:value={size}
          class="w-16 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-right tabular-nums"
          aria-label={t('text.fontSize')}
        />
      </label>
      <select
        bind:value={align}
        class="rounded border border-pc-border bg-pc-bg px-1 py-0.5"
        aria-label={t('text.align')}
      >
        <option value="left">{t('opt.left')}</option>
        <option value="center">{t('opt.center')}</option>
        <option value="right">{t('opt.right')}</option>
      </select>
    </div>
    <div class="flex items-center gap-3 text-xs text-pc-text">
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={bold} />
        <span class="font-bold">{t('opt.bold')}</span>
      </label>
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={italic} />
        <span class="italic">{t('opt.italic')}</span>
      </label>
      <label class="flex items-center gap-1">
        <input type="checkbox" bind:checked={antiAlias} />
        <span>{t('opt.antiAlias')}</span>
      </label>
    </div>
    <footer class="mt-1 flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary" disabled={params.text.trim().length === 0}>
        {t('text.place')}
      </button>
    </footer>
  </form>
</div>
