<script lang="ts">
  // One ruler of View ▸ Rulers (docs/specs/image-view-aids.md AC-02 – AC-04):
  // labelled ticks in the chosen unit at the current zoom and pan, and a
  // marker at the pointer. DOM elements, so tests measure them without
  // reading canvas pixels.
  import { locale } from '../../../../lib/i18n/index.svelte';
  import { formatUnitValue, rulerTicks, type ViewUnit } from '../../view-units';

  interface Props {
    axis: 'h' | 'v';
    /** Ruler length in CSS px (the canvas area's width or height). */
    length: number;
    /** Screen offset of image coordinate 0 along the axis. */
    pan: number;
    zoom: number;
    unit: ViewUnit;
    dpi: number;
    /** The pointer's screen offset along the axis, or `null`. */
    marker: number | null;
  }
  const { axis, length, pan, zoom, unit, dpi, marker }: Props = $props();

  const ticks = $derived(length > 0 ? rulerTicks(length, pan, zoom, unit, dpi) : []);
  const h = $derived(axis === 'h');
</script>

<div
  class="relative shrink-0 overflow-hidden border-pc-border bg-pc-panel text-[10px] leading-none text-pc-muted select-none"
  class:h-5={h}
  class:w-full={h}
  class:border-b={h}
  class:w-5={!h}
  class:h-full={!h}
  class:border-r={!h}
  data-testid="ruler-{axis}"
  aria-hidden="true"
>
  {#each ticks as tick (tick.value)}
    <div
      class="absolute bg-pc-muted"
      style={h
        ? `left: ${tick.pos}px; bottom: 0; width: 1px; height: 100%;`
        : `top: ${tick.pos}px; right: 0; height: 1px; width: 100%;`}
      data-testid="ruler-{axis}-tick"
      data-value={tick.value}
    ></div>
    <span
      class="absolute whitespace-nowrap"
      style={h
        ? `left: ${tick.pos + 2}px; top: 2px;`
        : `top: ${tick.pos + 2}px; left: 2px; writing-mode: vertical-lr;`}
      data-testid="ruler-{axis}-label"
      data-value={tick.value}>{formatUnitValue(tick.value, unit, locale())}</span
    >
  {/each}
  {#if marker !== null}
    <div
      class="absolute bg-pc-accent"
      style={h
        ? `left: ${marker}px; top: 0; width: 1px; height: 100%;`
        : `top: ${marker}px; left: 0; height: 1px; width: 100%;`}
      data-testid="ruler-{axis}-marker"
    ></div>
  {/if}
</div>
