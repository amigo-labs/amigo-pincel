<script lang="ts">
  // Image mode's dialogs on the shared shell (docs/specs/dialogs.md); this
  // wrapper only keeps the global shortcuts away while one is open
  // (`ui.modalOpen`).
  import { onMount, type Snippet } from 'svelte';
  import Dialog from '../../../../lib/components/Dialog.svelte';
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

  onMount(() => {
    ui.modalOpen = true;
    return () => {
      ui.modalOpen = false;
    };
  });
</script>

<Dialog {title} {applyLabel} {onApply} {onClose} {secondary} {applyDisabled} class="max-w-[calc(100vw-2rem)]">
  {@render children()}
</Dialog>
