<script lang="ts">
  import { t } from '../../../lib/i18n/index.svelte';
  // Export Animation dialog (spec §7.5, animated-export). Presentational:
  // `describe` asks the core about the chosen frames (count, default loop
  // count, GIF notices) and the parent runs `exportAnimation` from
  // `onSubmit`. `loops` is the total play count, 0 = forever.
  export interface AnimationExportOptions {
    format: 'apng' | 'gif';
    /** Tag name, or `null` for every frame. */
    tag: string | null;
    scale: number;
    loops: number;
  }

  /** What the core reports about the chosen frames. */
  export interface AnimationSummary {
    frameCount: number;
    defaultLoops: number;
    gifApproximated: boolean;
    semiTransparent: boolean;
  }

  let {
    frameCount,
    tags,
    frameWidth,
    frameHeight,
    describe,
    onSubmit,
    onCancel,
  }: {
    frameCount: number;
    tags: { name: string; from: number; to: number }[];
    frameWidth: number;
    frameHeight: number;
    describe: (tag: string | null) => AnimationSummary | null;
    onSubmit: (options: AnimationExportOptions) => void;
    onCancel: () => void;
  } = $props();

  const SCALES = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  let format = $state<'apng' | 'gif'>('apng');
  let tag = $state<string | null>(null);
  let scale = $state(1);
  // "All frames" always defaults to forever (AC-10).
  let forever = $state(true);
  let count = $state(1);

  const summary = $derived(describe(tag));
  const frames = $derived(summary?.frameCount ?? frameCount);

  /** A new frames choice resets the loop choice to its default (AC-10). */
  function chooseTag(next: string | null) {
    tag = next;
    const loops = summary?.defaultLoops ?? 0;
    forever = loops === 0;
    if (loops > 0) count = loops;
  }

  const loops = $derived(forever ? 0 : Math.max(1, Math.min(65535, Math.floor(count) || 1)));

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
  aria-label={t('anim.title')}
>
  <form
    class="flex w-80 flex-col gap-3 rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      onSubmit({ format, tag, scale, loops });
    }}
  >
    <h2 class="text-base font-semibold">{t('anim.title')}</h2>
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('anim.format')}</span>
      <select
        class="rounded border border-pc-border bg-pc-bg px-2 py-1"
        bind:value={format}
        aria-label={t('anim.format')}
      >
        <option value="apng">{t('anim.apng')}</option>
        <option value="gif">{t('anim.gif')}</option>
      </select>
    </label>
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('sheet.frames')}</span>
      <select
        class="rounded border border-pc-border bg-pc-bg px-2 py-1"
        value={tag ?? ''}
        onchange={(e) => chooseTag(e.currentTarget.value === '' ? null : e.currentTarget.value)}
        aria-label={t('sheet.frames')}
      >
        <option value="">{t('sheet.allFrames', { n: frameCount })}</option>
        {#each tags as tag (tag.name)}
          <option value={tag.name}>{tag.name} ({tag.to - tag.from + 1})</option>
        {/each}
      </select>
    </label>
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('anim.scale')}</span>
      <select
        class="rounded border border-pc-border bg-pc-bg px-2 py-1"
        bind:value={scale}
        aria-label={t('anim.scale')}
      >
        {#each SCALES as s (s)}
          <option value={s}>×{s}</option>
        {/each}
      </select>
    </label>
    <fieldset class="flex flex-col gap-1 text-xs text-pc-text">
      <legend class="mb-1">{t('anim.loop')}</legend>
      <label class="flex items-center gap-2">
        <input type="radio" name="anim-loop" checked={forever} onchange={() => (forever = true)} />
        {t('anim.forever')}
      </label>
      <label class="flex items-center gap-2">
        <input
          type="radio"
          name="anim-loop"
          checked={!forever}
          onchange={() => (forever = false)}
          aria-label={t('anim.times')}
        />
        <input
          type="number"
          min="1"
          max="65535"
          bind:value={count}
          disabled={forever}
          class="w-20 rounded border border-pc-border bg-pc-bg px-2 py-1 tabular-nums disabled:opacity-50"
          aria-label={t('anim.loopCount')}
        />
        {t('anim.timesSuffix')}
      </label>
    </fieldset>
    <p class="text-xs text-pc-muted" aria-live="polite">
      {t('anim.estimate', { n: frames, w: frameWidth * scale, h: frameHeight * scale })}
    </p>
    {#if format === 'gif' && summary?.gifApproximated}
      <p class="text-xs text-amber-400" role="status">{t('anim.gifApproximated')}</p>
    {/if}
    {#if format === 'gif' && summary?.semiTransparent}
      <p class="text-xs text-pc-muted" role="status">{t('anim.gifAlpha')}</p>
    {/if}
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
