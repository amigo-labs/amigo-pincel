<script lang="ts">
  // The managed installation in View ▸ Image generation…
  // (docs/specs/comfyui-install.md): packages, update, uninstall (AC-11 –
  // AC-14), "Installation incomplete" with Resume / Delete, and the way into
  // the wizard.
  import { t } from '../i18n/index.svelte';
  import {
    install,
    openInstallWizard,
    openPackages,
    resumeIncomplete,
    startUpdate,
    uninstall,
  } from './install.svelte';
  import { comfy } from './state.svelte';

  const info = $derived(install.info);
  const managed = $derived(info?.managed ?? { kind: 'none' as const });
  // AC-14: nothing changes under a running generation or another job.
  const locked = $derived((info?.generating ?? false) || install.job !== null);
  const lockedTitle = $derived(info?.generating ? t('comfy.errGenerationRunning') : undefined);
  const names = $derived(
    managed.kind === 'complete'
      ? (info?.catalog ?? []).filter((p) => managed.models.includes(p.id)).map((p) => p.name)
      : [],
  );
</script>

{#if managed.kind === 'complete'}
  <section class="mb-4 border-t border-pc-border pt-3" aria-label={t('comfy.managedTitle')}>
    <p class="text-xs text-pc-muted">{t('comfy.managedTitle')}</p>
    <p class="truncate" title={managed.dir}>{t('comfy.managedInfo', { version: managed.comfyui, path: managed.dir })}</p>
    <p class="mb-2 text-xs text-pc-muted">{names.join(', ')}</p>
    {#if managed.updateAvailable}
      <p class="mb-2">{t('comfy.updateAvailable')}</p>
    {/if}
    {#if install.confirmUninstall}
      <div class="rounded border border-pc-border p-2" role="alertdialog" aria-label={t('comfy.uninstallTitle')}>
        <p class="mb-2">{t('comfy.uninstallConfirm', { path: managed.dir })}</p>
        <div class="flex gap-2">
          <button class="panel-btn min-h-8" onclick={() => void uninstall()}>{t('comfy.uninstallButton')}</button>
          <button class="panel-btn min-h-8" onclick={() => (install.confirmUninstall = false)}
            >{t('common.cancel')}</button
          >
        </div>
      </div>
    {:else}
      <div class="flex flex-wrap gap-2">
        <button class="panel-btn min-h-8" disabled={locked} title={lockedTitle} onclick={openPackages}
          >{t('comfy.managePackages')}</button
        >
        {#if managed.updateAvailable}
          <button class="panel-btn min-h-8" disabled={locked} title={lockedTitle} onclick={() => void startUpdate()}
            >{t('comfy.update')}</button
          >
        {/if}
        <button
          class="panel-btn min-h-8"
          disabled={locked}
          title={lockedTitle}
          onclick={() => (install.confirmUninstall = true)}>{t('comfy.uninstall')}</button
        >
      </div>
    {/if}
  </section>
{:else if managed.kind === 'incomplete'}
  <section class="mb-4 border-t border-pc-border pt-3" aria-label={t('comfy.managedTitle')}>
    <p class="text-pc-danger-text">{t('comfy.incomplete')}</p>
    <p class="mb-2 truncate text-xs text-pc-muted" title={managed.dir}>{managed.dir}</p>
    <div class="flex flex-wrap gap-2">
      <button class="panel-btn min-h-8" disabled={locked} onclick={() => void resumeIncomplete()}
        >{t('comfy.resume')}</button
      >
      <button class="panel-btn min-h-8" disabled={locked} onclick={() => void uninstall()}>{t('comfy.delete')}</button>
    </div>
  </section>
{:else if managed.kind === 'none' && comfy.status.kind !== 'notFound' && info !== null}
  <div class="mb-4 border-t border-pc-border pt-3">
    <button class="panel-btn min-h-8" onclick={() => openInstallWizard()}>{t('comfy.install')}</button>
  </div>
{/if}
