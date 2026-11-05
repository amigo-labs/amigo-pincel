<script lang="ts">
  // The desktop start screen's clipboard offer (docs/specs/copy-cut-paste.md
  // AC-27, AC-29): a non-blocking card with a preview of the clipboard
  // image, its size, Open and Ignore. The preview is drawn into a canvas,
  // never read back (AGENTS.md §9).
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

<section
  class="fixed bottom-3 right-3 z-20 flex w-64 flex-col gap-2 rounded border border-pc-border bg-pc-panel p-3 text-pc-text shadow-xl"
  aria-labelledby="clip-offer-title"
>
  <h2 id="clip-offer-title" class="text-sm font-semibold">{t('offer.title')}</h2>
  <div
    class="flex h-[120px] items-center justify-center rounded border border-pc-border bg-pc-bg"
    role="img"
    aria-label={t('offer.preview')}
  >
    <canvas width={size.width} height={size.height} {@attach draw}></canvas>
  </div>
  <p class="text-xs text-pc-muted">{t('offer.size', { w: image.width, h: image.height })}</p>
  <div class="flex justify-end gap-2">
    <button class="panel-btn" onclick={onIgnore}>{t('offer.ignore')}</button>
    <button class="panel-btn panel-btn-primary" onclick={onOpen} disabled={busy}>
      {t('offer.open')}
    </button>
  </div>
</section>

