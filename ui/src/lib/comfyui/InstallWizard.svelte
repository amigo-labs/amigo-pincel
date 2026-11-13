<script lang="ts">
  // "Install ComfyUI…" (docs/specs/comfyui-install.md): hardware check,
  // location, packages, licenses, progress, errors. Also the package list
  // for adding and removing packages later (AC-11).
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import type { MessageKey } from '../i18n/en';
  import type { Unsupported } from './install-bridge';
  import {
    bytes,
    cancelJob,
    closeWizard,
    confirmRemoval,
    errorText,
    gib,
    install,
    pickLocation,
    required,
    retry,
    setPackage,
    startJob,
    stepText,
    toLicenses,
    unaccepted,
    useThisInstallation,
  } from './install.svelte';
  import { canDeselect, minimum } from './selection';
  import { chooseServer, pickFolder } from './state.svelte';

  let first: HTMLElement | undefined = $state();
  onMount(() => first?.focus());

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      closeWizard();
    }
  }

  const REASONS: Record<Unsupported['kind'], MessageKey> = {
    otherGpu: 'comfy.reasonOtherGpu',
    noNvidiaDriver: 'comfy.reasonNoNvidiaDriver',
    intelMac: 'comfy.reasonIntelMac',
    armCpu: 'comfy.reasonArmCpu',
    otherOs: 'comfy.reasonOtherOs',
  };
  const OS: Record<string, MessageKey> = {
    windows: 'comfy.osWindows',
    linux: 'comfy.osLinux',
    macos: 'comfy.osMacos',
    other: 'comfy.osOther',
  };

  const info = $derived(install.info);
  const hardware = $derived(info?.hardware ?? null);
  const catalog = $derived(info?.catalog ?? []);
  const check = $derived(install.check);
  const need = $derived(required());
  const short = $derived(check !== null && check.free !== null && check.free < need);
  const installed = $derived(info?.managed.kind === 'complete' ? info.managed.models : []);
  const changed = $derived(
    install.selected.length !== installed.length || install.selected.some((id) => !installed.includes(id)),
  );
  const canInstall = $derived(
    hardware !== null &&
      hardware.unsupported === null &&
      check !== null &&
      check.writable &&
      !short &&
      check.existing !== 'complete',
  );
  const pending = $derived(install.page === 'licenses' ? unaccepted() : []);
  const allAccepted = $derived(pending.every((p) => install.accepting.includes(p.id)));
  const progress = $derived(install.progress);
  const removing = $derived(catalog.find((p) => p.id === install.confirmRemove) ?? null);

  async function existing(): Promise<void> {
    closeWizard();
    await chooseServer('existing');
    await pickFolder();
  }

  async function custom(): Promise<void> {
    closeWizard();
    await chooseServer('custom');
  }
</script>

