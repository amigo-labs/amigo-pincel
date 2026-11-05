<script lang="ts">
  import { t } from '../i18n/index.svelte';
  import { wrapAngle } from '../effects/catalog';

  // An angle parameter (effect-catalog-v2 AC-09): a round dial whose handle
  // points in the angle's direction, plus a number field in degrees. The
  // effects' convention: 0° points right, 90° points down (image y axis).
  let {
    value,
    label,
    onchange,
  }: {
    value: number;
    label: string;
    onchange: (deg: number) => void;
  } = $props();

  const SIZE = 56;
  const R = SIZE / 2 - 6;
  let dial: SVGSVGElement;
  let dragging: number | null = null;

  const shown = $derived(wrapAngle(value));
  const hx = $derived(SIZE / 2 + R * Math.cos((shown * Math.PI) / 180));
  const hy = $derived(SIZE / 2 + R * Math.sin((shown * Math.PI) / 180));

  /** Direction from the dial's centre to the pointer, whole degrees 0–359. */
  function angleAt(e: PointerEvent): number {
    const rect = dial.getBoundingClientRect();
    const dx = e.clientX - (rect.left + rect.width / 2);
    const dy = e.clientY - (rect.top + rect.height / 2);
    const deg = Math.round((Math.atan2(dy, dx) * 180) / Math.PI);
    return ((deg % 360) + 360) % 360;
  }

  function onDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    e.preventDefault();
    dial.focus();
    dragging = e.pointerId;
    dial.setPointerCapture(e.pointerId);
    onchange(angleAt(e));
  }

  function onMove(e: PointerEvent): void {
    if (dragging === e.pointerId) onchange(angleAt(e));
  }

  function onUp(e: PointerEvent): void {
    if (dragging !== e.pointerId) return;
    dragging = null;
    if (dial.hasPointerCapture(e.pointerId)) dial.releasePointerCapture(e.pointerId);
  }

  const STEPS: Record<string, number> = {
    ArrowRight: 1,
    ArrowUp: 1,
    ArrowLeft: -1,
    ArrowDown: -1,
    PageUp: 15,
    PageDown: -15,
  };

  function onKey(e: KeyboardEvent): void {
    const step = STEPS[e.key];
    let next: number | null = step === undefined ? null : Math.round(shown) + step;
    if (e.key === 'Home') next = 0;
    if (e.key === 'End') next = 359;
    if (next === null) return;
    e.preventDefault();
    onchange(((next % 360) + 360) % 360);
  }

  function onField(e: Event & { currentTarget: HTMLInputElement }, commit: boolean): void {
    const v = e.currentTarget.valueAsNumber;
    if (!Number.isFinite(v)) return;
    onchange(commit ? wrapAngle(v) : v);
  }
</script>

<span class="flex items-center gap-2">
  <svg
    bind:this={dial}
    width={SIZE}
    height={SIZE}
    viewBox="0 0 {SIZE} {SIZE}"
    class="shrink-0 cursor-pointer touch-none rounded-full focus:outline-2 focus:outline-pc-accent"
    role="slider"
    tabindex="0"
    aria-label={t('effectDlg.angleDial', { label })}
    aria-valuemin={0}
    aria-valuemax={359}
    aria-valuenow={Math.round(shown) % 360}
    aria-valuetext="{Math.round(shown) % 360}°"
    data-handle-x={hx.toFixed(2)}
    data-handle-y={hy.toFixed(2)}
    onpointerdown={onDown}
    onpointermove={onMove}
    onpointerup={onUp}
    onpointercancel={onUp}
    onkeydown={onKey}
  >
    <circle
      cx={SIZE / 2}
      cy={SIZE / 2}
      r={R}
      class="fill-pc-bg stroke-pc-border"
      stroke-width="2"
    />
    <line
      x1={SIZE / 2}
      y1={SIZE / 2}
      x2={hx}
      y2={hy}
      class="stroke-pc-accent"
      stroke-width="2"
    />
    <circle cx={hx} cy={hy} r="4" class="fill-pc-accent" />
  </svg>
  <input
    type="number"
    min="0"
    max="360"
    step="1"
    value={value}
    oninput={(e) => onField(e, false)}
    onchange={(e) => onField(e, true)}
    class="w-16 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-right tabular-nums"
    aria-label={t('effectDlg.degrees', { label })}
  />
</span>
