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
    <section aria-label={t('comfy.hardware')}>
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
        <div class="am-banner am-banner--danger">
          <div>
            <strong>
              {t('comfy.unsupported', {
                reason: t(REASONS[reason.kind], reason.kind === 'otherGpu' ? { name: reason.name } : {}),
              })}
            </strong>
          </div>
        </div>
        <div class="mt-2 flex flex-wrap gap-2">
          <button class="am-btn" onclick={() => void existing()}>{t('comfy.useExisting')}</button>
          <button class="am-btn" onclick={() => void custom()}>{t('comfy.custom')}</button>
        </div>
      {:else}
        <p>{t('comfy.supported')}</p>
        {#if hardware.slow}
          <div class="am-banner am-banner--warning" role="note"><div><strong>{t('comfy.slow')}</strong></div></div>
        {/if}
      {/if}
    </section>
  {/if}
{/snippet}

{#snippet locationBlock()}
  <section aria-label={t('comfy.location')}>
    <p class="text-xs text-pc-muted">{t('comfy.location')}</p>
    <div class="flex items-center gap-2">
      <span class="min-w-0 flex-1 truncate" title={check?.target ?? ''}
        >{check ? t('comfy.target', { path: check.target }) : t('comfy.noLocation')}</span
      >
      <button class="am-btn" onclick={() => void pickLocation()}>{t('comfy.chooseLocation')}</button>
    </div>
    {#if check !== null}
      <p class:text-pc-danger-text={short}>
        {check.free === null
          ? t('comfy.spaceUnknown', { required: bytes(need) })
          : t('comfy.space', { free: bytes(check.free), required: bytes(need) })}
      </p>
      {#if short}<div class="am-banner am-banner--danger"><div><strong>{t('comfy.spaceShort')}</strong></div></div>{/if}
      {#if !check.writable}<div class="am-banner am-banner--danger"><div><strong>{t('comfy.notWritable')}</strong></div></div>{/if}
      {#if check.existing === 'complete'}
        <p>{t('comfy.existingComplete')}</p>
        <button class="am-btn mt-1" onclick={() => void useThisInstallation()}>{t('comfy.useThis')}</button>
      {:else if check.existing === 'incomplete'}
        <p class="text-pc-muted">{t('comfy.existingIncomplete')}</p>
      {/if}
    {/if}
  </section>
{/snippet}

{#snippet packagesBlock()}
  <fieldset>
    <legend class="mb-1 text-xs text-pc-muted">{t('comfy.packages')}</legend>
    {#each catalog as pkg (pkg.id)}
      {@const checked = install.selected.includes(pkg.id)}
      {@const min = hardware ? minimum(pkg, hardware) : null}
      <div class="mb-1">
        <label class="am-check flex min-h-8 items-center gap-2">
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
      <div class="am-banner am-banner--warning mt-2" role="alertdialog" aria-label={t('comfy.removeTitle')}>
        <div>
          <p class="mb-2">
            {t('comfy.removeLora', {
              name: removing.name,
              others: catalog
                .filter((p) => p.requires === removing.id)
                .map((p) => p.name)
                .join(', '),
            })}
          </p>
          <div class="flex justify-end gap-2">
            <button class="am-btn" onclick={() => confirmRemoval(false)}>{t('comfy.keep')}</button>
            <button class="am-btn am-btn--danger" onclick={() => confirmRemoval(true)}>{t('comfy.removeBoth')}</button>
          </div>
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
  <div class="am-dialog max-h-full w-[30rem] max-w-full overflow-y-auto">
    <header>
      <h2 id="comfy-install-title" tabindex="-1" bind:this={first}>
        {t(install.mode === 'change' ? 'comfy.packagesTitle' : 'comfy.installTitle')}
      </h2>
    </header>

    <div class="am-body flex flex-col gap-3">
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
      {:else if install.page === 'licenses'}
        <section aria-label={t('comfy.licensesTitle')}>
          {#each pending as pkg (pkg.id)}
            <div class="mb-2">
              <p>{t('comfy.licenseOf', { name: pkg.name, license: pkg.license })}</p>
              <p class="break-all text-xs text-pc-muted">{pkg.licenseUrl}</p>
              <label class="am-check flex min-h-8 items-center gap-2">
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
      {:else if install.page === 'progress'}
        <div class="flex flex-col gap-2" role="status" aria-live="polite">
          {#if progress === null}
            <p>{t('comfy.preparing')}</p>
          {:else}
            <p class="font-semibold">{stepText(progress, catalog)}</p>
            {#if progress.done !== null && progress.total !== null}
              {@const total = Math.max(progress.total, 1)}
              <div
                class="am-progress"
                role="progressbar"
                aria-label={stepText(progress, catalog)}
                aria-valuemin={0}
                aria-valuemax={total}
                aria-valuenow={Math.min(progress.done, total)}
              >
                <i style:width="{(Math.min(progress.done, total) / total) * 100}%"></i>
              </div>
              <p class="text-xs">{t('comfy.bytes', { done: bytes(progress.done), total: bytes(progress.total) })}</p>
            {:else if progress.line !== null}
              <p class="truncate font-mono text-xs text-pc-muted" title={progress.line}>{progress.line}</p>
            {/if}
          {/if}
        </div>
      {:else if install.page === 'failed' && install.error !== null}
        {@const error = install.error}
        <div class="am-banner am-banner--danger" role="alert">
          <div>
            <strong>{errorText(error)}</strong>
            {#if (error.kind === 'setupFailed' || error.kind === 'updateNotStarted') && error.log.length > 0}
              <p class="mt-2 text-xs text-pc-muted">{t('comfy.output')}</p>
              <pre
                class="max-h-48 overflow-auto rounded-pc-control border border-pc-border bg-pc-bg p-2 text-xs"
                aria-label={t('comfy.output')}>{error.log.join('\n')}</pre>
            {/if}
          </div>
        </div>
      {/if}
    </div>

    <footer>
      {#if install.page === 'setup'}
        <button class="am-btn" onclick={closeWizard}>{t('common.cancel')}</button>
        {#if install.mode === 'install'}
          {#if hardware?.unsupported === null}
            <button class="am-btn am-btn--primary" disabled={!canInstall} onclick={toLicenses}
              >{t('comfy.installButton')}</button
            >
          {/if}
        {:else}
          <button class="am-btn am-btn--primary" disabled={!changed} onclick={toLicenses}>{t('common.apply')}</button>
        {/if}
      {:else if install.page === 'licenses'}
        <button class="am-btn" onclick={() => (install.page = 'setup')}>{t('comfy.back')}</button>
        <button class="am-btn am-btn--primary" disabled={!allAccepted} onclick={() => void startJob()}>
          {t(install.mode === 'install' ? 'comfy.installButton' : 'common.apply')}
        </button>
      {:else if install.page === 'progress'}
        <button class="am-btn" onclick={cancelJob}>{t('common.cancel')}</button>
      {:else if install.page === 'failed' && install.error !== null}
        <button class="am-btn" onclick={closeWizard}>{t('common.close')}</button>
        {#if install.error.kind !== 'generationRunning' && install.error.kind !== 'busy'}
          <button class="am-btn am-btn--primary" onclick={() => void retry()}>{t('comfy.retry')}</button>
        {/if}
      {/if}
    </footer>
  </div>
</div>
