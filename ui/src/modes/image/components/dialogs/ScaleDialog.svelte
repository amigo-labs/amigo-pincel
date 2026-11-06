<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  // Scale Image dialog (spec §10.5, §16; docs/specs/image-resize-autocrop.md):
  // the shared size input plus a resampling choice. Scales all layers together.
  import type { Interpolation } from '../../core/wasm';
  import type { SizeDoc } from '../../size-input';
  import Modal from './Modal.svelte';
  import SizeInput from './SizeInput.svelte';

  interface Props {
    doc: SizeDoc;
    onApply: (width: number, height: number, interpolation: Interpolation) => void;
    onClose: () => void;
  }
  const { doc, onApply, onClose }: Props = $props();

  let size = $state<[number, number] | null>(null);
  let interpolation = $state<Interpolation>('supersampling');

  function apply(): void {
    if (size) onApply(size[0], size[1], interpolation);
  }
</script>

<Modal
  title={t('resize.scaleImageTitle')}
  applyLabel={t('resize.scale')}
  applyDisabled={size === null}
  onApply={apply}
  {onClose}
>
  <SizeInput {doc} onSize={(next) => (size = next)} />
  <label class="mb-3 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('resize.resampling')}</span>
    <select
      bind:value={interpolation}
      class="rounded border border-pc-border bg-pc-raised px-2 py-1"
    >
      <option value="supersampling">{t('resize.supersampling')}</option>
      <option value="nearest">{t('resize.nearestShort')}</option>
      <option value="bilinear">{t('resize.bilinear')}</option>
      <option value="bicubic">{t('resize.bicubic')}</option>
    </select>
  </label>
</Modal>
