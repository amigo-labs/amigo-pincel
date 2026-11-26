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
  class="am-banner am-banner--float fixed right-3 bottom-3 z-(--pc-z-drawer) max-w-sm items-center"
  aria-label={t('update.notice')}
  data-update-notice
>
  <div>{t('update.noticeText', { version })}</div>
  <button class="am-btn" onkeydown={onKeydown} onclick={onInstall}>{t('about.install')}</button>
  <button class="am-btn" onkeydown={onKeydown} onclick={onLater}>{t('update.later')}</button>
  <button
    class="am-btn"
    onkeydown={onKeydown}
    aria-label={t('update.closeNotice')}
    title={t('update.closeNotice')}
    onclick={onLater}
  >
    &times;
  </button>
</section>
