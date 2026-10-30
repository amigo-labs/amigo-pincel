<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  // JPEG quality dialog: Export ▸ JPEG (spec §13.2) and the first JPEG Save
  // of a document (save-original-format). PNG/WebP are lossless and write
  // directly without this dialog.
  import Modal from './Modal.svelte';

  interface Props {
    /** Dialog title; defaults to the export title. */
    title?: string;
    /** Apply button label; defaults to "Export". */
    applyLabel?: string;
    onApply: (quality: number) => void;
    onClose: () => void;
  }
  const { title, applyLabel, onApply, onClose }: Props = $props();

  let quality = $state(90);
</script>

<Modal
  title={title ?? t('export.jpegTitle')}
  applyLabel={applyLabel ?? t('export.export')}
  onApply={() => onApply(quality)}
  {onClose}
>
  <label class="mb-3 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('export.quality')}</span>
    <input type="range" min="1" max="100" bind:value={quality} class="flex-1" />
    <span class="w-8 text-right tabular-nums">{quality}</span>
  </label>
</Modal>
