<script lang="ts">
  import Dialog from '../../../lib/components/Dialog.svelte';
  import { t } from '../../../lib/i18n/index.svelte';
  import { EXPORT_SCALES } from '../export-scale';
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
    /** Integer nearest-neighbour upscale, 1–10 (scaled-export). */
    scale: number;
  }

  let {
    frameCount,
    tags,
    frameWidth,
    frameHeight,
    scale: initialScale,
    onSubmit,
    onCancel,
  }: {
    frameCount: number;
    tags: { name: string; from: number; to: number }[];
    frameWidth: number;
    frameHeight: number;
    /** The preselected scale (the last one used). */
    scale: number;
    onSubmit: (options: SpriteSheetOptions) => void;
    onCancel: () => void;
  } = $props();

  // Remounted per open, so the initial props are the only ones seen.
  // svelte-ignore state_referenced_locally
  let columns = $state(Math.max(1, Math.ceil(Math.sqrt(frameCount))));
  let tag = $state<string | null>(null);
  let data = $state(true);
  // svelte-ignore state_referenced_locally
  let scale = $state(initialScale);

  const count = $derived.by(() => {
    const found = tags.find((x) => x.name === tag);
    return found ? found.to - found.from + 1 : frameCount;
  });
  const cols = $derived(Math.max(1, Math.min(count, Math.floor(columns) || 1)));
  const rows = $derived(Math.ceil(count / cols));
</script>

<Dialog
  title={t('sheet.title')}
  applyLabel={t('menu.export')}
  onApply={() => onSubmit({ columns: cols, tag, data, scale })}
  onClose={onCancel}
>
  <div class="flex flex-col gap-3">
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('sheet.frames')}</span>
      <select
        class="am-input w-auto"
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
        class="am-input tabular-nums w-auto"
        aria-label={t('sheet.columns')}
      />
    </label>
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('export.scale')}</span>
      <select
        class="am-input w-auto"
        bind:value={scale}
        aria-label={t('export.scale')}
      >
        {#each EXPORT_SCALES as s (s)}
          <option value={s}>×{s}</option>
        {/each}
      </select>
    </label>
    <p class="text-xs text-pc-muted" aria-live="polite">
      {t('sheet.grid', { cols, rows, w: cols * frameWidth * scale, h: rows * frameHeight * scale })}
    </p>
    <label class="am-check flex items-center gap-2 text-xs text-pc-text">
      <input type="checkbox" bind:checked={data} aria-label={t('sheet.json')} />
      {t('sheet.jsonHint')}
    </label>
  </div>
</Dialog>
