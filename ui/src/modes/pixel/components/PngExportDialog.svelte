<script lang="ts">
  import { t } from '../../../lib/i18n/index.svelte';
  import { EXPORT_SCALES } from '../export-scale';
  // File ▸ Export ▸ PNG (current frame) (docs/specs/scaled-export.md).
  // Presentational: the parent exports from `onSubmit`; nothing is written
  // before Export is pressed.

  let {
    frameWidth,
    frameHeight,
    scale: initialScale,
    onSubmit,
    onCancel,
  }: {
    frameWidth: number;
    frameHeight: number;
    /** The preselected scale (the last one used). */
    scale: number;
    onSubmit: (scale: number) => void;
    onCancel: () => void;
  } = $props();

  // Remounted per open, so the initial props are the only ones seen.
  // svelte-ignore state_referenced_locally
  let scale = $state(initialScale);

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
  aria-label={t('png.title')}
>
  <form
    class="am-dialog"
    onsubmit={(e) => {
      e.preventDefault();
      onSubmit(scale);
    }}
  >
    <header>
      <h2>{t('png.title')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <label class="flex flex-col gap-1 text-xs text-pc-text">
        <span>{t('export.scale')}</span>
        <select
          class="am-input w-auto"
          bind:value={scale}
          aria-label={t('export.scale')}
        >
          {#each EXPORT_SCALES as s (s)}
            <option value={s}>×{s}</option>
          {/each}
        </select>
      </label>
      <p class="text-xs text-pc-muted" aria-live="polite">
        {t('png.size', { w: frameWidth * scale, h: frameHeight * scale })}
      </p>
    </div>
    <footer>
      <button type="button" class="am-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="am-btn am-btn--primary">{t('menu.export')}</button>
    </footer>
  </form>
</div>
