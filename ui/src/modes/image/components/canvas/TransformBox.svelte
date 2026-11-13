<script lang="ts">
  // The Move tools' transform box (docs/specs/image-transform-handles.md
  // AC-01): the outline turned with the transform, four corner and four
  // edge handles and the pivot, drawn in viewport space at the same size at
  // every zoom; during a drag it follows the dragged box (AC-23). Pointer
  // input goes to the stage (`tools/transform-drag.ts`), so this layer
  // takes no events.
  import { live, tool, view } from '../../stores/editor.svelte';
  import { transformBox } from '../../core/transform';
  import { corners, handles, pivotPoint, type BoxState } from '../../tools/transform-box';

  /** Edge of a handle square, CSS px. */
  const SIZE = 8;
  /** Diameter of the pivot marker, CSS px. */
  const PIVOT = 10;

  const box = $derived.by((): BoxState | null =>
    tool.kind === 'move_selection' || tool.kind === 'move_mask' ? transformBox() : null,
  );
  // Live from the first drag on, also before the core has seen it.
  const state = $derived(live.transform === null && live.transformDrag === null ? 'idle' : 'live');
  const toScreen = (p: readonly [number, number]): [number, number] => [
    view.panX + p[0] * view.zoom,
    view.panY + p[1] * view.zoom,
  ];
  const outline = $derived(
    box === null
      ? ''
      : corners(box)
          .map((c) => toScreen(c).join(','))
          .join(' '),
  );
  const marks = $derived(
    box === null
      ? []
      : handles(box).map((h) => ({
          id:
            h.hit.kind === 'corner'
              ? `corner${h.hit.corner}`
              : h.hit.kind === 'edge'
                ? h.hit.edge
                : '',
          at: toScreen(h.at),
        })),
  );
  const pivot = $derived(box === null ? null : toScreen(pivotPoint(box)));
</script>

{#if box !== null}
  <svg
    class="pointer-events-none absolute inset-0 h-full w-full overflow-visible"
    aria-hidden="true"
    data-transform-box={state}
  >
    <polygon points={outline} fill="none" class="stroke-pc-accent" stroke-width="1" />
  </svg>
  {#each marks as m (m.id)}
    <div
      data-transform-handle={m.id}
      class="pointer-events-none absolute rounded-[2px] border border-pc-bg bg-pc-accent ring-1 ring-pc-bg"
      style="left: {m.at[0] - SIZE / 2}px; top: {m.at[1] -
        SIZE / 2}px; width: {SIZE}px; height: {SIZE}px;"
    ></div>
  {/each}
  {#if pivot !== null}
    <div
      data-transform-pivot
      class="pointer-events-none absolute rounded-full border-2 border-pc-accent bg-pc-bg/40"
      style="left: {pivot[0] - PIVOT / 2}px; top: {pivot[1] -
        PIVOT / 2}px; width: {PIVOT}px; height: {PIVOT}px;"
    ></div>
  {/if}
{/if}
