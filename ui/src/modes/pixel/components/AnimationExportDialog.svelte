<script lang="ts">
  import Dialog from '../../../lib/components/Dialog.svelte';
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
</script>

<Dialog
  title={t('anim.title')}
  applyLabel={t('menu.export')}
  onApply={() => onSubmit({ format, tag, scale, loops })}
  onClose={onCancel}
>
  <div class="flex flex-col gap-3">
    <label class="flex flex-col gap-1 text-xs text-pc-text">
      <span>{t('anim.format')}</span>
      <select
        class="am-input w-auto"
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
        class="am-input w-auto"
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
        class="am-input w-auto"
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
      <label class="am-check flex items-center gap-2">
        <input type="radio" name="anim-loop" checked={forever} onchange={() => (forever = true)} />
        {t('anim.forever')}
      </label>
      <label class="flex items-center gap-2">
        <span class="am-check">
          <input
            type="radio"
            name="anim-loop"
            checked={!forever}
            onchange={() => (forever = false)}
            aria-label={t('anim.times')}
          />
        </span>
        <input
          type="number"
          min="1"
          max="65535"
          bind:value={count}
          disabled={forever}
          class="am-input w-20 tabular-nums"
          aria-label={t('anim.loopCount')}
        />
        {t('anim.timesSuffix')}
      </label>
    </fieldset>
    <p class="text-xs text-pc-muted" aria-live="polite">
      {t('anim.estimate', { n: frames, w: frameWidth * scale, h: frameHeight * scale })}
    </p>
    {#if format === 'gif' && summary?.gifApproximated}
      <div class="am-banner am-banner--warning" role="status"><div><strong>{t('anim.gifApproximated')}</strong></div></div>
    {/if}
    {#if format === 'gif' && summary?.semiTransparent}
      <p class="text-xs text-pc-muted" role="status">{t('anim.gifAlpha')}</p>
    {/if}
  </div>
</Dialog>
