<script lang="ts">
  // The desktop start screen's clipboard offer (docs/specs/copy-cut-paste.md
  // AC-27, AC-29): a non-blocking card with a preview of the clipboard
  // image, its size, Open and Ignore. The preview is drawn into a canvas,
  // never read back (AGENTS.md §9).
  import Button from '../components/Button.svelte';
  import type { Attachment } from 'svelte/attachments';
  import { t } from '../i18n/index.svelte';
  import { markOffered, type OfferImage } from './clipboard-offer.svelte';

  let {
    image,
    busy = false,
    onOpen,
    onIgnore,
  }: {
    image: OfferImage;
    busy?: boolean;
    onOpen: () => void;
    onIgnore: () => void;
  } = $props();

  const MAX_W = 160;
  const MAX_H = 120;

  // Scaled down, never up, aspect ratio kept (AC-27).
  const size = $derived.by(() => {
    const scale = Math.min(1, MAX_W / image.width, MAX_H / image.height);
    return {
      width: Math.max(1, Math.round(image.width * scale)),
      height: Math.max(1, Math.round(image.height * scale)),
    };
  });

  // Shown once is offered for the rest of the app session (AC-28).
  $effect(() => markOffered(image));

  const draw: Attachment<HTMLCanvasElement> = (canvas) => {
    const { width, height } = size;
    const src = image;
    let cancelled = false;
    const pixels = new ImageData(
      new Uint8ClampedArray(src.rgba.buffer, src.rgba.byteOffset, src.rgba.byteLength),
      src.width,
      src.height,
    );
    void createImageBitmap(pixels, {
      resizeWidth: width,
      resizeHeight: height,
      resizeQuality: 'high',
    })
      .then((bitmap) => {
        const ctx = canvas.getContext('2d');
        if (!cancelled && ctx) {
          ctx.clearRect(0, 0, width, height);
          ctx.drawImage(bitmap, 0, 0);
        }
        bitmap.close();
      })
      .catch((err: unknown) => console.error('clipboard preview failed', err));
    return () => {
      cancelled = true;
    };
  };
</script>

<!-- In the start screen's page, between the New cards and the "Open from
     clipboard" row (AC-27): preview on the left, title, size and buttons on
     the right; stacked on narrow screens. -->
<section
  class="start-surface mt-3 flex flex-wrap items-stretch gap-3 rounded-lg border border-pc-border bg-pc-panel p-3 text-pc-text"
  aria-labelledby="clip-offer-title"
>
  <div
    class="flex h-[120px] w-[160px] shrink-0 items-center justify-center rounded border border-pc-border bg-pc-bg"
    role="img"
    aria-label={t('offer.preview')}
  >
    <canvas width={size.width} height={size.height} {@attach draw}></canvas>
  </div>
  <div class="flex min-w-[10rem] flex-1 flex-col gap-1">
    <h2 id="clip-offer-title" class="text-[13px] leading-5 font-semibold">{t('offer.title')}</h2>
    <p class="text-xs leading-4 text-pc-muted">
      {t('offer.size', { w: image.width, h: image.height })}
    </p>
    <div class="mt-auto flex flex-wrap justify-end gap-2 pt-2">
      <Button onclick={onIgnore}>{t('offer.ignore')}</Button>
      <Button variant="primary" onclick={onOpen} {busy}>{t('offer.open')}</Button>
    </div>
  </div>
</section>
