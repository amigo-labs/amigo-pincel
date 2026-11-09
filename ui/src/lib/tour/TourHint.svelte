<script lang="ts">
  // The one-time offer of an editor's part of the tour
  // (docs/specs/first-run-tutorial.md AC-10): shown on that editor's first
  // open only, never after No thanks on the start-screen banner.
  import { t } from '../i18n/index.svelte';
  import { hintDue, markHinted, startTour, tour } from './tour-state.svelte';

  let { mode, active }: { mode: 'pixel' | 'image'; active: boolean } = $props();
  let shown = $state(false);

  $effect(() => {
    if (active && tour.loaded && hintDue(mode)) {
      shown = true;
      markHinted(mode);
    }
  });
</script>

{#if shown && active}
  <div class="flex min-w-0 items-center gap-1.5 text-xs text-pc-text" role="status">
    <span class="min-w-0 truncate">{mode === 'pixel' ? t('tour.hintPixel') : t('tour.hintImage')}</span>
    <button
      class="panel-btn panel-btn-primary shrink-0 py-0.5"
      onclick={() => {
        shown = false;
        startTour(mode);
      }}>{t('tour.hintTake')}</button
    >
    <button class="panel-btn shrink-0 py-0.5" onclick={() => (shown = false)}>{t('tour.hintClose')}</button>
  </div>
{/if}
