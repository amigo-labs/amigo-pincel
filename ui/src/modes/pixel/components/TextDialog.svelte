<script lang="ts">
  import { t } from '../../../lib/i18n/index.svelte';
  import FontPicker from '../../../lib/components/fonts/FontPicker.svelte';
  import { pixelFont } from '../text-font';
  // Text tool dialog. The parent owns the wasm `Document`, runs
  // `previewText` on every `onPreview(params)` and commits via
  // `onApply(params)`; `onCancel` restores the un-previewed composite.
  export type TextParams = {
    /** The font family (docs/specs/system-fonts.md); the face is resolved
     *  by the text tool. */
    family: string;
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
    family: pixelFont.state.family,
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
  class="pointer-events-none fixed inset-0 z-(--pc-z-overlay) flex items-start justify-end p-4"
  role="dialog"
  aria-modal="false"
  aria-labelledby="text-title"
>
  <form
    class="am-dialog pointer-events-auto"
    onsubmit={(e) => {
      e.preventDefault();
      if (params.text.trim().length > 0) onApply(params);
    }}
  >
    <header class="flex items-baseline justify-between">
      <h2 id="text-title">{t('tool.text')}</h2>
      <span class="text-xs text-pc-muted">at {x}, {y}</span>
    </header>
    <div class="am-body flex flex-col gap-3">
      <!-- svelte-ignore a11y_autofocus -->
      <textarea
        bind:value={text}
        rows="3"
        autofocus
        placeholder={t('text.placeholder')}
        class="am-input py-1 font-mono w-auto h-auto"
        aria-label={t('text.content')}
      ></textarea>
      <div class="flex items-center gap-2 text-xs text-pc-text">
        <span>{t('font.label')}</span>
        <FontPicker
          family={pixelFont.state.family}
          missing={pixelFont.availability() === 'missing'}
          onPick={(f) => pixelFont.pick(f)}
          onAdded={(r) => pixelFont.added(r)}
        />
      </div>
      <div class="flex items-center gap-3 text-xs text-pc-text">
        <label class="flex items-center gap-1">
          <span>{t('opt.size')}</span>
          <input
            type="number"
            min="1"
            max="2000"
            bind:value={size}
            class="am-input w-16 text-right tabular-nums"
            aria-label={t('text.fontSize')}
          />
        </label>
        <select
          bind:value={align}
          class="am-input w-auto"
          aria-label={t('text.align')}
        >
          <option value="left">{t('opt.left')}</option>
          <option value="center">{t('opt.center')}</option>
          <option value="right">{t('opt.right')}</option>
        </select>
      </div>
      <div class="flex items-center gap-3 text-xs text-pc-text">
        <label class="am-check flex items-center gap-1">
          <input type="checkbox" bind:checked={bold} />
          <span class="font-bold">{t('opt.bold')}</span>
        </label>
        <label class="am-check flex items-center gap-1">
          <input type="checkbox" bind:checked={italic} />
          <span class="italic">{t('opt.italic')}</span>
        </label>
        <label class="am-check flex items-center gap-1">
          <input type="checkbox" bind:checked={antiAlias} />
          <span>{t('opt.antiAlias')}</span>
        </label>
      </div>
    </div>
    <footer>
      <button type="button" class="am-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="am-btn am-btn--primary" disabled={params.text.trim().length === 0}>
        {t('text.place')}
      </button>
    </footer>
  </form>
</div>
