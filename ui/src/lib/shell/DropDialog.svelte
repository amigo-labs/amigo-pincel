<script lang="ts">
  // "Dropped images": add the images dropped onto an open editor as
  // layers, or open the first as a document (docs/specs/file-access.md
  // AC-20, AC-22, AC-25). Keys typed here never reach the editor's
  // window-level shortcuts.
  import Button from '../components/Button.svelte';
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';

  interface Props {
    /** Names of the dropped images, in drop order. */
    names: readonly string[];
    /** "Add as layer" is possible for this drop (AC-22). */
    canAddLayers: boolean;
    onChoose: (action: 'layer' | 'document', dontAsk: boolean) => void;
    onCancel: () => void;
  }
  const { names, canAddLayers, onChoose, onCancel }: Props = $props();

  let dontAsk = $state(false);
  let primary: HTMLButtonElement | undefined = $state();
  let openButton: HTMLButtonElement | undefined = $state();

  // Add as layer is the default; without it (AC-22) Open as document takes
  // the focus, so Escape and the editor's keys stay with the dialog.
  onMount(() => (primary ?? openButton)?.focus());

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<div
  class="fixed inset-0 z-(--pc-z-dialog) flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('drop.title')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div class="am-dialog">
    <header>
      <h2>{t('drop.title')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <p class="text-pc-muted">{t('drop.prompt')}</p>
      <ul class="max-h-32 overflow-y-auto text-xs">
        {#each names as name, i (i)}
          <li class="truncate">{name}</li>
        {/each}
      </ul>
      {#if !canAddLayers}
        <div class="am-banner am-banner--warning"><div><strong>{t('drop.pngOnly')}</strong></div></div>
      {/if}
      <label class="am-check flex items-center gap-2 text-xs">
        <input type="checkbox" bind:checked={dontAsk} />
        <span>{t('drop.dontAsk')}</span>
      </label>
    </div>
    <footer class="flex-wrap">
      <Button onclick={onCancel}>{t('common.cancel')}</Button>
      <Button bind:el={openButton} onclick={() => onChoose('document', dontAsk)}>
        {t('drop.openDocument')}
      </Button>
      {#if canAddLayers}
        <Button bind:el={primary} variant="primary" onclick={() => onChoose('layer', dontAsk)}>
          {t('drop.addLayer')}
        </Button>
      {:else}
        <Button disabled>{t('drop.addLayer')}</Button>
      {/if}
    </footer>
  </div>
</div>
