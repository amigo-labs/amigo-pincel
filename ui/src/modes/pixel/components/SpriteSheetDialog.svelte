<script lang="ts">
  import { t } from '../../../lib/i18n/index.svelte';
  // Export Sprite Sheet dialog (spec §7.3). Presentational: the parent
  // runs `exportAtlas` from `onSubmit`. Frames pack left-to-right,
  // top-to-bottom into a grid of `columns` cells; `tag` restricts the
  // sheet to one animation; `data` also writes the Aseprite `--data`
  // JSON sidecar (durations, tags, slices).
  export interface SpriteSheetOptions {
    columns: number;
    /** Tag name, or `null` for every frame. */
    tag: string | null;
    data: boolean;
  }

  let {
    frameCount,
    tags,
    frameWidth,
    frameHeight,
    onSubmit,
    onCancel,
  }: {
    frameCount: number;
    tags: { name: string; from: number; to: number }[];
    frameWidth: number;
    frameHeight: number;
    onSubmit: (options: SpriteSheetOptions) => void;
    onCancel: () => void;
  } = $props();

  // Remounted per open, so the initial props are the only ones seen.
  // svelte-ignore state_referenced_locally
  let columns = $state(Math.max(1, Math.ceil(Math.sqrt(frameCount))));
  let tag = $state<string | null>(null);
  let data = $state(true);

  const count = $derived.by(() => {
    const found = tags.find((x) => x.name === tag);
    return found ? found.to - found.from + 1 : frameCount;
  });
  const cols = $derived(Math.max(1, Math.min(count, Math.floor(columns) || 1)));
  const rows = $derived(Math.ceil(count / cols));

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50"
  role="dialog"
  aria-modal="true"
  aria-label={t('sheet.title')}
>
  <form
    class="flex w-80 flex-col gap-3 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      onSubmit({ columns: cols, tag, data });
    }}
  >
    <h2 class="text-base font-semibold">{t('sheet.title')}</h2>
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('sheet.frames')}</span>
      <select
        class="rounded border border-pc-border bg-pc-bg px-2 py-1"
        value={tag ?? ''}
        onchange={(e) => (tag = e.currentTarget.value === '' ? null : e.currentTarget.value)}
        aria-label={t('sheet.frames')}
      >
        <option value="">{t('sheet.allFrames', { n: frameCount })}</option>
        {#each tags as tag (tag.name)}
          <option value={tag.name}>{tag.name} ({tag.to - tag.from + 1})</option>
        {/each}
      </select>
    </label>
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('sheet.columns')}</span>
      <input
        type="number"
        min="1"
        max={count}
        bind:value={columns}
        class="rounded border border-pc-border bg-pc-bg px-2 py-1 tabular-nums"
        aria-label={t('sheet.columns')}
      />
    </label>
    <p class="text-xs text-pc-muted" aria-live="polite">
      {cols} × {rows} cells · {cols * frameWidth}×{rows * frameHeight} px
    </p>
    <label class="flex items-center gap-2 text-xs text-pc-text">
      <input type="checkbox" bind:checked={data} aria-label={t('sheet.json')} />
      {t('sheet.jsonHint')}
    </label>
    <div class="flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary">{t('menu.export')}</button>
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
  .panel-btn-primary {
    border-color: rgb(59 130 246);
    background-color: rgb(29 78 216);
  }
</style>
