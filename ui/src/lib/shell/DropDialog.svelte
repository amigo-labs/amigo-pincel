<script lang="ts">
  // "Dropped images": add the images dropped onto an open editor as
  // layers, or open the first as a document (docs/specs/file-access.md
  // AC-20, AC-22, AC-25). Keys typed here never reach the editor's
  // window-level shortcuts.
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
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('drop.title')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="w-96 max-w-full rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
  >
    <p class="font-semibold">{t('drop.title')}</p>
    <p class="mt-3 text-pc-muted">{t('drop.prompt')}</p>
    <ul class="mt-2 max-h-32 overflow-y-auto text-xs">
      {#each names as name, i (i)}
        <li class="truncate">{name}</li>
      {/each}
    </ul>
    {#if !canAddLayers}
      <p class="mt-2 text-xs text-amber-400">{t('drop.pngOnly')}</p>
    {/if}
    <label class="mt-3 flex items-center gap-2 text-xs">
      <input type="checkbox" bind:checked={dontAsk} />
      <span>{t('drop.dontAsk')}</span>
    </label>
    <div class="mt-4 flex flex-wrap justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button
        bind:this={openButton}
        type="button"
        class="panel-btn"
        onclick={() => onChoose('document', dontAsk)}
      >
        {t('drop.openDocument')}
      </button>
      {#if canAddLayers}
        <button
          bind:this={primary}
          type="button"
          class="panel-btn panel-btn-primary"
          onclick={() => onChoose('layer', dontAsk)}
        >
          {t('drop.addLayer')}
        </button>
      {:else}
        <button type="button" class="panel-btn" disabled>{t('drop.addLayer')}</button>
      {/if}
    </div>
  </div>
</div>
