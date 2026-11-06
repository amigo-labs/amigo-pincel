<script lang="ts">
  // The Image canvas area with View ▸ Rulers around it
  // (docs/specs/image-view-aids.md): the rulers take their strips out of the
  // area, so Fit fits beside them (AC-05), and the area tracks the pointer
  // the rulers and the status bar follow (AC-04, AC-10, AC-24).
  import { onMount } from 'svelte';
  import { editor, view } from '../../stores/editor.svelte';
  import { viewAids } from '../../stores/view-aids.svelte';
  import { documentDpi } from '../../core/controller';
  import MainCanvas from './MainCanvas.svelte';
  import Ruler from './Ruler.svelte';

  let area: HTMLDivElement;
  let areaSize = $state({ w: 0, h: 0 });
  // The active document's resolution, read again on every tab switch and
  // document change (AC-19, AC-23).
  const dpi = $derived.by(() => {
    void editor.handle;
    void editor.revision;
    return documentDpi();
  });

  // Touch has no hover: a touch shows the position only while it is down,
  // following the first contact (AC-24).
  let touches: readonly number[] = [];
  let firstTouch: number | null = null;

  // The tracked pointer in client coordinates: the area itself moves when
  // the rulers or the panels toggle under a resting pointer.
  let client: { x: number; y: number } | null = null;

  function track(e: PointerEvent): void {
    client = { x: e.clientX, y: e.clientY };
    place();
  }

  /** The tracked pointer inside the area, or hidden outside it. */
  function place(): void {
    if (client === null) return;
    const rect = area.getBoundingClientRect();
    const sx = client.x - rect.left;
    const sy = client.y - rect.top;
    // A tool's pointer capture keeps the moves of a pressed pointer coming
    // after it left the area and holds back its pointerleave.
    if (sx < 0 || sy < 0 || sx >= rect.width || sy >= rect.height) {
      viewAids.pointer = null;
      return;
    }
    // Kept in screen space: the markers sit at it and the status bar maps it
    // through the view current at render time, so a zoom, a pan (whose
    // move handler runs after this capture handler) or a tab switch never
    // leaves them stale.
    viewAids.pointer = { x: sx, y: sy };
  }

  function onDown(e: PointerEvent): void {
    if (e.pointerType === 'touch') {
      touches = [...touches, e.pointerId];
      firstTouch ??= e.pointerId;
      if (e.pointerId !== firstTouch) return;
    }
    track(e);
  }

  function onMove(e: PointerEvent): void {
    if (e.pointerType === 'touch' && e.pointerId !== firstTouch) return;
    track(e);
  }

  function onUp(e: PointerEvent): void {
    if (e.pointerType !== 'touch') return;
    touches = touches.filter((id) => id !== e.pointerId);
    if (touches.length === 0) {
      firstTouch = null;
      hide();
    }
  }

  function onLeave(e: PointerEvent): void {
    if (e.pointerType !== 'touch') hide();
  }

  function hide(): void {
    client = null;
    viewAids.pointer = null;
  }

  onMount(() => {
    const ro = new ResizeObserver(() => {
      areaSize = { w: area.clientWidth, h: area.clientHeight };
      // The rulers, the panels or the window moved the area under a
      // resting pointer.
      place();
    });
    ro.observe(area);
    return () => {
      ro.disconnect();
      hide();
    };
  });

</script>

<div class="flex h-full w-full flex-col">
  {#if viewAids.showRulers}
    <div class="flex shrink-0">
      <div
        class="h-5 w-5 shrink-0 border-r border-b border-pc-border bg-pc-panel"
        data-testid="ruler-corner"
      ></div>
      <div class="min-w-0 flex-1">
        <Ruler
          axis="h"
          length={areaSize.w}
          pan={view.panX}
          zoom={view.zoom}
          unit={viewAids.units}
          {dpi}
          marker={viewAids.pointer?.x ?? null}
        />
      </div>
    </div>
  {/if}
  <div class="flex min-h-0 flex-1">
    {#if viewAids.showRulers}
      <Ruler
        axis="v"
        length={areaSize.h}
        pan={view.panY}
        zoom={view.zoom}
        unit={viewAids.units}
        {dpi}
        marker={viewAids.pointer?.y ?? null}
      />
    {/if}
    <div
      bind:this={area}
      class="relative min-w-0 flex-1"
      data-testid="canvas-area"
      role="presentation"
      onpointerdowncapture={onDown}
      onpointermovecapture={onMove}
      onpointerupcapture={onUp}
      onpointercancelcapture={onUp}
      onpointerleave={onLeave}
    >
      <MainCanvas />
    </div>
  </div>
</div>
