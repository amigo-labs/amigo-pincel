<script lang="ts">
  // The color picker both modes open from a swatch (color-picker spec):
  // saturation / value area, hue and alpha sliders, a hex field, R / G / B
  // / A fields and the recent colors. CSS gradients only — no `<canvas>`,
  // so nothing can ever be read back from it (AGENTS.md §9). Every edit
  // goes out through `onChange` at once, so the swatch and the tools
  // follow a drag live (AC-08). The parent owns the color; the picker
  // keeps hue and saturation of its own so black and grey don't reset
  // them (AC-11).
  import {
    clampByte,
    hsvToRgb,
    parseHex,
    rgbToHsv,
    sameColor,
    toHex,
    type Hsv,
    type Rgba8,
  } from '../../color';
  import { t } from '../../i18n/index.svelte';

  interface Props {
    value: Rgba8;
    recent: readonly Rgba8[];
    onChange: (c: Rgba8) => void;
  }
  const { value, recent, onChange }: Props = $props();

  // Hue / saturation / value the picker shows; re-derived from `value`
  // whenever it changes from outside, keeping hue and saturation where
  // the color has none.
  let hsv = $state<Hsv>({ h: 0, s: 0, v: 0 });
  let last: Rgba8 | null = null;
  $effect.pre(() => {
    const v = value;
    if (last !== null && sameColor(v, last)) return;
    last = v;
    hsv = rgbToHsv(v, { h: hsv.h, s: hsv.s });
  });

  function emit(c: Rgba8): void {
    last = c;
    onChange(c);
  }

  function setHsv(h: number, s: number, v: number): void {
    hsv = { h, s, v };
    emit({ ...hsvToRgb(h, s, v), a: value.a });
  }

  function setRgba(c: Rgba8): void {
    hsv = rgbToHsv(c, { h: hsv.h, s: hsv.s });
    emit(c);
  }

  // Saturation / value area: pointer drag with capture (mouse, touch, pen).
  let area: HTMLElement;
  let dragPointer: number | null = null;

  function fromPointer(e: PointerEvent): void {
    const r = area.getBoundingClientRect();
    const s = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const v = 1 - Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
    setHsv(hsv.h, s, v);
  }

  function onAreaDown(e: PointerEvent): void {
    if (dragPointer !== null || (e.pointerType === 'mouse' && e.button !== 0)) return;
    e.preventDefault();
    area.focus();
    area.setPointerCapture(e.pointerId);
    dragPointer = e.pointerId;
    fromPointer(e);
  }

  function onAreaMove(e: PointerEvent): void {
    if (e.pointerId === dragPointer) fromPointer(e);
  }

  function onAreaUp(e: PointerEvent): void {
    if (e.pointerId === dragPointer) dragPointer = null;
  }

  // Arrow keys: one step, ten with Shift (AC-10; saturation / value in %).
  function onAreaKey(e: KeyboardEvent): void {
    const step = (e.shiftKey ? 10 : 1) / 100;
    const ds = e.key === 'ArrowRight' ? step : e.key === 'ArrowLeft' ? -step : 0;
    const dv = e.key === 'ArrowUp' ? step : e.key === 'ArrowDown' ? -step : 0;
    if (ds === 0 && dv === 0) return;
    e.preventDefault();
    const clamp = (x: number) => Math.round(Math.min(1, Math.max(0, x)) * 100) / 100;
    setHsv(hsv.h, clamp(hsv.s + ds), clamp(hsv.v + dv));
  }

  // Native range inputs step by 1; Shift + arrow steps by 10 (AC-10).
  function shiftStep(e: KeyboardEvent, current: number, max: number, set: (v: number) => void) {
    if (!e.shiftKey) return;
    const dir =
      e.key === 'ArrowRight' || e.key === 'ArrowUp'
        ? 1
        : e.key === 'ArrowLeft' || e.key === 'ArrowDown'
          ? -1
          : 0;
    if (dir === 0) return;
    e.preventDefault();
    set(Math.min(max, Math.max(0, current + dir * 10)));
  }

  // Text fields commit on Enter or blur (`change`); Escape in the popover
  // discards what was typed (AC-18). Invalid hex is dropped (AC-06).
  let hexText = $state('');
  let hexEditing = $state(false);
  $effect.pre(() => {
    const shown = toHex(value, value.a !== 255);
    if (!hexEditing) hexText = shown;
  });

  function commitHex(): void {
    hexEditing = false;
    const p = parseHex(hexText);
    if (p) setRgba({ r: p.r, g: p.g, b: p.b, a: p.a ?? value.a });
    hexText = toHex(value, value.a !== 255);
  }

  function commitChannel(key: 'r' | 'g' | 'b' | 'a', input: HTMLInputElement): void {
    const n = input.valueAsNumber;
    if (Number.isNaN(n)) {
      input.value = String(value[key]);
      return;
    }
    const v = clampByte(n);
    input.value = String(v);
    setRgba({ ...value, [key]: v });
  }

  const hueColor = $derived(`hsl(${hsv.h} 100% 50%)`);
  const rgb = $derived(`rgb(${value.r} ${value.g} ${value.b})`);
  const CHANNELS = [
    { key: 'r', label: 'color.red' },
    { key: 'g', label: 'color.green' },
    { key: 'b', label: 'color.blue' },
    { key: 'a', label: 'color.alphaChannel' },
  ] as const;
