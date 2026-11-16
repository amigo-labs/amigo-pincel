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
    /** Disables Apply, and Enter then applies nothing. */
    applyDisabled?: boolean;
    children: Snippet;
  }
  const { title, applyLabel, onApply, onClose, secondary, applyDisabled = false, children }: Props =
    $props();

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
      if (!applyDisabled) onApply();
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
    class="am-dialog max-w-[calc(100vw-2rem)]"
    role="dialog"
    aria-label={title}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
  >
    <header>
      <h2>{title}</h2>
    </header>

    <div class="am-body">
      {@render children()}
    </div>

    <footer>
      <button class="am-btn" onclick={onClose}>{t('common.cancel')}</button>
      {#if secondary}
        <button class="am-btn" onclick={secondary.onClick}>
          {secondary.label}
        </button>
      {/if}
      <button
        class="am-btn am-btn--primary"
        disabled={applyDisabled}
        onclick={onApply}
      >
        {applyLabel}
      </button>
    </footer>
  </div>
</div>
