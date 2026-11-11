<script lang="ts">
  // A labelled row of color swatches (color-picker AC-25 – AC-29): Image
  // mode's recent colors and default palette in its Colors panel. A left
  // click picks the color for the foreground; a right click, a touch or pen
  // long press (the tool rail's gesture) or the context-menu key picks it
  // for the background, and the browser's context menu stays closed.
  // A named swatch (a `.gpl` entry) shows its name next to its hex value;
  // with `maxRows` the grid stops growing at that many rows and scrolls
  // vertically (image-palette-gradient AC-08, AC-19).
  import { onDestroy } from 'svelte';
  import { toHex, type Rgba8 } from '../../color';
  import { t } from '../../i18n/index.svelte';
  import { LONG_PRESS_MS, LONG_PRESS_SLOP } from '../layout/tools';

  interface Props {
    label: string;
    colors: readonly Rgba8[];
    /** Names lined up with `colors`; `undefined` where a swatch has none. */
    names?: readonly (string | undefined)[];
    /** Rows shown before the grid scrolls vertically; unlimited without. */
    maxRows?: number;
    onPick: (c: Rgba8, target: 'fg' | 'bg') => void;
  }
  const { label, colors, names, maxRows, onPick }: Props = $props();

  const hex = (c: Rgba8): string => toHex(c, c.a !== 255);
  const name = (c: Rgba8, i: number): string => {
    const n = names?.[i];
    return n ? t('palette.swatchNamed', { name: n, hex: hex(c) }) : hex(c);
  };

  // The height of `maxRows` rows of swatches, measured from the first one
  // (swatches stretch with the panel width).
  let grid = $state<HTMLDivElement | null>(null);
  let maxHeight = $state<number | null>(null);
  $effect(() => {
    const el = grid;
    void colors.length;
    if (!el || maxRows === undefined) return;
    const rows = maxRows;
    const measure = (): void => {
      const first = el.firstElementChild;
      if (!(first instanceof HTMLElement)) return;
      const gap = Number.parseFloat(getComputedStyle(el).rowGap) || 0;
      maxHeight = rows * first.offsetHeight + (rows - 1) * gap;
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  });

  // A touch / pen hold on one swatch; `fired` once it picked the background.
  let press: {
    pointerId: number;
    x: number;
    y: number;
    fired: boolean;
    timer: ReturnType<typeof setTimeout> | null;
  } | null = null;
  let swallowClickUntil = 0;

  function cancelPress(): void {
    if (press?.timer) clearTimeout(press.timer);
    press = null;
  }
  onDestroy(cancelPress);

  function onPointerDown(e: PointerEvent, c: Rgba8): void {
    if (e.pointerType === 'mouse' || !e.isPrimary || e.button !== 0) return;
    cancelPress();
    const pointerId = e.pointerId;
    press = {
      pointerId,
      x: e.clientX,
      y: e.clientY,
      fired: false,
      timer: setTimeout(() => {
        if (press?.pointerId !== pointerId) return;
        press.timer = null;
        press.fired = true;
        onPick(c, 'bg');
      }, LONG_PRESS_MS),
    };
  }

  function onPointerMove(e: PointerEvent): void {
    if (!press || press.fired || e.pointerId !== press.pointerId) return;
    if (Math.hypot(e.clientX - press.x, e.clientY - press.y) > LONG_PRESS_SLOP) cancelPress();
  }

  function onPointerEnd(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointerId) return;
    if (press.fired) swallowClickUntil = performance.now() + 400;
    cancelPress();
  }

  function onClick(c: Rgba8): void {
    if (performance.now() < swallowClickUntil) return;
    onPick(c, 'fg');
  }

  function onContextMenu(e: MouseEvent, c: Rgba8): void {
    e.preventDefault();
    // A touch or pen long press fires its own `contextmenu`; the hold
    // above owns that gesture.
    if (press !== null || performance.now() < swallowClickUntil) return;
    onPick(c, 'bg');
  }
</script>

<div class="flex flex-col gap-1 text-xs">
  <span class="text-pc-muted">{label}</span>
  <div
    bind:this={grid}
    class="grid min-h-5 grid-cols-[repeat(auto-fill,minmax(1.125rem,1fr))] gap-1"
    class:capped={maxRows !== undefined}
    style:max-height={maxRows !== undefined && maxHeight !== null ? `${maxHeight}px` : undefined}
    role="group"
    aria-label={label}
  >
    {#each colors as c, i (`${i}-${hex(c)}`)}
      <button
        type="button"
        class="chip aspect-square min-h-4 min-w-4 rounded-sm border border-pc-border"
        style:--chip={`rgb(${c.r} ${c.g} ${c.b} / ${c.a / 255})`}
        title={name(c, i)}
        aria-label={name(c, i)}
        onclick={() => onClick(c)}
        oncontextmenu={(e) => onContextMenu(e, c)}
        onpointerdown={(e) => onPointerDown(e, c)}
        onpointermove={onPointerMove}
        onpointerup={onPointerEnd}
        onpointercancel={onPointerEnd}
      ></button>
    {/each}
  </div>
</div>

<style>
  .capped {
    overflow-x: hidden;
    overflow-y: auto;
  }
  .chip {
    background:
      linear-gradient(var(--chip), var(--chip)),
      repeating-conic-gradient(#999 0 25%, #fff 0 50%) 0 0 / 8px 8px;
  }
</style>
