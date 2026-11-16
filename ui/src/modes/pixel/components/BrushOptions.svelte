<script lang="ts">
  import { rangeFill } from '../../../lib/range-fill';
  // Options-bar controls of the Pixel-mode Pencil / Eraser: size (field +
  // slider, 1–8), shape, pixel-perfect and pen pressure → size. One setting
  // shared by both tools.
  import { t } from '../../../lib/i18n/index.svelte';
  import { MAX_BRUSH_SIZE, MIN_BRUSH_SIZE, clampBrushSize } from '../brush';
  import { toolState } from '../stores/editor.svelte';

  function onSizeChange(e: Event & { currentTarget: HTMLInputElement }) {
    const size = clampBrushSize(e.currentTarget.valueAsNumber);
    toolState.brushSize = size;
    // The state may not change (8 → typed 12 → 8), so fix the field too.
    e.currentTarget.value = String(size);
  }
</script>

<label class="opt">
  {t('opt.size')}
  <input
    type="number"
    min={MIN_BRUSH_SIZE}
    max={MAX_BRUSH_SIZE}
    step="1"
    value={toolState.brushSize}
    onchange={onSizeChange}
    class="am-input am-input--sm w-12 text-right tabular-nums"
    aria-label={t('px.brushSize')}
  />
</label>
<input
  type="range"
  min={MIN_BRUSH_SIZE}
  max={MAX_BRUSH_SIZE}
  step="1"
  bind:value={toolState.brushSize}
  class="am-range w-20"
  aria-label={t('px.brushSizeSlider')}
  use:rangeFill={toolState.brushSize}
/>
<label class="opt">
  {t('opt.shape')}
  <select bind:value={toolState.brushShape} class="am-input am-input--sm w-auto" aria-label={t('px.brushShape')}>
    <option value="square">{t('opt.square')}</option>
    <option value="round">{t('opt.round')}</option>
  </select>
</label>
<label class="am-check opt opt-divided">
  <input
    type="checkbox"
    bind:checked={toolState.pixelPerfect}
    disabled={toolState.brushSize > 1}
    aria-label={t('opt.pixelPerfect')}
  />
  {t('opt.pixelPerfect')}
</label>
<span class="opt opt-divided" role="group" aria-label={t('pressure.label')} title={t('pressure.hint')}>
  <span class="text-pc-muted">{t('pressure.label')}</span>
  <label class="am-check flex items-center gap-1">
    <input type="checkbox" bind:checked={toolState.pressureSize} />
    {t('pressure.size')}
  </label>
</span>
