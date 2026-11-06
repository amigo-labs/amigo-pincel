<script lang="ts">
  // A labelled row of color swatches (color-picker AC-25 – AC-29): Image
  // mode's recent colors and default palette in its Colors panel. A left
  // click picks the color for the foreground; a right click, a touch or pen
  // long press (the tool rail's gesture) or the context-menu key picks it
  // for the background, and the browser's context menu stays closed.
  import { onDestroy } from 'svelte';
  import { toHex, type Rgba8 } from '../../color';
  import { LONG_PRESS_MS, LONG_PRESS_SLOP } from '../layout/tools';

  interface Props {
    label: string;
    colors: readonly Rgba8[];
    onPick: (c: Rgba8, target: 'fg' | 'bg') => void;
  }
  const { label, colors, onPick }: Props = $props();

  const name = (c: Rgba8): string => toHex(c, c.a !== 255);

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
    class="grid min-h-5 grid-cols-[repeat(auto-fill,minmax(1.125rem,1fr))] gap-1"
    role="group"
    aria-label={label}
  >
    {#each colors as c, i (`${i}-${name(c)}`)}
      <button
        type="button"
        class="chip aspect-square min-h-4 min-w-4 rounded-sm border border-pc-border"
        style:--chip={`rgb(${c.r} ${c.g} ${c.b} / ${c.a / 255})`}
        title={name(c)}
        aria-label={name(c)}
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
  .chip {
    background:
      linear-gradient(var(--chip), var(--chip)),
      repeating-conic-gradient(#999 0 25%, #fff 0 50%) 0 0 / 8px 8px;
  }
</style>
