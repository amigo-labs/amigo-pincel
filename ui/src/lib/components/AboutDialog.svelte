<script lang="ts">
  // Help ▸ About Pincel. On macOS the native menu's About item serves this
  // instead (spec §11.3), so the dialog only ever opens where the in-app
  // Help menu exists.
  import { t } from '../i18n/index.svelte';

  let { onClose }: { onClose: () => void } = $props();

  // Release version stamped in by the release workflow; `dev` locally.
  const version = import.meta.env.VITE_APP_VERSION ?? 'dev';
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/70 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="about-title"
>
  <div class="w-full max-w-sm rounded border border-pc-border bg-pc-panel p-6 shadow-xl">
    <h2 id="about-title" class="mb-1 text-lg font-semibold">{t('menu.about')}</h2>
    <p class="mb-3 text-sm text-pc-muted">{t('about.version', { version })}</p>
    <p class="mb-4 text-sm text-pc-text">{t('about.tagline')}</p>
    <div class="flex justify-end">
      <button class="toolbar-btn" onclick={onClose}>{t('about.close')}</button>
    </div>
  </div>
</div>
