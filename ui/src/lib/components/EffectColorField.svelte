<script lang="ts">
  import { untrack } from 'svelte';
  import { formatHexColor, parseHexColor } from '../effects/catalog';
  import { t } from '../i18n/index.svelte';

  // The effect panel's colour parameter (outline-effect AC-01, AC-11): a
  // native colour input for RGB plus a hex text field that carries alpha.
  // The value is `0xRRGGBBAA`; only `#RRGGBB` / `#RRGGBBAA` text commits,
  // anything else marks the field invalid and keeps the last valid value.
  // Until `color-picker.md` builds Pincel's own picker, this is the
  // control every colour parameter uses.
  let {
    value,
    label,
    onchange,
  }: {
    /** `0xRRGGBBAA`. */
    value: number;
    /** Already translated. */
    label: string;
    onchange: (rgba: number) => void;
  } = $props();

  const uid = $props.id();
  const hex = $derived(formatHexColor(value));
  const rgb = $derived(hex.slice(0, 7));
  const alpha = $derived(value % 256);

  // The text follows the value: a new value from elsewhere (colour input,
  // Reset) replaces it, while text that already spells the value (typed
  // `#RRGGBB` on the way to `#RRGGBBAA`, upper case) stays as typed.
  let text = $state('');
  let invalid = $state(false);
  $effect(() => {
    const next = hex;
    untrack(() => {
      if (parseHexColor(text) !== value) {
        text = next;
        invalid = false;
      }
    });
  });

  function onText(next: string): void {
    text = next;
    const rgba = parseHexColor(next);
    invalid = rgba === null;
    if (rgba !== null && rgba !== value) onchange(rgba);
  }

  function onPicker(next: string): void {
    const rgba = parseHexColor(next);
    if (rgba !== null) onchange(rgba - 255 + alpha);
  }
</script>

<div class="flex flex-col gap-1">
<div class="flex items-center justify-between gap-2 text-xs text-pc-text">
  <span>{label}</span>
  <span class="flex items-center gap-2">
    <input
      type="color"
      value={rgb}
      oninput={(e) => onPicker(e.currentTarget.value)}
      class="swatch h-6 w-8"
      aria-label={label}
      title={label}
    />
    <input
      type="text"
      value={text}
      oninput={(e) => onText(e.currentTarget.value)}
      onblur={() => {
        if (!invalid) text = hex;
      }}
      spellcheck="false"
      maxlength="9"
      class="w-24 rounded border bg-pc-bg px-1 py-0.5 font-mono {invalid
        ? 'border-red-500'
        : 'border-pc-border'}"
      aria-label={t('effectDlg.hex', { label })}
      aria-invalid={invalid ? 'true' : 'false'}
      aria-describedby={invalid ? `${uid}-error` : undefined}
    />
  </span>
</div>
{#if invalid}
  <p id="{uid}-error" class="text-right text-xs text-red-300" role="status">
    {t('effectDlg.hexInvalid')}
  </p>
{/if}
</div>

<style>
  .swatch {
    cursor: pointer;
    border-radius: 0.25rem;
    border: 1px solid var(--color-pc-border);
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
