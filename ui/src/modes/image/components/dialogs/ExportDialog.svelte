<script lang="ts">
  // JPEG export dialog (spec §13.2): lossy quality 1–100. PNG/WebP are
  // lossless and export directly without this dialog.
  import Modal from './Modal.svelte';

  interface Props {
    onApply: (quality: number) => void;
    onClose: () => void;
  }
  const { onApply, onClose }: Props = $props();

  let quality = $state(90);
</script>

<Modal title="Export JPEG" applyLabel="Export" onApply={() => onApply(quality)} {onClose}>
  <label class="mb-3 flex items-center justify-between gap-2">
    <span class="text-neutral-400">Quality</span>
    <input type="range" min="1" max="100" bind:value={quality} class="flex-1" />
    <span class="w-8 text-right tabular-nums">{quality}</span>
  </label>
</Modal>
