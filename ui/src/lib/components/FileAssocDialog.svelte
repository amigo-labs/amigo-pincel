<script lang="ts">
  // First-launch advisory dialog for the file associations (`.aseprite`
  // sprites and the Image-mode raster formats).
  // On Tauri the install bundle registers the type at the OS level —
  // on macOS LaunchServices, on Windows the installer, on Linux the
  // `.desktop` file. We can't programmatically force Pincel to be the
  // default handler without elevation, so this dialog walks the user
  // through the per-OS steps once. "Don't show again" sets a pref so
  // the dialog never re-appears.
  import { t } from '../i18n/index.svelte';

  let {
    platform,
    onDismiss,
  }: {
    platform: 'macos' | 'windows' | 'linux' | 'unknown';
    onDismiss: (dontShowAgain: boolean) => void;
  } = $props();

  let dontShowAgain = $state(false);
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/70 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="fileassoc-title"
>
  <div class="w-full max-w-lg rounded border border-pc-border bg-pc-panel p-6 shadow-xl">
    <h2 id="fileassoc-title" class="mb-3 text-lg font-semibold">{t('fileAssoc.title')}</h2>
    <p class="mb-3 text-sm text-pc-text">{t('fileAssoc.body')}</p>
    {#if platform === 'macos'}
      <ol class="mb-4 list-decimal space-y-1 pl-5 text-sm text-pc-text">
        <li>{t('fileAssoc.mac1')}</li>
        <li>{t('fileAssoc.mac2')}</li>
        <li>{t('fileAssoc.mac3')}</li>
      </ol>
    {:else if platform === 'windows'}
      <ol class="mb-4 list-decimal space-y-1 pl-5 text-sm text-pc-text">
        <li>{t('fileAssoc.win1')}</li>
        <li>{t('fileAssoc.win2')}</li>
        <li>{t('fileAssoc.win3')}</li>
      </ol>
    {:else if platform === 'linux'}
      <ol class="mb-4 list-decimal space-y-1 pl-5 text-sm text-pc-text">
        <li>{t('fileAssoc.linux1', { command: 'xdg-mime default pincel.desktop application/x-aseprite' })}</li>
        <li>{t('fileAssoc.linux2', { command: 'update-desktop-database' })}</li>
      </ol>
    {:else}
      <p class="mb-4 text-sm text-pc-muted">{t('fileAssoc.other')}</p>
    {/if}
    <label class="mb-4 flex items-center gap-2 text-sm text-pc-text">
      <input type="checkbox" bind:checked={dontShowAgain} />
      {t('fileAssoc.dontShow')}
    </label>
    <div class="flex justify-end">
      <button class="toolbar-btn" onclick={() => onDismiss(dontShowAgain)}>{t('fileAssoc.gotIt')}</button>
    </div>
  </div>
</div>
