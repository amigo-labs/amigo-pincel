<script lang="ts">
  // Handles of the live object (docs/specs/image-live-objects-ui.md AC-01,
  // AC-05, AC-18): drawn in viewport space over the canvas and the stage, the
  // same size at every zoom. Pointer input goes to the stage; hit-testing
  // lives in `tools/live-handles.ts`, so this layer takes no events.
  import { live, view } from '../../stores/editor.svelte';
  import { handlesOf } from '../../tools/live-handles';

  /** Edge of a handle square, CSS px. */
  const SIZE = 9;

  const obj = $derived(live.object?.type === 'text' ? null : live.object);
  const toScreen = (p: [number, number]): [number, number] => [
    view.panX + p[0] * view.zoom,
    view.panY + p[1] * view.zoom,
  ];
  const marks = $derived(
    obj === null
      ? []
      : handlesOf(obj, view.zoom).map((h) => ({
          id:
            h.hit.kind === 'box'
              ? h.hit.handle
              : h.hit.kind === 'end'
                ? `end${h.hit.index}`
                : h.hit.kind,
          at: toScreen(h.at),
        })),
  );
  // A Line or Gradient shows its body as a thin guide between its ends.
  const guide = $derived(
    obj === null || (obj.type === 'shape' && obj.shape !== 'line')
      ? null
      : { a: toScreen(obj.points[0]), b: toScreen(obj.points[1]) },
  );
</script>

{#if obj !== null}
  <svg class="pointer-events-none absolute inset-0 h-full w-full overflow-visible" aria-hidden="true">
    {#if guide}
      <line
        x1={guide.a[0]}
        y1={guide.a[1]}
        x2={guide.b[0]}
        y2={guide.b[1]}
        class="stroke-pc-accent"
        stroke-width="1"
        stroke-dasharray="4 3"
      />
    {/if}
  </svg>
  {#each marks as m (m.id)}
    <div
      data-live-handle={m.id}
      class="pointer-events-none absolute border border-pc-bg bg-pc-accent shadow-[0_0_0_1px_rgba(0,0,0,0.6)] {m.id ===
      'grip'
        ? 'rounded-full'
        : 'rounded-[2px]'}"
      style="left: {m.at[0] - SIZE / 2}px; top: {m.at[1] - SIZE / 2}px; width: {SIZE}px; height: {SIZE}px;"
    ></div>
  {/each}
{/if}
