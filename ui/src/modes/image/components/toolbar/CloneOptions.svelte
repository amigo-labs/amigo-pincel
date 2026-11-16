<script lang="ts">
  // The Clone Stamp's own options (docs/specs/clone-stamp.md AC-02, AC-04):
  // Sample and the Set source toggle, after the brush tip controls it
  // shares with the other brush tools.
  import { tool } from '../../stores/editor.svelte';
  import { t } from '../../../../lib/i18n/index.svelte';
</script>

<label class="flex items-center gap-2">
  <span class="text-pc-muted">{t('opt.sample')}</span>
  <select bind:value={tool.cloneSample} class="am-input am-input--sm w-auto">
    <option value="current_layer">{t('opt.currentLayer')}</option>
    <option value="all_layers">{t('opt.allLayers')}</option>
  </select>
</label>

<!-- Pen and touch have no Ctrl / ⌘: the next press sets the source. -->
<button
  type="button"
  class="am-btn am-btn--sm"
  aria-pressed={tool.cloneSetSource}
  title={t('clone.setSourceHint')}
  onclick={() => (tool.cloneSetSource = !tool.cloneSetSource)}>{t('clone.setSource')}</button
>
