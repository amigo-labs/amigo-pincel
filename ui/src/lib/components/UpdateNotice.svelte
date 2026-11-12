<script lang="ts">
  // The "notify" announcement (docs/specs/desktop-app.md AC-26): a card at
  // the bottom of the window that leaves the editor usable and takes no
  // focus. Install downloads and reports in the About dialog; Later, the
  // close button and Escape inside the card mark the version announced.
  // The focus can only be on one of its buttons, so they take Escape.
  import { t } from '../i18n/index.svelte';

  let {
    version,
    onInstall,
    onLater,
  }: { version: string; onInstall: () => void; onLater: () => void } = $props();

  function onKeydown(e: KeyboardEvent): void {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    onLater();
  }
</script>

<section
  class="fixed right-3 bottom-3 z-30 flex max-w-sm items-center gap-3 rounded border border-pc-border bg-pc-panel px-3 py-2 text-sm text-pc-text shadow-xl"
  aria-label={t('update.notice')}
  data-update-notice
>
  <span>{t('update.noticeText', { version })}</span>
  <button class="panel-btn" onkeydown={onKeydown} onclick={onInstall}>{t('about.install')}</button>
  <button class="panel-btn" onkeydown={onKeydown} onclick={onLater}>{t('update.later')}</button>
  <button
    class="panel-btn"
    onkeydown={onKeydown}
    aria-label={t('update.closeNotice')}
    title={t('update.closeNotice')}
    onclick={onLater}
  >
    &times;
  </button>
</section>
