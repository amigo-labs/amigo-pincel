<script lang="ts" module>
  /** The dialog's modes; the numbers are the wasm wire format. */
  export type SvgMode = 'color' | 'mono' | 'exact';

  /** One trace from the editor's `Vectorizer`: the SVG and its counts, or why it failed. */
  export type SvgTrace =
    | { svg: string; layers: number; outlines: number; threshold: number }
    | { error: string };

  export const SVG_MODE_WIRE: Record<SvgMode, number> = { color: 0, mono: 1, exact: 2 };
</script>

<script lang="ts">
  import { t } from '../i18n/index.svelte';

  // Export as SVG (docs/specs/vectorize-dialog.md). Presentational: the
  // editor passes `trace`, which runs its wasm `Vectorizer` over the
  // export's source image, and saves the SVG handed to `onExport`.
  let {
    defaultMode,
    trace,
    onExport,
    onCancel,
  }: {
    defaultMode: SvgMode;
    trace: (mode: number, params: number[]) => SvgTrace;
    onExport: (svg: string) => void;
    onCancel: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let mode = $state<SvgMode>(defaultMode);
  let colors = $state(6);
  let autoThreshold = $state(true);
  let threshold = $state(128);
  let invert = $state(false);
  let fill = $state('#000000');
  let despeckle = $state(false);
  let minArea = $state(2);
  let alphaMax = $state(1);
  let optCurve = $state(true);
  let tolerance = $state(0.2);

  const clampInt = (v: number, lo: number, hi: number) =>
    Math.max(lo, Math.min(hi, Math.round(Number.isFinite(v) ? v : lo)));

  /** The flat option vector of `TraceOptions::from_wire`. */
  const params = $derived([
    clampInt(colors, 2, 16),
    autoThreshold ? -1 : clampInt(threshold, 0, 255),
    invert ? 1 : 0,
    Number.parseInt(fill.slice(1), 16) || 0,
    despeckle ? 1 : 0,
    clampInt(minArea, 0, 100000),
    Math.max(0, Math.min(1.34, Number.isFinite(alphaMax) ? alphaMax : 1)),
    optCurve ? 1 : 0,
    Math.max(0, Number.isFinite(tolerance) ? tolerance : 0.2),
  ]);

  let result = $state<SvgTrace | null>(null);

  // Coalesce option changes: re-trace 150 ms after the last change.
  $effect(() => {
    const wire = SVG_MODE_WIRE[mode];
    const p = params;
    const timer = setTimeout(() => (result = trace(wire, p)), 150);
    return () => clearTimeout(timer);
  });

  const svgResult = $derived(result && 'svg' in result ? result : null);
  const errorText = $derived.by(() => {
    if (!result || !('error' in result)) return null;
    const count = /the image has (\d+)/.exec(result.error)?.[1];
    return count !== undefined
      ? t('svg.tooManyColors', { n: count })
      : t('svg.failed', { error: result.error });
  });
  const previewUrl = $derived(
    svgResult ? `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svgResult.svg)}` : null,
  );

  function formatSize(bytes: number): string {
    return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
  }

  const summary = $derived(
    svgResult
      ? t('svg.summary', {
          layers:
            svgResult.layers === 1 ? t('svg.layersOne') : t('svg.layersMany', { n: svgResult.layers }),
          outlines:
            svgResult.outlines === 1
              ? t('svg.outlinesOne')
              : t('svg.outlinesMany', { n: svgResult.outlines }),
          size: formatSize(new TextEncoder().encode(svgResult.svg).length),
        })
      : null,
  );

  /** Export the options shown now, even if a re-trace is still pending. */
  function submit() {
    const fresh = trace(SVG_MODE_WIRE[mode], params);
    result = fresh;
    if ('svg' in fresh) onExport(fresh.svg);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }

  const field = 'rounded border border-pc-border bg-pc-bg px-2 py-1';
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={t('svg.title')}
>
  <form
    class="flex max-h-[90vh] w-96 flex-col gap-3 overflow-y-auto rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <h2 class="text-base font-semibold">{t('svg.title')}</h2>

    <label class="flex flex-col gap-1 text-xs">
      <span>{t('svg.mode')}</span>
      <select class={field} bind:value={mode} aria-label={t('svg.mode')}>
        <option value="color">{t('svg.modeColor')}</option>
        <option value="mono">{t('svg.modeMono')}</option>
        <option value="exact">{t('svg.modeExact')}</option>
      </select>
    </label>

    {#if mode === 'color'}
      <label class="flex items-center justify-between gap-2 text-xs">
        <span>{t('svg.colors')}</span>
        <input type="number" min="2" max="16" step="1" class="w-20 {field}" bind:value={colors} aria-label={t('svg.colors')} />
      </label>
    {:else if mode === 'mono'}
      <label class="flex items-center gap-2 text-xs">
        <input type="checkbox" bind:checked={autoThreshold} />
        {t('svg.autoThreshold')}
      </label>
      <label class="flex items-center justify-between gap-2 text-xs">
        <span>{t('svg.threshold')}</span>
        <input
          type="number"
          min="0"
          max="255"
          step="1"
          class="w-20 {field} disabled:opacity-50"
          bind:value={threshold}
          disabled={autoThreshold}
          aria-label={t('svg.threshold')}
        />
      </label>
      <label class="flex items-center gap-2 text-xs">
        <input type="checkbox" bind:checked={invert} />
        {t('svg.invert')}
      </label>
      <label class="flex items-center justify-between gap-2 text-xs">
        <span>{t('svg.fill')}</span>
        <input type="color" class="h-7 w-12 {field}" bind:value={fill} aria-label={t('svg.fill')} />
      </label>
    {/if}

    {#if mode !== 'exact'}
      <label class="flex items-center gap-2 text-xs">
        <input type="checkbox" bind:checked={despeckle} />
        {t('svg.despeckle')}
      </label>
      <label class="flex items-center justify-between gap-2 text-xs">
        <span>{t('svg.minArea')}</span>
        <input type="number" min="0" step="1" class="w-20 {field}" bind:value={minArea} aria-label={t('svg.minArea')} />
      </label>
      <label class="flex items-center justify-between gap-2 text-xs">
        <span>{t('svg.cornerThreshold')}</span>
        <input
          type="number"
          min="0"
          max="1.34"
          step="0.01"
          class="w-20 {field}"
          bind:value={alphaMax}
          aria-label={t('svg.cornerThreshold')}
        />
      </label>
      <label class="flex items-center gap-2 text-xs">
        <input type="checkbox" bind:checked={optCurve} />
        {t('svg.optCurve')}
      </label>
      <label class="flex items-center justify-between gap-2 text-xs">
        <span>{t('svg.tolerance')}</span>
        <input
          type="number"
          min="0"
          step="0.05"
          class="w-20 {field} disabled:opacity-50"
          bind:value={tolerance}
          disabled={!optCurve}
          aria-label={t('svg.tolerance')}
        />
      </label>
    {/if}

    <div class="flex h-48 items-center justify-center rounded border border-pc-border bg-pc-bg">
      {#if errorText}
        <p class="p-2 text-center text-xs text-amber-400" role="alert">{errorText}</p>
      {:else if previewUrl}
        <img class="h-full w-full object-contain p-1" src={previewUrl} alt={t('svg.preview')} />
      {/if}
    </div>
    <p class="text-xs text-pc-muted" aria-live="polite" data-testid="svg-summary">{summary ?? ''}</p>

    <div class="flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary" disabled={errorText !== null}>
        {t('export.export')}
      </button>
    </div>
  </form>
</div>

<style>
  .panel-btn {
    border-radius: 0.25rem;
    border: 1px solid rgb(64 64 64);
    padding: 0.25rem 0.75rem;
  }
  .panel-btn:hover {
    background-color: rgb(38 38 38);
  }
  .panel-btn:disabled {
    opacity: 0.5;
  }
  .panel-btn-primary {
    border-color: rgb(59 130 246);
    background-color: rgb(29 78 216);
  }
</style>
