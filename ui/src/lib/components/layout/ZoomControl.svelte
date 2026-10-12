<script lang="ts">
  // Floating zoom control in the canvas corner (spec §9.2), same in both
  // modes: out · current zoom (click fits) · in · 1:1.
  import IconButton from './IconButton.svelte';
  import { hint } from '../../shell/shortcuts';
  import { t } from '../../i18n/index.svelte';

  interface Props {
    /** Current zoom, e.g. `125%` or `8×`. */
    label: string;
    onZoomIn: () => void;
    onZoomOut: () => void;
    onFit: () => void;
    onActual: () => void;
    canZoomIn?: boolean;
    canZoomOut?: boolean;
  }
  const {
    label,
    onZoomIn,
    onZoomOut,
    onFit,
    onActual,
    canZoomIn = true,
    canZoomOut = true,
  }: Props = $props();
</script>

<div
  class="absolute right-3 bottom-3 z-20 flex items-center gap-0.5 rounded-md border border-pc-border bg-pc-panel/90 p-0.5 text-xs shadow-lg backdrop-blur"
  role="group"
  aria-label={t('zoom.label')}
>
  <IconButton icon="zoom-out" label={t('zoom.out')} hint={hint('zoomOut')} size="sm" onclick={onZoomOut} disabled={!canZoomOut} />
  <button
    type="button"
    class="h-6 min-w-12 rounded px-1 text-center tabular-nums hover:bg-pc-hover"
    title={`${t('zoom.fit')} (${hint('zoomFit')})`}
    aria-label={t('zoom.fit')}
    onclick={onFit}
  >
    {label}
  </button>
  <IconButton icon="zoom-in" label={t('zoom.in')} hint={hint('zoomIn')} size="sm" onclick={onZoomIn} disabled={!canZoomIn} />
  <button
    type="button"
    class="h-6 rounded px-1.5 hover:bg-pc-hover"
    title={`${t('zoom.actual')} (${hint('zoomActual')})`}
    aria-label={t('zoom.actual')}
    onclick={onActual}
  >
    1:1
  </button>
</div>
