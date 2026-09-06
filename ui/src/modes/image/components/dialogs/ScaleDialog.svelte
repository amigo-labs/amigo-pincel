<script lang="ts">
  // Scale Image dialog (spec §10.5, §16): width/height with optional
  // proportion lock and an interpolation choice. Scales all layers together.
  import { editor } from '../../stores/editor.svelte';
  import type { Interpolation } from '../../core/wasm';
  import Modal from './Modal.svelte';

  interface Props {
    onApply: (width: number, height: number, interpolation: Interpolation) => void;
    onClose: () => void;
  }
  const { onApply, onClose }: Props = $props();

  const aspect = editor.height > 0 ? editor.width / editor.height : 1;
  let width = $state(editor.width);
  let height = $state(editor.height);
  let constrain = $state(true);
  let interpolation = $state<Interpolation>('bilinear');

  function onWidthInput(): void {
    if (constrain && aspect > 0) {
      height = Math.max(1, Math.round(width / aspect));
    }
  }
  function onHeightInput(): void {
    if (constrain) {
      width = Math.max(1, Math.round(height * aspect));
    }
  }

  function apply(): void {
    if (width > 0 && height > 0) {
      onApply(Math.round(width), Math.round(height), interpolation);
    }
  }
</script>

<Modal title="Scale Image" applyLabel="Scale" onApply={apply} {onClose}>
  <label class="mb-2 flex items-center justify-between gap-2">
    <span class="text-neutral-400">Width</span>
    <input
      type="number"
      min="1"
      max="32767"
      bind:value={width}
      oninput={onWidthInput}
      class="w-24 rounded border border-[var(--fl-panel-border)] bg-[var(--fl-app-bg)] px-2 py-1"
    />
  </label>
  <label class="mb-2 flex items-center justify-between gap-2">
    <span class="text-neutral-400">Height</span>
    <input
      type="number"
      min="1"
      max="32767"
      bind:value={height}
      oninput={onHeightInput}
      class="w-24 rounded border border-[var(--fl-panel-border)] bg-[var(--fl-app-bg)] px-2 py-1"
    />
  </label>
  <label class="mb-2 flex items-center gap-2">
    <input type="checkbox" bind:checked={constrain} />
    <span class="text-neutral-400">Constrain proportions</span>
  </label>
  <label class="mb-3 flex items-center justify-between gap-2">
    <span class="text-neutral-400">Resampling</span>
    <select
      bind:value={interpolation}
      class="rounded border border-[var(--fl-panel-border)] bg-neutral-800 px-2 py-1"
    >
      <option value="nearest">Nearest</option>
      <option value="bilinear">Bilinear</option>
      <option value="bicubic">Bicubic</option>
    </select>
  </label>
</Modal>
