<script lang="ts">
  import Dialog from '../../../lib/components/Dialog.svelte';
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
</script>

<Dialog
  title={t('png.title')}
  applyLabel={t('menu.export')}
  onApply={() => onSubmit(scale)}
  onClose={onCancel}
>
  <div class="flex flex-col gap-3">
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
</Dialog>
