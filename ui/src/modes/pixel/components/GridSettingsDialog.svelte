<script lang="ts">
  import Dialog from '../../../lib/components/Dialog.svelte';
  import { t } from '../../../lib/i18n/index.svelte';
  // View ▸ Grid Settings… (docs/specs/pixel-grid.md). Presentational: the
  // parent applies the result — cell size and offset to the sprite (one
  // undo step), line colour and opacity to the per-user `pixelGrid` pref.
  // Values the `.aseprite` header cannot store are refused here (AC-13).
  export interface GridSettings {
    x: number;
    y: number;
    w: number;
    h: number;
    /** `#rrggbb`. */
    color: string;
    /** 0–100 %. */
    opacity: number;
  }

  let {
    initial,
    tileSize,
    onSubmit,
    onCancel,
  }: {
    initial: GridSettings;
    /** The active tilemap layer's tile size, or `null` (AC-17). */
    tileSize: { w: number; h: number } | null;
    onSubmit: (settings: GridSettings) => void;
    onCancel: () => void;
  } = $props();

  // Remounted per open, so the initial props are the only ones seen.
  // svelte-ignore state_referenced_locally
  let w = $state(initial.w);
  // svelte-ignore state_referenced_locally
  let h = $state(initial.h);
  // svelte-ignore state_referenced_locally
  let x = $state(initial.x);
  // svelte-ignore state_referenced_locally
  let y = $state(initial.y);
  // svelte-ignore state_referenced_locally
  let color = $state(initial.color);
  // svelte-ignore state_referenced_locally
  let opacity = $state(initial.opacity);
  let error = $state<string | null>(null);

  const inRange = (v: unknown, min: number, max: number) =>
    typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max;

  function submit() {
    if (
      !inRange(w, 1, 65535) ||
      !inRange(h, 1, 65535) ||
      !inRange(x, -32768, 32767) ||
      !inRange(y, -32768, 32767)
    ) {
      error = t('grid.invalid');
      return;
    }
    const pct = typeof opacity === 'number' && Number.isFinite(opacity) ? opacity : initial.opacity;
    onSubmit({ x, y, w, h, color, opacity: Math.min(100, Math.max(0, Math.round(pct))) });
  }

  function useTileSize() {
    if (!tileSize) return;
    w = tileSize.w;
    h = tileSize.h;
    x = 0;
    y = 0;
    error = null;
  }
</script>

<Dialog
  title={t('grid.title')}
  applyLabel={t('common.apply')}
  onApply={submit}
  onClose={onCancel}
>
  <div class="flex flex-col gap-3">
    <div class="grid grid-cols-2 gap-2">
      {#each [
        { label: t('grid.cellWidth'), get: () => w, set: (v: number) => (w = v) },
        { label: t('grid.cellHeight'), get: () => h, set: (v: number) => (h = v) },
        { label: t('grid.offsetX'), get: () => x, set: (v: number) => (x = v) },
        { label: t('grid.offsetY'), get: () => y, set: (v: number) => (y = v) },
      ] as field (field.label)}
        <label class="flex flex-col gap-1 text-xs text-pc-text">
          <span>{field.label}</span>
          <input
            type="number"
            step="1"
            value={field.get()}
            oninput={(e) => {
              field.set(e.currentTarget.valueAsNumber);
              error = null;
            }}
            class="am-input tabular-nums w-auto"
            aria-label={field.label}
          />
        </label>
      {/each}
    </div>
    {#if tileSize}
      <button type="button" class="am-btn self-start" onclick={useTileSize}>
        {t('grid.useTileSize', { w: tileSize.w, h: tileSize.h })}
      </button>
    {/if}
    <div class="grid grid-cols-2 gap-2">
      <label class="flex flex-col gap-1 text-xs text-pc-text">
        <span>{t('grid.color')}</span>
        <input
          type="color"
          bind:value={color}
          class="h-7 w-full rounded border border-pc-border bg-pc-bg"
          aria-label={t('grid.color')}
        />
      </label>
      <label class="flex flex-col gap-1 text-xs text-pc-text">
        <span>{t('grid.opacity')}</span>
        <input
          type="number"
          min="0"
          max="100"
          step="1"
          bind:value={opacity}
          class="am-input tabular-nums w-auto"
          aria-label={t('grid.opacity')}
        />
      </label>
    </div>
    {#if error}
      <p class="am-hint am-hint--error" role="alert">{error}</p>
    {/if}
  </div>
</Dialog>
