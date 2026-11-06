<script lang="ts">
  // The Clone Stamp's source crosshair (docs/specs/clone-stamp.md AC-15):
  // at the centre of the source pixel until the first stroke fixes the
  // offset, then at the pointer plus the offset while the pointer is over
  // the canvas or painting. A fixed 16 CSS px at every zoom: it sits in the
  // viewport, outside the zoomed stage.
  import { tool, view } from '../../stores/editor.svelte';
  import { activeCloneSource, cloneHover } from '../../tools/clone-source.svelte';

  const SIZE = 16;

  const at = $derived.by((): [number, number] | null => {
    if (tool.kind !== 'clone') return null;
    const source = activeCloneSource();
    if (source === null) return null;
    if (source.offset === null) return [source.point[0] + 0.5, source.point[1] + 0.5];
    const p = cloneHover.point;
    return p === null ? null : [p[0] + source.offset[0], p[1] + source.offset[1]];
  });
</script>

{#if at}
  <svg
    class="pointer-events-none absolute"
    data-testid="clone-crosshair"
    width={SIZE}
    height={SIZE}
    viewBox="0 0 16 16"
    style="left: {view.panX + at[0] * view.zoom - SIZE / 2}px; top: {view.panY +
      at[1] * view.zoom -
      SIZE / 2}px;"
    aria-hidden="true"
  >
    <path d="M8 0v16M0 8h16" stroke="#ffffff" stroke-width="3" />
    <path d="M8 0v16M0 8h16" stroke="#000000" stroke-width="1" />
  </svg>
{/if}
