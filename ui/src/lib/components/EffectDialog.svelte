<script lang="ts">
  import {
    defaultValues,
    formatHexColor,
    parseHexColor,
    toParams,
    type EffectDef,
  } from '../effects/catalog';
  import { effectLabel, fxText } from '../effects/labels';
  import { t } from '../i18n/index.svelte';

  // Parameter dialog for one effect / adjustment. Purely presentational:
  // the parent owns the wasm `Document`, runs `previewEffect` on every
  // `onPreview(params)`, restores the un-previewed composite on
  // `onPreview(null)` (Preview off) and commits via `onApply(params)`.
  // `onCancel` restores the un-previewed composite too. Parameters are edited as a flat
  // value map keyed by `ParamDef.key`; `toParams` orders them for the wire.
  let {
    def,
    hasSelection = false,
    foreground,
    onPreview,
    onApply,
    onCancel,
  }: {
    def: EffectDef;
    hasSelection?: boolean;
    /** The foreground colour as 0xRRGGBBAA, the default of colour params. */
    foreground?: number;
    /** `null` turns the preview off: show the image without the effect. */
    onPreview: (params: number[] | null) => void;
    onApply: (params: number[]) => void;
    onCancel: () => void;
  } = $props();

  // The parent remounts this dialog per effect (`{#key def.id}`), so the
  // initial snapshot of `def` is the only one this instance ever sees.
  // svelte-ignore state_referenced_locally
  let values = $state<Record<string, number>>(defaultValues(def, { foreground }));
  // Hex text the user typed per colour param; without an entry the field
  // shows the colour itself. Only a valid `#RRGGBB[AA]` reaches `values`;
  // anything else marks the field invalid and keeps the last good colour.
  let hexEdits = $state<Record<string, string>>({});

  function hexText(key: string, fallback: number): string {
    return hexEdits[key] ?? formatHexColor(values[key] ?? fallback);
  }

  function onHexInput(key: string, text: string) {
    hexEdits[key] = text;
    const rgba = parseHexColor(text);
    if (rgba !== null) values[key] = rgba;
  }

  /** The native input picks RGB only; the colour keeps its alpha. */
  function onSwatchInput(key: string, rgbHex: string) {
    const rgb = parseHexColor(rgbHex);
    if (rgb === null) return;
    const rgba = (rgb - 0xff + ((values[key] ?? 0xff) & 0xff)) >>> 0;
    values[key] = rgba;
    delete hexEdits[key];
  }
  let preview = $state(true);

  const params = $derived(toParams(def, values));

  // Live preview: re-run whenever a value changes while preview is on.
  // Coalesced through rAF so a slider drag renders at most once a frame.
  let rafId = 0;
  $effect(() => {
    const p = params;
    const on = preview;
    if (rafId) cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(() => {
      rafId = 0;
      // `null`, not `[]`: a parameterless effect previews with `[]`.
      onPreview(on ? p : null);
    });
    return () => {
      if (rafId) cancelAnimationFrame(rafId);
    };
  });

  function reset() {
    values = defaultValues(def, { foreground });
    hexEdits = {};
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && !(e.target instanceof HTMLTextAreaElement)) {
      e.preventDefault();
      onApply(params);
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="pointer-events-none fixed inset-0 z-20 flex items-start justify-end p-4"
  role="dialog"
  aria-modal="false"
  aria-labelledby="effect-title"
>
  <form
    class="pointer-events-auto flex w-80 max-h-full flex-col gap-3 overflow-y-auto rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-2xl"
    onsubmit={(e) => {
      e.preventDefault();
      onApply(params);
    }}
  >
    <header class="flex items-baseline justify-between">
      <h2 id="effect-title" class="text-base font-semibold">{effectLabel(def)}</h2>
      <span class="text-xs text-pc-muted">
        {hasSelection ? t('effectDlg.selection') : t('effectDlg.wholeLayer')}
      </span>
    </header>

    {#if def.params.length === 0}
      <p class="text-xs text-pc-muted">{t('effectDlg.noParams')}</p>
    {/if}

    {#each def.params as p (p.key)}
      {#if p.kind === 'number'}
        <label class="flex flex-col gap-1 text-xs text-pc-text">
          <span class="flex justify-between">
            <span>{fxText(p.label)}</span>
            <span class="tabular-nums text-pc-faint">{values[p.key]}</span>
          </span>
          <span class="flex items-center gap-2">
            <input
              type="range"
              min={p.min}
              max={p.max}
              step={p.step}
              bind:value={values[p.key]}
              class="flex-1 cursor-pointer"
              aria-label={fxText(p.label)}
            />
            <input
              type="number"
              min={p.min}
              max={p.max}
              step={p.step}
              bind:value={values[p.key]}
              class="w-16 rounded border border-pc-border bg-pc-bg px-1 py-0.5 text-right tabular-nums"
              aria-label={t('effectDlg.value', { label: fxText(p.label) })}
            />
          </span>
        </label>
      {:else if p.kind === 'bool'}
        <label class="flex items-center gap-2 text-xs text-pc-text">
          <input
            type="checkbox"
            checked={values[p.key] !== 0}
            onchange={(e) => (values[p.key] = e.currentTarget.checked ? 1 : 0)}
          />
          <span>{fxText(p.label)}</span>
        </label>
      {:else if p.kind === 'color'}
        {@const text = hexText(p.key, p.default)}
        {@const invalid = parseHexColor(text) === null}
        <div class="flex items-center justify-between gap-2 text-xs text-pc-text">
          <span>{fxText(p.label)}</span>
          <span class="flex items-center gap-2">
            <input
              type="color"
              value={formatHexColor(values[p.key] ?? p.default).slice(0, 7)}
              oninput={(e) => onSwatchInput(p.key, e.currentTarget.value)}
              class="swatch h-6 w-8"
              aria-label={fxText(p.label)}
              title={fxText(p.label)}
            />
            <input
              type="text"
              value={text}
              oninput={(e) => onHexInput(p.key, e.currentTarget.value)}
              spellcheck="false"
              maxlength="9"
              class="opt-field w-24 font-mono"
              class:invalid
              aria-invalid={invalid}
              aria-label={t('effectDlg.hex', { label: fxText(p.label) })}
              title={invalid ? t('effectDlg.hexInvalid') : undefined}
            />
          </span>
        </div>
      {:else}
        <label class="flex items-center justify-between gap-2 text-xs text-pc-text">
          <span>{fxText(p.label)}</span>
          <select
            value={values[p.key]}
            onchange={(e) => (values[p.key] = Number(e.currentTarget.value))}
            class="rounded border border-pc-border bg-pc-bg px-1 py-0.5"
            aria-label={fxText(p.label)}
          >
            {#each p.options as opt, i (opt)}
              <option value={i}>{fxText(opt)}</option>
            {/each}
          </select>
        </label>
      {/if}
    {/each}

    <footer class="mt-1 flex items-center justify-between gap-2">
      <label class="flex items-center gap-1 text-xs text-pc-muted">
        <input type="checkbox" bind:checked={preview} />
        <span>{t('effectDlg.preview')}</span>
      </label>
      <span class="flex gap-2">
        <button type="button" class="panel-btn" onclick={reset} disabled={def.params.length === 0}>
          {t('effectDlg.reset')}
        </button>
        <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
        <button type="submit" class="panel-btn panel-btn-primary">{t('common.ok')}</button>
      </span>
    </footer>
  </form>
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
  .invalid {
    outline: 1px solid #f87171;
  }
</style>
