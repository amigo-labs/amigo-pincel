<script lang="ts">
  // Resize Canvas dialog (spec §10.4, §16): width/height + 9-grid anchor. Does
  // not scale content; layers are padded/cropped per the anchor.
  import { editor, MAX_CANVAS_DIM, clampCanvasSize } from '../../stores/editor.svelte';
  import type { ResizeAnchor } from '../../core/wasm';
  import Modal from './Modal.svelte';

  interface Props {
    onApply: (width: number, height: number, anchor: ResizeAnchor) => void;
    onClose: () => void;
  }
  const { onApply, onClose }: Props = $props();

  let width = $state(editor.width);
  let height = $state(editor.height);
  let anchor = $state<ResizeAnchor>('center');

  // 3×3 anchor grid in row-major (top→bottom, left→right) order.
  const grid: ResizeAnchor[] = [
    'top_left',
    'top_center',
    'top_right',
    'center_left',
    'center',
    'center_right',
    'bottom_left',
    'bottom_center',
    'bottom_right',
  ];

  // Typed values can exceed the input's max; clamp to the edge / area limits.

  function apply(): void {
    if (width > 0 && height > 0) {
      onApply(...clampCanvasSize(width, height), anchor);
    }
  }
</script>

<Modal title="Resize Canvas" applyLabel="Resize" onApply={apply} {onClose}>
  <label class="mb-2 flex items-center justify-between gap-2">
    <span class="text-pc-muted">Width</span>
    <input
      type="number"
      min="1"
      max={MAX_CANVAS_DIM}
      bind:value={width}
      class="w-24 rounded border border-pc-border bg-pc-bg px-2 py-1"
    />
  </label>
  <label class="mb-3 flex items-center justify-between gap-2">
    <span class="text-pc-muted">Height</span>
    <input
      type="number"
      min="1"
      max={MAX_CANVAS_DIM}
      bind:value={height}
      class="w-24 rounded border border-pc-border bg-pc-bg px-2 py-1"
    />
  </label>

  <div class="mb-3">
    <span class="mb-1 block text-pc-muted">Anchor</span>
    <div class="grid w-fit grid-cols-3 gap-1">
      {#each grid as a (a)}
        <button
          class="h-7 w-7 rounded border text-xs"
          class:border-pc-accent={anchor === a}
          class:bg-pc-accent={anchor === a}
          class:border-transparent={anchor !== a}
          class:bg-pc-raised={anchor !== a}
          aria-label={a}
          aria-pressed={anchor === a}
          onclick={() => (anchor = a)}
        >
          {anchor === a ? '●' : ''}
        </button>
      {/each}
    </div>
  </div>
</Modal>
