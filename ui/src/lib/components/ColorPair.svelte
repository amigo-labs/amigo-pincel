<script lang="ts">
  // Foreground / background colors, shared by both modes (spec §9.2):
  // overlapping swatches with swap (X) and reset (D), a hex field for the
  // foreground and, where the mode paints with alpha, an alpha slider.
  import Icon from './icons/Icon.svelte';
  import { hint } from '../shell/shortcuts';

  interface Props {
    /** `#rrggbb`. */
    fg: string;
    bg: string;
    onFg: (hex: string) => void;
    onBg: (hex: string) => void;
    onSwap: () => void;
    onReset: () => void;
    /** Foreground alpha 0–255; omit to hide the slider. */
    alpha?: number;
    onAlpha?: (alpha: number) => void;
  }
  const { fg, bg, onFg, onBg, onSwap, onReset, alpha, onAlpha }: Props = $props();

  // The hex field mirrors the foreground; only a full #RRGGBB commits.
  let hexText = $state('');
  let hexFocused = $state(false);
  $effect(() => {
    if (!hexFocused) hexText = fg;
  });

  function onHexInput(): void {
    const hex = /^#?([0-9a-f]{6})$/i.exec(hexText.trim())?.[1];
    if (hex !== undefined) onFg(`#${hex.toLowerCase()}`);
  }
</script>

<div class="flex items-center gap-3">
  <div class="relative h-14 w-14 shrink-0">
    <input
      type="color"
      value={bg}
      oninput={(e) => onBg(e.currentTarget.value)}
      class="swatch absolute right-0 bottom-0 h-9 w-9"
      aria-label="Background color"
      title="Background color"
    />
    <input
      type="color"
      value={fg}
      oninput={(e) => onFg(e.currentTarget.value)}
      class="swatch absolute top-0 left-0 z-10 h-9 w-9"
      aria-label="Foreground color"
      title="Foreground color"
    />
    <button
      type="button"
      class="absolute -top-0.5 -right-0.5 rounded p-0.5 text-pc-muted hover:text-pc-text"
      title={`Swap colors (${hint('swapColors')})`}
      aria-label="Swap foreground and background colors"
      onclick={onSwap}
    >
      <Icon name="swap" size={13} />
    </button>
    <button
      type="button"
      class="absolute -bottom-0.5 -left-0.5 flex h-4 w-4 items-end"
      title={`Default colors (${hint('resetColors')})`}
      aria-label="Reset to default colors"
      onclick={onReset}
    >
      <span class="absolute bottom-0 left-1 h-2.5 w-2.5 border border-pc-muted bg-white"></span>
      <span class="absolute bottom-1 left-0 h-2.5 w-2.5 border border-pc-muted bg-black"></span>
    </button>
  </div>
  <div class="flex min-w-0 flex-col gap-1.5 text-xs text-pc-muted">
    <label class="flex items-center gap-1.5">
      Hex
      <input
        type="text"
        bind:value={hexText}
        oninput={onHexInput}
        onfocus={() => (hexFocused = true)}
        onblur={() => {
          hexFocused = false;
          hexText = fg;
        }}
        spellcheck="false"
        maxlength="7"
        class="opt-field w-20 font-mono"
        aria-label="Foreground color hex value"
      />
    </label>
    {#if alpha !== undefined && onAlpha}
      <label class="flex items-center gap-1.5" title="Foreground alpha">
        α
        <input
          type="range"
          min="0"
          max="255"
          value={alpha}
          oninput={(e) => onAlpha(Number(e.currentTarget.value))}
          class="w-20"
          aria-label="Foreground alpha"
        />
        <span class="w-7 text-right tabular-nums">{alpha}</span>
      </label>
    {/if}
  </div>
</div>

<style>
  .swatch {
    cursor: pointer;
    border-radius: 0.25rem;
    border: 2px solid var(--color-pc-panel);
    outline: 1px solid var(--color-pc-border);
    padding: 0;
    background: transparent;
  }
  .swatch::-webkit-color-swatch-wrapper {
    padding: 0;
  }
  .swatch::-webkit-color-swatch {
    border: none;
    border-radius: 0.125rem;
  }
  .swatch::-moz-color-swatch {
    border: none;
    border-radius: 0.125rem;
  }
</style>
