<script lang="ts">
  // "Paste larger image" (spec §5.4): Expand canvas / Keep canvas size /
  // Cancel, with Image mode's wording. Pixel mode mounts it; Image mode
  // shows the same text through its own Modal.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';

  interface Props {
    width: number;
    height: number;
    /** The canvas can grow to cover the image; otherwise only Keep is offered. */
    canExpand: boolean;
    onExpand: () => void;
    onKeep: () => void;
    onCancel: () => void;
  }
  const { width, height, canExpand, onExpand, onKeep, onCancel }: Props = $props();

  let primary: HTMLButtonElement | undefined = $state();

  onMount(() => primary?.focus());

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
  aria-label={t('paste.largerTitle')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="w-96 max-w-full rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
  >
    <p class="font-semibold">{t('paste.largerTitle')}</p>
    <p class="mt-3 text-pc-muted">
      {t(canExpand ? 'paste.larger' : 'paste.tooLarge', { w: width, h: height })}
    </p>
    <div class="mt-4 flex flex-wrap justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      {#if canExpand}
        <button type="button" class="panel-btn" onclick={onKeep}>{t('paste.keep')}</button>
        <button bind:this={primary} type="button" class="panel-btn panel-btn-primary" onclick={onExpand}>
          {t('paste.expand')}
        </button>
      {:else}
        <button bind:this={primary} type="button" class="panel-btn panel-btn-primary" onclick={onKeep}>
          {t('paste.keep')}
        </button>
      {/if}
    </div>
  </div>
</div>