</script>

<div class="flex w-60 flex-col gap-2.5 text-xs text-pc-text">
  <div
    bind:this={area}
    class="sv relative h-36 w-full cursor-crosshair touch-none rounded outline-none focus-visible:ring-2 focus-visible:ring-pc-accent"
    style:background-color={hueColor}
    role="slider"
    tabindex="0"
    aria-label={t('color.satValue')}
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow={Math.round(hsv.s * 100)}
    aria-valuetext={t('color.satValueText', {
      s: Math.round(hsv.s * 100),
      v: Math.round(hsv.v * 100),
    })}
    onpointerdown={onAreaDown}
    onpointermove={onAreaMove}
    onpointerup={onAreaUp}
    onpointercancel={onAreaUp}
    onlostpointercapture={onAreaUp}
    onkeydown={onAreaKey}
  >
    <span
      class="pointer-events-none absolute h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-[0_0_0_1px_rgb(0_0_0/0.6)]"
      style:left="{hsv.s * 100}%"
      style:top="{(1 - hsv.v) * 100}%"
    ></span>
  </div>

  <input
    type="range"
    min="0"
    max="360"
    step="1"
    value={Math.round(hsv.h)}
    oninput={(e) => setHsv(e.currentTarget.valueAsNumber, hsv.s, hsv.v)}
    onkeydown={(e) => shiftStep(e, Math.round(hsv.h), 360, (h) => setHsv(h, hsv.s, hsv.v))}
    class="track hue"
    aria-label={t('color.hue')}
  />
  <input
    type="range"
    min="0"
    max="255"
    step="1"
    value={value.a}
    oninput={(e) => setRgba({ ...value, a: e.currentTarget.valueAsNumber })}
    onkeydown={(e) => shiftStep(e, value.a, 255, (a) => setRgba({ ...value, a }))}
    class="track alpha"
    style:--rgb={rgb}
    aria-label={t('color.alphaSlider')}
  />

  <label class="flex items-center gap-2">
    <span class="w-8 text-pc-muted">{t('color.hex')}</span>
    <input
      type="text"
      class="opt-field min-w-0 flex-1 font-mono"
      spellcheck="false"
      maxlength="9"
      bind:value={hexText}
      oninput={() => (hexEditing = true)}
      onchange={commitHex}
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          commitHex();
        }
      }}
      aria-label={t('color.hexField')}
    />
  </label>

  <div class="grid grid-cols-4 gap-1.5">
    {#each CHANNELS as ch (ch.key)}
      <label class="flex flex-col gap-0.5">
        <span class="text-center text-pc-muted">{ch.key.toUpperCase()}</span>
        <input
          type="number"
          min="0"
          max="255"
          step="1"
          value={value[ch.key]}
          class="opt-field w-full text-right tabular-nums"
          onchange={(e) => commitChannel(ch.key, e.currentTarget)}
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              commitChannel(ch.key, e.currentTarget);
            }
          }}
          aria-label={t(ch.label)}
        />
      </label>
    {/each}
  </div>

  <div class="flex flex-col gap-1">
    <span class="text-pc-muted">{t('color.recent')}</span>
    <div class="flex min-h-5 flex-wrap gap-1" role="group" aria-label={t('color.recent')}>
      {#each recent as c (toHex(c, true))}
        <button
          type="button"
          class="chip h-5 w-5 rounded-sm border border-pc-border"
          style:--chip={`rgb(${c.r} ${c.g} ${c.b} / ${c.a / 255})`}
          title={toHex(c, c.a !== 255)}
          aria-label={toHex(c, c.a !== 255)}
          onclick={() => setRgba(c)}
        ></button>
      {/each}
    </div>
  </div>
</div>

<style>
  .sv {
    background-image:
      linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent);
  }
  .track {
    appearance: none;
    width: 100%;
    height: 0.75rem;
    border-radius: 0.375rem;
    cursor: pointer;
    outline: none;
  }
  .track:focus-visible {
    box-shadow: 0 0 0 2px var(--color-pc-accent);
  }
  .hue {
    background: linear-gradient(
      to right,
      #f00 0%,
      #ff0 16.67%,
      #0f0 33.33%,
      #0ff 50%,
      #00f 66.67%,
      #f0f 83.33%,
      #f00 100%
    );
  }
  .alpha {
    background:
      linear-gradient(to right, transparent, var(--rgb)),
      repeating-conic-gradient(#999 0 25%, #fff 0 50%) 0 0 / 8px 8px;
  }
  .track::-webkit-slider-thumb {
    appearance: none;
    width: 0.875rem;
    height: 0.875rem;
    border-radius: 9999px;
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.6);
    background: transparent;
  }
  .track::-moz-range-thumb {
    width: 0.625rem;
    height: 0.625rem;
    border-radius: 9999px;
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.6);
    background: transparent;
  }
  .chip {
    background:
      linear-gradient(var(--chip), var(--chip)),
      repeating-conic-gradient(#999 0 25%, #fff 0 50%) 0 0 / 8px 8px;
  }
</style>
