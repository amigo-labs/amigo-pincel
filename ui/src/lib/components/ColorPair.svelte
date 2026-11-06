<script lang="ts">
  // Foreground / background colors, shared by both modes (spec §9.2):
  // overlapping swatches with swap (X) and reset (D), a hex field and an
  // alpha slider for the foreground. A click on a swatch opens the own
  // color picker in a popover next to it (color-picker spec) — never the
  // browser's native color dialog.
  import Icon from './icons/Icon.svelte';
  import ColorPicker from './color/ColorPicker.svelte';
  import { parseHex, toHex, type Rgba8 } from '../color';
  import { withHint } from '../shell/shortcuts';
  import { t } from '../i18n/index.svelte';

  interface Props {
    fg: Rgba8;
    bg: Rgba8;
    onFg: (c: Rgba8) => void;
    onBg: (c: Rgba8) => void;
    onSwap: () => void;
    onReset: () => void;
    /** The mode's recent colors, most recent first. */
    recent: readonly Rgba8[];
  }
  const { fg, bg, onFg, onBg, onSwap, onReset, recent }: Props = $props();

  // The panel's hex field mirrors the foreground; a full #RRGGBB commits.
  let hexText = $state('');
  let hexFocused = $state(false);
  $effect(() => {
    if (!hexFocused) hexText = toHex(fg);
  });

  function onHexInput(): void {
    const p = /^#?[0-9a-f]{6}$/i.test(hexText.trim()) ? parseHex(hexText) : null;
    if (p) onFg({ r: p.r, g: p.g, b: p.b, a: fg.a });
  }

  // The open picker: which color it edits and where it shows.
  let editing = $state<'fg' | 'bg' | null>(null);
  let anchor = $state<DOMRect | null>(null);
  let swatches: HTMLElement;
  let popWidth = $state(0);
  let popHeight = $state(0);
  let winWidth = $state(0);
  let winHeight = $state(0);
  const MARGIN = 8;

  let popover = $state<HTMLElement | null>(null);

  function open(which: 'fg' | 'bg', e: MouseEvent): void {
    anchor = (e.currentTarget as HTMLElement).getBoundingClientRect();
    editing = which;
  }

  // Focus moves into the popover, so Escape and Tab act on it.
  $effect(() => {
    popover?.focus();
  });

  // Field text left uncommitted commits on its blur — but only while the
  // picker is open: Escape closes first, so its text is dropped (AC-18).
  function apply(c: Rgba8): void {
    if (editing === 'fg') onFg(c);
    else if (editing === 'bg') onBg(c);
  }

  function close(): void {
    const which = editing;
    editing = null;
    swatches.querySelector<HTMLElement>(`[data-swatch="${which}"]`)?.focus();
  }

  // Beside the swatch when there is room, else on its other side; always
  // wholly inside the window (AC-20).
  const pos = $derived.by(() => {
    if (!anchor) return { left: MARGIN, top: MARGIN };
    const right = anchor.right + MARGIN;
    const left = right + popWidth <= winWidth - MARGIN ? right : anchor.left - MARGIN - popWidth;
    const clamp = (v: number, size: number, max: number) =>
      Math.max(MARGIN, Math.min(v, max - size - MARGIN));
    return { left: clamp(left, popWidth, winWidth), top: clamp(anchor.top, popHeight, winHeight) };
  });

  function onPopKey(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    // Discards uncommitted field text: the fields go with the popover (AC-18).
    e.preventDefault();
    e.stopPropagation();
    close();
  }

  const css = (c: Rgba8) => `rgb(${c.r} ${c.g} ${c.b} / ${c.a / 255})`;
</script>

<svelte:window bind:innerWidth={winWidth} bind:innerHeight={winHeight} />

