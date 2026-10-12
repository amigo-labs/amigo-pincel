<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  // JPEG export dialog (spec §13.2): lossy quality 1–100. PNG/WebP are
  // lossless and export directly without this dialog.
  import Modal from './Modal.svelte';

  interface Props {
    onApply: (quality: number) => void;
    onClose: () => void;
  }
  const { onApply, onClose }: Props = $props();

  let quality = $state(90);
</script>

<Modal title={t('export.jpegTitle')} applyLabel={t('export.export')} onApply={() => onApply(quality)} {onClose}>
  <label class="mb-3 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('export.quality')}</span>
    <input type="range" min="1" max="100" bind:value={quality} class="flex-1" />
    <span class="w-8 text-right tabular-nums">{quality}</span>
  </label>
</Modal>
