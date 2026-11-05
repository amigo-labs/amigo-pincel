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
    class="flex w-72 flex-col gap-3 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      onSubmit(scale);
    }}
  >
    <h2 class="text-base font-semibold">{t('png.title')}</h2>
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('export.scale')}</span>
      <select
        class="rounded border border-pc-border bg-pc-bg px-2 py-1"
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
    <div class="flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary">{t('menu.export')}</button>
    </div>
  </form>
</div>

<style>
  .panel-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.25rem 0.75rem;
  }
  .panel-btn:hover {
    background-color: rgb(38 38 38);
  }
  .panel-btn-primary {
    border-color: rgb(59 130 246);
    background-color: rgb(29 78 216);
  }
</style>
