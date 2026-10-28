<script lang="ts">
  import { t } from '../../../../lib/i18n/index.svelte';
  // Shared modal shell: backdrop, panel, title, Cancel/Apply footer. Owns the
  // dialog keyboard behavior (Escape cancels, Enter applies), focuses the
  // first form control, and suppresses the global shortcuts while open via
  // `ui.modalOpen`.
  import { onMount, type Snippet } from 'svelte';
  import { ui } from '../../stores/editor.svelte';

  interface Props {
    title: string;
    applyLabel: string;
    onApply: () => void;
    onClose: () => void;
    /** An extra choice between Cancel and Apply. */
    secondary?: { label: string; onClick: () => void } | undefined;
    children: Snippet;
  }
  const { title, applyLabel, onApply, onClose, secondary, children }: Props = $props();

  let panel: HTMLDivElement;

  onMount(() => {
    ui.modalOpen = true;
    panel.querySelector<HTMLElement>('input, select, button')?.focus();
    return () => {
      ui.modalOpen = false;
    };
  });

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.stopPropagation();
      onClose();
    } else if (e.key === 'Enter' && !(e.target instanceof HTMLButtonElement)) {
      // Buttons keep their native Enter=click; everywhere else Enter applies.
      e.stopPropagation();
      e.preventDefault();
      onApply();
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
  role="presentation"
  onclick={onClose}
  onkeydown={onKeydown}
>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    bind:this={panel}
    class="max-w-[calc(100vw-2rem)] rounded-lg border border-pc-border bg-pc-panel p-4 text-sm text-pc-text"
    class:w-72={!secondary}
    class:w-96={secondary}
    role="dialog"
    aria-label={title}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
  >
    <h2 class="mb-3 text-sm font-semibold">{title}</h2>

    {@render children()}

    <div class="flex justify-end gap-2">
      <button class="rounded px-3 py-1 hover:bg-pc-hover" onclick={onClose}>{t('common.cancel')}</button>
      {#if secondary}
        <button class="rounded border border-pc-border px-3 py-1 hover:bg-pc-hover" onclick={secondary.onClick}>
          {secondary.label}
        </button>
      {/if}
      <button class="rounded bg-pc-accent px-3 py-1 text-white" onclick={onApply}>
        {applyLabel}
      </button>
    </div>
  </div>
</div>