<div class="flex items-center gap-3">
  <div class="relative h-14 w-14 shrink-0" bind:this={swatches}>
    <button
      type="button"
      class="swatch absolute right-0 bottom-0 h-9 w-9"
      style:--swatch={css(bg)}
      data-swatch="bg"
      aria-label={t('color.bg')}
      aria-haspopup="dialog"
      aria-expanded={editing === 'bg'}
      title={t('color.bg')}
      onclick={(e) => open('bg', e)}
    ></button>
    <button
      type="button"
      class="swatch absolute top-0 left-0 z-10 h-9 w-9"
      style:--swatch={css(fg)}
      data-swatch="fg"
      aria-label={t('color.fg')}
      aria-haspopup="dialog"
      aria-expanded={editing === 'fg'}
      title={t('color.fg')}
      onclick={(e) => open('fg', e)}
    ></button>
    <button
      type="button"
      class="absolute -top-0.5 -right-0.5 rounded p-0.5 text-pc-muted hover:text-pc-text"
      title={withHint(t('color.swap'), 'swapColors')}
      aria-label={t('color.swapLabel')}
      onclick={onSwap}
    >
      <Icon name="swap" size={13} />
    </button>
    <button
      type="button"
      class="absolute -bottom-0.5 -left-0.5 flex h-4 w-4 items-end"
      title={withHint(t('color.reset'), 'resetColors')}
      aria-label={t('color.resetLabel')}
      onclick={onReset}
    >
      <span class="absolute bottom-0 left-1 h-2.5 w-2.5 border border-pc-muted bg-white"></span>
      <span class="absolute bottom-1 left-0 h-2.5 w-2.5 border border-pc-muted bg-black"></span>
    </button>
  </div>
  <div class="flex min-w-0 flex-col gap-1.5 text-xs text-pc-muted">
    <label class="flex items-center gap-1.5">
      {t('color.hex')}
      <input
        type="text"
        bind:value={hexText}
        oninput={onHexInput}
        onfocus={() => (hexFocused = true)}
        onblur={() => {
          hexFocused = false;
          hexText = toHex(fg);
        }}
        spellcheck="false"
        maxlength="7"
        class="opt-field w-20 font-mono"
        aria-label={t('color.hexLabel')}
      />
    </label>
    <label class="flex items-center gap-1.5" title={t('color.alpha')}>
      α
      <input
        type="range"
        min="0"
        max="255"
        value={fg.a}
        oninput={(e) => onFg({ ...fg, a: Number(e.currentTarget.value) })}
        class="w-20"
        aria-label={t('color.alpha')}
      />
      <span class="w-7 text-right tabular-nums">{fg.a}</span>
    </label>
  </div>
</div>

{#if editing}
  <!-- Backdrop: a click outside commits field text (its blur) and closes. -->
  <button
    type="button"
    class="fixed inset-0 z-[60] cursor-default"
    tabindex="-1"
    aria-label={t('color.closePicker')}
    onclick={close}
  ></button>
  <div
    class="fixed z-[61] rounded-lg border border-pc-border bg-pc-panel p-3 shadow-[0_8px_30px_rgb(0_0_0/0.5)]"
    style:left="{pos.left}px"
    style:top="{pos.top}px"
    bind:this={popover}
    bind:offsetWidth={popWidth}
    bind:offsetHeight={popHeight}
    role="dialog"
    aria-modal="true"
    aria-label={t(editing === 'fg' ? 'color.pickerFg' : 'color.pickerBg')}
    tabindex="-1"
    onkeydown={onPopKey}
  >
    <ColorPicker value={editing === 'fg' ? fg : bg} {recent} onChange={apply} />
  </div>
{/if}

<style>
  .swatch {
    cursor: pointer;
    border-radius: 0.25rem;
    border: 2px solid var(--color-pc-panel);
    outline: 1px solid var(--color-pc-border);
    padding: 0;
    background:
      linear-gradient(var(--swatch), var(--swatch)),
      repeating-conic-gradient(#999 0 25%, #fff 0 50%) 0 0 / 8px 8px;
  }
</style>
