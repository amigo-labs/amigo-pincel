<script lang="ts">
  // Import sprite sheet dialog (docs/specs/sprite-sheet-import.md): a
  // preview of the decoded sheet with the grid drawn over it, the grid
  // fields with the core's suggestion, the background option, the frame
  // duration and "Rows as tags". The core answers every question about
  // the pixels (suggestion, top-left colour, summary); the preview canvas
  // is written, never read (AGENTS.md §9).
  import { onMount } from 'svelte';
  import type { SpriteSheet } from '../core';
  import { t } from '../i18n/index.svelte';
  import { drawGrid, gridCells } from './grid-overlay';
  import type { SheetImportValues } from './open';
  import {
    GRID_FIELDS,
    MAX_DURATION,
    MIN_DURATION,
    fieldRange,
    fitPreview,
    gridErrorKey,
    hexOf,
    parseHex,
    parseWhole,
    rgbHexOf,
    sheetDefaults,
    type GridField,
    type GridValues,
  } from './values';

  let {
    sheet,
    onImport,
    onCancel,
  }: {
    sheet: SpriteSheet;
    onImport: (values: SheetImportValues) => void;
    onCancel: () => void;
  } = $props();

  // Mounted per import, so the initial sheet is the only one seen.
  // svelte-ignore state_referenced_locally
  const W = sheet.width;
  // svelte-ignore state_referenced_locally
  const H = sheet.height;
  // svelte-ignore state_referenced_locally
  const topLeft = sheet.topLeft() >>> 0;
  const [previewW, previewH] = fitPreview(W, H, 512, 288);

  /** The core's suggestion for `background`, or `null` (AC-09, AC-18). */
  function suggestion(background: number | undefined): GridValues | null {
    const g = sheet.suggest(background);
    return g && g.length === 6 ? [g[0]!, g[1]!, g[2]!, g[3]!, g[4]!, g[5]!] : null;
  }

  function asFields(grid: GridValues): Record<GridField, string> {
    return Object.fromEntries(GRID_FIELDS.map((f, i) => [f, String(grid[i])])) as Record<
      GridField,
      string
    >;
  }

  // AC-07: the colour is the top-left pixel; on when it is fully opaque.
  const opaque = (topLeft & 0xff) === 0xff;
  let bgOn = $state(opaque);
  let bgText = $state(hexOf(topLeft));
  // AC-01, AC-10: the suggestion, else the last confirmed import's values.
  const defaults = sheetDefaults();
  const first = suggestion(opaque ? topLeft : undefined);
  let fields = $state(asFields(first ?? defaults.grid));
  let noGrid = $state(first === null);
  let duration = $state(String(defaults.duration));
  let rowsAsTags = $state(defaults.rowsAsTags);

  const bgColor = $derived(parseHex(bgText));
  const background = $derived(bgOn && bgColor !== null ? bgColor : undefined);

  /** The grid when every field is a whole number in range, else the first failing field. */
  const parsed = $derived.by((): { grid: GridValues } | { error: string } => {
    const values: number[] = [];
    for (const field of GRID_FIELDS) {
      const range = fieldRange(field, W, H);
      const v = parseWhole(fields[field], range.min, range.max);
      if (v === null) {
        return {
          error: t('sheetImport.fieldRange', {
            field: t(range.label),
            min: range.min,
            max: range.max,
          }),
        };
      }
      values.push(v);
    }
    return { grid: values as unknown as GridValues };
  });
  const grid = $derived('grid' in parsed ? parsed.grid : null);
  const durationMs = $derived(parseWhole(duration, MIN_DURATION, MAX_DURATION));

  /** The core's summary of the grid (AC-03, AC-12). */
  const summary = $derived.by(() => {
    if (grid === null) return null;
    const s = sheet.summary(Uint32Array.from(grid), background, rowsAsTags);
    const out = { frames: s.frames, tags: s.tags, error: s.error ?? null };
    s.free();
    return out;
  });

  /** Why Import is disabled, or `null` (AC-12). */
  const reason = $derived.by((): string | null => {
    if ('error' in parsed) return parsed.error;
    if (durationMs === null) {
      return t('sheetImport.fieldRange', {
        field: t('sheetImport.durationName'),
        min: MIN_DURATION,
        max: MAX_DURATION,
      });
    }
    if (bgOn && bgColor === null) return t('sheetImport.badColor');
    const key = summary?.error ? gridErrorKey(summary.error) : null;
    return key ? t(key) : null;
  });

  const cells = $derived.by(() => {
    if (grid === null) return 0;
    const [cols, rows] = gridCells(grid, W, H);
    return cols * rows;
  });

  let previewCanvas: HTMLCanvasElement | undefined = $state();
  let gridCanvas: HTMLCanvasElement | undefined = $state();
  let dialogEl: HTMLDivElement | undefined = $state();

  onMount(() => {
    // The focus moves into the dialog, so its key handler sees every key.
    dialogEl?.querySelector<HTMLInputElement>('input')?.focus();
    const ctx = previewCanvas?.getContext('2d');
    if (!ctx) return;
    const rgba = sheet.preview(previewW, previewH);
    ctx.putImageData(new ImageData(new Uint8ClampedArray(rgba), previewW, previewH), 0, 0);
  });

  $effect(() => {
    if (gridCanvas) drawGrid(gridCanvas, grid, W, H);
  });

  // AC-11: the suggestion for the current background setting.
  function detect() {
    const g = suggestion(background);
    if (g) fields = asFields(g);
    noGrid = g === null;
  }

  function submit(e: SubmitEvent) {
    e.preventDefault();
    if (reason !== null || grid === null || durationMs === null) return;
    onImport({ grid, background, duration: durationMs, rowsAsTags });
  }

  // Keys typed in the dialog must not reach the mounted editors'
  // window-level shortcut maps (AC-14: the open documents stay unchanged).
  function onKeydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<div
  bind:this={dialogEl}
  class="fixed inset-0 z-(--pc-z-overlay) flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('sheetImport.title')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <form class="am-dialog max-h-full overflow-auto" style:width="min(560px, 100%)" onsubmit={submit}>
    <header>
      <h2>{t('sheetImport.title')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <div
        class="relative mx-auto max-w-full self-center border border-pc-border bg-pc-bg"
        role="img"
        aria-label={t('sheetImport.preview')}
      >
        <canvas
          bind:this={previewCanvas}
          width={previewW}
          height={previewH}
          class="block h-auto max-w-full [image-rendering:pixelated]"
        ></canvas>
        <canvas
          bind:this={gridCanvas}
          width={previewW}
          height={previewH}
          class="absolute inset-0 h-full w-full"
          data-testid="sheet-grid"
          data-grid={grid ? grid.join(',') : ''}
          data-cells={cells}
        ></canvas>
      </div>
      <p class="text-xs text-pc-muted">{t('sheetImport.sheetSize', { w: W, h: H })}</p>
      <div class="grid grid-cols-2 gap-2">
        {#each GRID_FIELDS as field (field)}
          {@const range = fieldRange(field, W, H)}
          <label class="flex flex-col gap-1 text-xs text-pc-text">
            <span>{t(range.label)}</span>
            <input
              type="number"
              inputmode="numeric"
              min={range.min}
              max={range.max}
              step="1"
              value={fields[field]}
              oninput={(e) => (fields[field] = e.currentTarget.value)}
              class="am-input tabular-nums"
              aria-label={t(range.label)}
            />
          </label>
        {/each}
      </div>
      <div class="flex items-center gap-2">
        <button type="button" class="am-btn am-btn--sm" onclick={detect}
          >{t('sheetImport.detect')}</button
        >
        {#if noGrid}
          <span class="text-xs text-pc-muted">{t('sheetImport.noGrid')}</span>
        {/if}
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <label class="am-check flex items-center gap-2 text-xs text-pc-text">
          <input type="checkbox" bind:checked={bgOn} aria-label={t('sheetImport.background')} />
          {t('sheetImport.background')}
        </label>
        <input
          type="text"
          class="am-input w-28 font-mono text-xs"
          bind:value={bgText}
          spellcheck="false"
          aria-label={t('sheetImport.backgroundColor')}
          aria-invalid={bgColor === null}
        />
        <input
          type="color"
          class="h-7 w-9 cursor-pointer rounded-pc-control border border-pc-border bg-transparent"
          value={rgbHexOf(bgColor ?? topLeft)}
          oninput={(e) => {
            bgText = e.currentTarget.value;
            bgOn = true;
          }}
          aria-label={t('sheetImport.backgroundPicker')}
        />
      </div>
      <label class="flex flex-col gap-1 text-xs text-pc-text">
        <span>{t('sheetImport.duration')}</span>
        <input
          type="number"
          inputmode="numeric"
          min={MIN_DURATION}
          max={MAX_DURATION}
          step="1"
          value={duration}
          oninput={(e) => (duration = e.currentTarget.value)}
          class="am-input tabular-nums w-auto"
          aria-label={t('sheetImport.duration')}
        />
      </label>
      <label class="am-check flex items-center gap-2 text-xs text-pc-text">
        <input type="checkbox" bind:checked={rowsAsTags} aria-label={t('sheetImport.rowsAsTags')} />
        {t('sheetImport.rowsAsTags')}
      </label>
      {#if reason !== null}
        <p class="am-hint am-hint--error" role="alert">{reason}</p>
      {:else if summary}
        <p class="text-xs text-pc-muted" aria-live="polite">
          {t('sheetImport.summary', { frames: summary.frames, tags: summary.tags })}
        </p>
      {/if}
    </div>
    <footer>
      <button type="button" class="am-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="am-btn am-btn--primary" disabled={reason !== null}>
        {t('sheetImport.import')}
      </button>
    </footer>
  </form>
</div>
