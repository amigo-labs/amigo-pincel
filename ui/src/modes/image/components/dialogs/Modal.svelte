<script lang="ts">
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
    children: Snippet;
  }
  const { title, applyLabel, onApply, onClose, children }: Props = $props();

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
    class="w-72 rounded-lg border border-[var(--fl-panel-border)] bg-[var(--fl-panel-bg)] p-4 text-sm text-neutral-200"
    role="dialog"
    aria-label={title}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
  >
    <h2 class="mb-3 text-sm font-semibold">{title}</h2>

    {@render children()}

    <div class="flex justify-end gap-2">
      <button class="rounded px-3 py-1 hover:bg-neutral-700" onclick={onClose}>Cancel</button>
      <button class="rounded bg-[var(--fl-accent)] px-3 py-1 text-white" onclick={onApply}>
        {applyLabel}
      </button>
    </div>
  </div>
</div>
