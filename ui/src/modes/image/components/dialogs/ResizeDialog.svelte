<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  // Resize Canvas dialog (spec §10.4, §16; docs/specs/image-resize-autocrop.md):
  // the shared size input + 9-grid anchor. Does not scale content; layers are
  // padded/cropped per the anchor.
  import type { ResizeAnchor } from '../../core/wasm';
  import type { SizeDoc } from '../../size-input';
  import Modal from './Modal.svelte';
  import SizeInput from './SizeInput.svelte';

  interface Props {
    doc: SizeDoc;
    onApply: (width: number, height: number, anchor: ResizeAnchor) => void;
    onClose: () => void;
  }
  const { doc, onApply, onClose }: Props = $props();

  let size = $state<[number, number] | null>(null);
  let anchor = $state<ResizeAnchor>('center');

  // 3×3 anchor grid in row-major (top→bottom, left→right) order.
  const grid: ResizeAnchor[] = [
    'top_left',
    'top_center',
    'top_right',
    'center_left',
    'center',
    'center_right',
    'bottom_left',
    'bottom_center',
    'bottom_right',
  ];

  function apply(): void {
    if (size) onApply(size[0], size[1], anchor);
  }
</script>

<Modal
  title={t('resize.resizeCanvasTitle')}
  applyLabel={t('resize.resize')}
  applyDisabled={size === null}
  onApply={apply}
  {onClose}
>
  <SizeInput {doc} onSize={(next) => (size = next)} />

  <div class="mb-3">
    <span class="mb-1 block text-pc-muted">{t('resize.anchor')}</span>
    <div class="grid w-fit grid-cols-3 gap-1">
      {#each grid as a (a)}
        <button
          class="h-7 w-7 rounded border text-xs"
          class:border-pc-accent={anchor === a}
          class:bg-pc-accent={anchor === a}
          class:border-transparent={anchor !== a}
          class:bg-pc-raised={anchor !== a}
          aria-label={a}
          aria-pressed={anchor === a}
          onclick={() => (anchor = a)}
        >
          {anchor === a ? '●' : ''}
        </button>
      {/each}
    </div>
  </div>
</Modal>
