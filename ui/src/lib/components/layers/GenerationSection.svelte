<script lang="ts">
  // The active layer's generation info in the layers panel
  // (docs/specs/comfyui-generation-info.md AC-02, AC-04 – AC-06). Mounted
  // per layer, so "Could not copy" goes when another layer is shown.
  import { percent, t } from '../../i18n/index.svelte';
  import type { LayerGeneration } from './layers';

  interface Props {
    generation: LayerGeneration;
    /** Loads the values into the Generate panel; the button is absent
     *  without one (AC-06). */
    onUse?: ((generation: LayerGeneration) => void) | undefined;
  }
  const { generation, onUse }: Props = $props();

  let copyFailed = $state(false);

  const rows = $derived<[string, string][]>([
    [t('gen.prompt'), generation.prompt],
    [t('gen.negative'), generation.negative || '—'],
    [t('gen.model'), generation.model],
    [t('gen.lora'), generation.lora || '—'],
    ...(generation.lora ? [[t('gen.loraStrength'), String(generation.loraStrength)] as [string, string]] : []),
    [t('gen.strength'), percent(generation.strength, 1)],
    [t('gen.seed'), generation.seed],
  ]);

  async function copy(): Promise<void> {
    copyFailed = false;
    try {
      await navigator.clipboard.writeText(generation.text);
    } catch {
      copyFailed = true;
    }
  }

  function use(): void {
    copyFailed = false;
    onUse?.(generation);
  }
</script>

<div
  class="flex flex-col gap-1.5 border-t border-pc-border pt-2"
  role="group"
  aria-label={t('layers.generation')}
>
  <span class="text-pc-faint">{t('layers.generation')}</span>
  <dl class="grid grid-cols-[auto_minmax(0,1fr)] gap-x-2 gap-y-0.5">
    {#each rows as [label, value] (label)}
      <dt class="text-pc-faint">{label}</dt>
      <dd class="min-w-0 break-words whitespace-pre-wrap text-pc-text select-text">{value}</dd>
    {/each}
  </dl>
  <div class="flex flex-wrap items-center gap-1">
    <button type="button" class="am-btn am-btn--sm" onclick={() => void copy()}>{t('layers.genCopy')}</button>
    {#if onUse}
      <button type="button" class="am-btn am-btn--sm" onclick={use}>{t('layers.genUse')}</button>
    {/if}
    {#if copyFailed}
      <span class="text-pc-danger-text" role="status">{t('layers.genCopyFailed')}</span>
    {/if}
  </div>
</div>
