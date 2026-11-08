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
                : h.hit.kind === 'ctrl'
                  ? `ctrl${h.hit.index}`
                  : h.hit.kind,
          at: toScreen(h.at),
        })),
  );
  // A Line or Gradient shows its body as a thin guide between its ends; a
  // curve's guide follows the curve (docs/specs/image-shapes-curves.md).
  const guide = $derived.by(() => {
    if (obj === null || (obj.type === 'shape' && obj.shape !== 'line')) return null;
    const pts =
      obj.type === 'shape' && obj.polyline.length > 1 ? obj.polyline : [obj.points[0], obj.points[1]];
    return pts.map((p) => toScreen(p).join(',')).join(' ');
  });
</script>

{#if obj !== null}
  <svg class="pointer-events-none absolute inset-0 h-full w-full overflow-visible" aria-hidden="true">
    {#if guide}
      <polyline
        points={guide}
        fill="none"
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
