<script lang="ts">
  // "Animated image": open an animated GIF, APNG or WebP as an animation in
  // Pixel mode or its first frame as the open path does it otherwise
  // (docs/specs/animated-image-import.md AC-01, AC-06 – AC-08). Modal: keys
  // typed here never reach the editor's window-level shortcuts (AC-15).
  import Button from '../components/Button.svelte';
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import type { AnimatedFile, AnimationChoice } from './animation-open';

  interface Props {
    /** The file's name. */
    name: string;
    /** What the probe found. */
    info: AnimatedFile;
    onChoose: (choice: AnimationChoice) => void;
  }
  const { name, info, onChoose }: Props = $props();

  let animationButton: HTMLButtonElement | undefined = $state();
  let firstButton: HTMLButtonElement | undefined = $state();

  /** Why "Open as animation" is disabled (AC-08), or `null`. */
  const limitNote = $derived.by(() => {
    if (info.exceeded === 'side') {
      return t('animOpen.limitSide', { w: info.width, h: info.height, max: info.maxSide });
    }
    if (info.exceeded === 'pixels') {
      return t('animOpen.limitPixels', {
        n: info.frames,
        w: info.width,
        h: info.height,
        total: info.totalPixels,
        max: info.maxPixels,
      });
    }
    return null;
  });

  onMount(() => (animationButton ?? firstButton)?.focus());

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onChoose('cancel');
    }
  }
</script>

<div
  class="fixed inset-0 z-(--pc-z-dialog) flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('animOpen.title')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div class="am-dialog">
    <header>
      <h2>{t('animOpen.title')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <p class="text-pc-muted">
        {t('animOpen.prompt', { name, n: info.frames, w: info.width, h: info.height })}
      </p>
      {#if limitNote}
        <div class="am-banner am-banner--warning" data-testid="animation-limit">
          <div><strong>{t('animOpen.tooLarge', { reason: limitNote })}</strong></div>
        </div>
      {/if}
    </div>
    <footer class="flex-wrap">
      <Button onclick={() => onChoose('cancel')}>{t('common.cancel')}</Button>
      <Button bind:el={firstButton} onclick={() => onChoose('first')}>
        {t('animOpen.openFirst')}
      </Button>
      {#if limitNote}
        <Button disabled>{t('animOpen.openAnimation')}</Button>
      {:else}
        <Button bind:el={animationButton} variant="primary" onclick={() => onChoose('animation')}>
          {t('animOpen.openAnimation')}
        </Button>
      {/if}
    </footer>
  </div>
</div>
