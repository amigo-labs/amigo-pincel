<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  // Scale Image dialog (spec §10.5, §16): width/height with optional
  // proportion lock and an interpolation choice. Scales all layers together.
  import { editor, MAX_CANVAS_DIM, clampCanvasSize } from '../../stores/editor.svelte';
  import type { Interpolation } from '../../core/wasm';
  import Modal from './Modal.svelte';

  interface Props {
    onApply: (width: number, height: number, interpolation: Interpolation) => void;
    onClose: () => void;
  }
  const { onApply, onClose }: Props = $props();

  const aspect = editor.height > 0 ? editor.width / editor.height : 1;
  let width = $state(editor.width);
  let height = $state(editor.height);
  let constrain = $state(true);
  let interpolation = $state<Interpolation>('bilinear');

  function onWidthInput(): void {
    if (constrain && aspect > 0) {
      height = Math.max(1, Math.round(width / aspect));
    }
  }
  function onHeightInput(): void {
    if (constrain) {
      width = Math.max(1, Math.round(height * aspect));
    }
  }

  // Typed values can exceed the input's max; clamp to the edge / area limits.

  function apply(): void {
    if (width > 0 && height > 0) {
      onApply(...clampCanvasSize(width, height), interpolation);
    }
  }
</script>

<Modal title={t('resize.scaleImageTitle')} applyLabel={t('resize.scale')} onApply={apply} {onClose}>
  <label class="mb-2 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('common.width')}</span>
    <input
      type="number"
      min="1"
      max={MAX_CANVAS_DIM}
      bind:value={width}
      oninput={onWidthInput}
      class="w-24 rounded border border-pc-border bg-pc-bg px-2 py-1"
    />
  </label>
  <label class="mb-2 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('common.height')}</span>
    <input
      type="number"
      min="1"
      max={MAX_CANVAS_DIM}
      bind:value={height}
      oninput={onHeightInput}
      class="w-24 rounded border border-pc-border bg-pc-bg px-2 py-1"
    />
  </label>
  <label class="mb-2 flex items-center gap-2">
    <input type="checkbox" bind:checked={constrain} />
    <span class="text-pc-muted">{t('resize.constrain')}</span>
  </label>
  <label class="mb-3 flex items-center justify-between gap-2">
    <span class="text-pc-muted">{t('resize.resampling')}</span>
    <select
      bind:value={interpolation}
      class="rounded border border-pc-border bg-pc-raised px-2 py-1"
    >
      <option value="nearest">{t('resize.nearestShort')}</option>
      <option value="bilinear">{t('resize.bilinear')}</option>
      <option value="bicubic">{t('resize.bicubic')}</option>
    </select>
  </label>
</Modal>
