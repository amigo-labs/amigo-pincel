<script lang="ts">
  // File ▸ Print… (docs/specs/image-formats-more-print.md AC-01, AC-08):
  // the placement, then Print hands over to the platform print dialog.
  import { t } from '../../../../lib/i18n/index.svelte';
  import type { PrintPlacement } from '../../core/print';
  import Modal from './Modal.svelte';

  interface Props {
    onPrint: (placement: PrintPlacement) => void;
    onClose: () => void;
  }
  const { onPrint, onClose }: Props = $props();

  let placement = $state<PrintPlacement>('fit');
</script>

<Modal title={t('print.title')} applyLabel={t('print.print')} onApply={() => onPrint(placement)} {onClose}>
  <fieldset class="mb-4 flex flex-col gap-2">
    <label class="flex items-center gap-2">
      <input type="radio" name="print-placement" value="fit" bind:group={placement} />
      <span>{t('print.fit')}</span>
    </label>
    <label class="flex items-center gap-2">
      <input type="radio" name="print-placement" value="actual" bind:group={placement} />
      <span>{t('print.actual')}</span>
    </label>
  </fieldset>
</Modal>