{#snippet hardwareBlock()}
  {#if hardware !== null}
    <section class="mb-3" aria-label={t('comfy.hardware')}>
      <p class="text-xs text-pc-muted">{t('comfy.hardware')}</p>
      <p>
        {t(OS[hardware.os] ?? 'comfy.osOther')}
        {#if hardware.device !== null}
          · {hardware.device.backend === 'mps'
            ? t('comfy.hwMac', { name: hardware.device.name, memory: gib(hardware.device.memory) })
            : t('comfy.gpu', { name: hardware.device.name, vram: gib(hardware.device.memory) })}
        {/if}
      </p>
      {#if hardware.unsupported !== null}
        {@const reason = hardware.unsupported}
        <p class="text-pc-danger-text">
          {t('comfy.unsupported', {
            reason: t(REASONS[reason.kind], reason.kind === 'otherGpu' ? { name: reason.name } : {}),
          })}
        </p>
        <div class="mt-2 flex flex-wrap gap-2">
          <button class="panel-btn min-h-8" onclick={() => void existing()}>{t('comfy.useExisting')}</button>
          <button class="panel-btn min-h-8" onclick={() => void custom()}>{t('comfy.custom')}</button>
        </div>
      {:else}
        <p>{t('comfy.supported')}</p>
        {#if hardware.slow}
          <p class="text-pc-accent-text" role="note">{t('comfy.slow')}</p>
        {/if}
      {/if}
    </section>
  {/if}
{/snippet}

{#snippet locationBlock()}
  <section class="mb-3" aria-label={t('comfy.location')}>
    <p class="text-xs text-pc-muted">{t('comfy.location')}</p>
    <div class="flex items-center gap-2">
      <span class="min-w-0 flex-1 truncate" title={check?.target ?? ''}
        >{check ? t('comfy.target', { path: check.target }) : t('comfy.noLocation')}</span
      >
      <button class="panel-btn min-h-8" onclick={() => void pickLocation()}>{t('comfy.chooseLocation')}</button>
    </div>
    {#if check !== null}
      <p class:text-pc-danger-text={short}>
        {check.free === null
          ? t('comfy.spaceUnknown', { required: bytes(need) })
          : t('comfy.space', { free: bytes(check.free), required: bytes(need) })}
      </p>
      {#if short}<p class="text-pc-danger-text">{t('comfy.spaceShort')}</p>{/if}
      {#if !check.writable}<p class="text-pc-danger-text">{t('comfy.notWritable')}</p>{/if}
      {#if check.existing === 'complete'}
        <p>{t('comfy.existingComplete')}</p>
        <button class="panel-btn mt-1 min-h-8" onclick={() => void useThisInstallation()}>{t('comfy.useThis')}</button>
      {:else if check.existing === 'incomplete'}
        <p class="text-pc-muted">{t('comfy.existingIncomplete')}</p>
      {/if}
    {/if}
  </section>
{/snippet}

{#snippet packagesBlock()}
  <fieldset class="mb-3">
    <legend class="mb-1 text-xs text-pc-muted">{t('comfy.packages')}</legend>
    {#each catalog as pkg (pkg.id)}
      {@const checked = install.selected.includes(pkg.id)}
      {@const min = hardware ? minimum(pkg, hardware) : null}
      <div class="mb-1">
        <label class="flex min-h-8 items-center gap-2">
          <input
            type="checkbox"
            {checked}
            disabled={checked && !canDeselect(catalog, install.selected, pkg.id)}
            onchange={(e) => {
              setPackage(pkg.id, e.currentTarget.checked);
              // A refused or pending change keeps the box as the selection says.
              e.currentTarget.checked = install.selected.includes(pkg.id);
            }}
          />
          {pkg.name}
        </label>
        <p class="ml-6 text-xs text-pc-muted">
          {bytes(pkg.size)} · {t('comfy.packageLicense', { license: pkg.license })}
          {#if min !== null}
            ·
            <span class={min.below ? 'font-semibold text-pc-danger-text' : ''} data-below={min.below}
              >{t(min.mac ? 'comfy.minMemory' : 'comfy.minVram', { gb: gib(min.bytes) })}</span
            >
          {/if}
        </p>
      </div>
    {/each}
    {#if removing !== null}
      <div class="mt-2 rounded border border-pc-border p-2" role="alertdialog" aria-label={t('comfy.removeTitle')}>
        <p class="mb-2">
          {t('comfy.removeLora', {
            name: removing.name,
            others: catalog
              .filter((p) => p.requires === removing.id)
              .map((p) => p.name)
              .join(', '),
          })}
        </p>
        <div class="flex gap-2">
          <button class="panel-btn min-h-8" onclick={() => confirmRemoval(true)}>{t('comfy.removeBoth')}</button>
          <button class="panel-btn min-h-8" onclick={() => confirmRemoval(false)}>{t('comfy.keep')}</button>
        </div>
      </div>
    {/if}
  </fieldset>
{/snippet}

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="comfy-install-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="max-h-full w-[30rem] max-w-full overflow-y-auto rounded border border-pc-border bg-pc-panel p-5 text-sm text-pc-text shadow-xl"
  >
    <h2 id="comfy-install-title" class="mb-3 text-lg font-semibold" tabindex="-1" bind:this={first}>
      {t(install.mode === 'change' ? 'comfy.packagesTitle' : 'comfy.installTitle')}
    </h2>

    {#if install.page === 'setup'}
      {#if install.mode === 'install'}
        {@render hardwareBlock()}
        {#if hardware?.unsupported === null}
          {@render locationBlock()}
        {/if}
      {/if}
      {#if hardware?.unsupported === null}
        {@render packagesBlock()}
      {/if}
      <div class="flex justify-end gap-2">
        <button class="panel-btn min-h-8" onclick={closeWizard}>{t('common.cancel')}</button>
        {#if install.mode === 'install'}
          {#if hardware?.unsupported === null}
            <button class="panel-btn min-h-8" disabled={!canInstall} onclick={toLicenses}
              >{t('comfy.installButton')}</button
            >
          {/if}
        {:else}
          <button class="panel-btn min-h-8" disabled={!changed} onclick={toLicenses}>{t('common.apply')}</button>
        {/if}
      </div>
    {:else if install.page === 'licenses'}
      <section class="mb-3" aria-label={t('comfy.licensesTitle')}>
        {#each pending as pkg (pkg.id)}
          <div class="mb-2">
            <p>{t('comfy.licenseOf', { name: pkg.name, license: pkg.license })}</p>
            <p class="break-all text-xs text-pc-muted">{pkg.licenseUrl}</p>
            <label class="flex min-h-8 items-center gap-2">
              <input
                type="checkbox"
                checked={install.accepting.includes(pkg.id)}
                onchange={(e) => {
                  install.accepting = e.currentTarget.checked
                    ? [...install.accepting, pkg.id]
                    : install.accepting.filter((id) => id !== pkg.id);
                }}
              />
              {t('comfy.accept', { name: pkg.name })}
            </label>
          </div>
        {/each}
        {#if install.mode === 'install'}
          <p class="text-xs text-pc-muted" role="note">{t('comfy.separateNotice')}</p>
        {/if}
      </section>
      <div class="flex justify-end gap-2">
        <button class="panel-btn min-h-8" onclick={() => (install.page = 'setup')}>{t('comfy.back')}</button>
        <button class="panel-btn min-h-8" disabled={!allAccepted} onclick={() => void startJob()}>
          {t(install.mode === 'install' ? 'comfy.installButton' : 'common.apply')}
        </button>
      </div>
    {:else if install.page === 'progress'}
      <div class="mb-3" role="status" aria-live="polite">
        {#if progress === null}
          <p>{t('comfy.preparing')}</p>
        {:else}
          <p class="font-semibold">{stepText(progress, catalog)}</p>
          {#if progress.done !== null && progress.total !== null}
            <progress class="w-full" value={progress.done} max={Math.max(progress.total, 1)}></progress>
            <p class="text-xs">{t('comfy.bytes', { done: bytes(progress.done), total: bytes(progress.total) })}</p>
          {:else if progress.line !== null}
            <p class="truncate font-mono text-xs text-pc-muted" title={progress.line}>{progress.line}</p>
          {/if}
        {/if}
      </div>
      <div class="flex justify-end">
        <button class="panel-btn min-h-8" onclick={cancelJob}>{t('common.cancel')}</button>
      </div>
    {:else if install.page === 'failed' && install.error !== null}
      {@const error = install.error}
      <div class="mb-3" role="alert">
        <p class="text-pc-danger-text">{errorText(error)}</p>
        {#if (error.kind === 'setupFailed' || error.kind === 'updateNotStarted') && error.log.length > 0}
          <p class="mt-2 text-xs text-pc-muted">{t('comfy.output')}</p>
          <pre
            class="max-h-48 overflow-auto rounded border border-pc-border bg-pc-bg p-2 text-xs"
            aria-label={t('comfy.output')}>{error.log.join('\n')}</pre>
        {/if}
      </div>
      <div class="flex justify-end gap-2">
        <button class="panel-btn min-h-8" onclick={closeWizard}>{t('common.close')}</button>
        {#if error.kind !== 'generationRunning' && error.kind !== 'busy'}
          <button class="panel-btn min-h-8" onclick={() => void retry()}>{t('comfy.retry')}</button>
        {/if}
      </div>
    {/if}
  </div>
</div>
