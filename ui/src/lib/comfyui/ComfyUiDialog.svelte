<script lang="ts">
  // View ▸ Image generation… (docs/specs/comfyui-connect.md): server choice,
  // status, model list. The backend (src-tauri/src/comfyui/) does the work.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import type { MessageKey } from '../i18n/en';
  import type { FolderProblem, ServerChoice } from './bridge';
  import {
    chooseServer,
    comfy,
    isLoopbackUrl,
    pickFolder,
    retryStart,
    testConnection,
  } from './state.svelte';
  import InstallWizard from './InstallWizard.svelte';
  import ManagedSection from './ManagedSection.svelte';
  import { chooseAnotherServer, install, installAgain, loadInstall, openInstallWizard } from './install.svelte';

  let { onClose }: { onClose: () => void } = $props();

  let closeButton: HTMLButtonElement | undefined = $state();

  // The dialog owns the keyboard: keys must not reach the editor behind it.
  onMount(() => {
    closeButton?.focus();
    void loadInstall();
  });

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  const FOLDER_PROBLEM: Record<FolderProblem, MessageKey> = {
    notFound: 'comfy.folderNotFound',
    noComfyUi: 'comfy.folderNoComfyUi',
    noPython: 'comfy.folderNoPython',
    desktop: 'comfy.folderDesktop',
  };

  const choice = $derived<ServerChoice>(comfy.settings?.server ?? 'automatic');
  const choices = $derived<[ServerChoice, MessageKey][]>([
    ['automatic', 'comfy.automatic'],
    ['existing', 'comfy.existing'],
    ['custom', 'comfy.custom'],
    // Offered once comfyui-install has installed one.
    ...(comfy.settings?.managedDir ? [['managed', 'comfy.managed'] as [ServerChoice, MessageKey]] : []),
  ]);
  const remote = $derived(choice === 'custom' && isLoopbackUrl(comfy.url) === false);
  const status = $derived(comfy.status);

  function gb(bytes: number): string {
    return (bytes / 1024 ** 3).toFixed(1);
  }

  // comfyui-install edge case: CUDA's error for an old driver names it.
  function namesDriver(log: string[]): boolean {
    return log.some((line) => /nvidia driver|driver version is insufficient|cuda driver/i.test(line));
  }
</script>

{#snippet folderButton(label: MessageKey)}
  <button class="am-btn" onclick={() => void pickFolder()}>{t(label)}</button>
{/snippet}

{#if install.open}
  <InstallWizard />
{:else}
<div
  class="fixed inset-0 z-(--pc-z-overlay) flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="comfy-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div class="am-dialog max-h-full w-[28rem] max-w-full overflow-y-auto">
    <header>
      <h2 id="comfy-title">{t('comfy.title')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">

      <fieldset>
        <legend class="mb-1 text-xs text-pc-muted">{t('comfy.server')}</legend>
        <div class="flex flex-col gap-1" role="radiogroup" aria-label={t('comfy.server')}>
          {#each choices as [value, label] (value)}
            <label class="am-check flex min-h-8 items-center gap-2">
              <input
                type="radio"
                name="comfy-server"
                {value}
                checked={choice === value}
                onchange={() => void chooseServer(value)}
              />
              {t(label)}
            </label>
          {/each}
        </div>
      </fieldset>

      {#if choice === 'existing'}
        <div class="mb-3 flex items-center gap-2">
          <span class="min-w-0 flex-1 truncate text-xs text-pc-muted" title={comfy.settings?.existingDir ?? ''}
            >{comfy.settings?.existingDir ?? t('comfy.noFolder')}</span
          >
          {@render folderButton('comfy.chooseFolder')}
        </div>
      {:else if choice === 'custom'}
        <form
          class="mb-3 flex items-center gap-2"
          onsubmit={(e) => {
            e.preventDefault();
            void testConnection();
          }}
        >
          <input
            type="text"
            bind:value={comfy.url}
            placeholder="http://127.0.0.1:8188"
            spellcheck="false"
            autocomplete="off"
            class="am-input min-w-0 flex-1"
            aria-label={t('comfy.url')}
          />
          <button type="submit" class="am-btn" disabled={comfy.busy}>{t('comfy.test')}</button>
        </form>
        {#if remote}
          <div class="am-banner am-banner--warning mb-3" role="note"><div><strong>{t('comfy.remoteNotice')}</strong></div></div>
        {/if}
      {/if}

      {#if comfy.folderProblem !== null}
        <div class="am-banner am-banner--danger mb-2" role="alert"><div><strong>{t(FOLDER_PROBLEM[comfy.folderProblem])}</strong></div></div>
      {/if}

      <div class="border-t border-pc-border pt-3" role="status" aria-live="polite">
        {#if comfy.error !== null}
          <div class="am-banner am-banner--danger"><div><strong>{t('comfy.error', { error: comfy.error })}</strong></div></div>
        {:else if status.kind === 'starting'}
          <p>{t('comfy.starting')}</p>
        {:else if comfy.busy && status.kind !== 'connected'}
          <p class="text-pc-muted">{t('comfy.connecting')}</p>
        {:else if status.kind === 'idle'}
          {#if choice === 'custom'}
            <p class="text-pc-muted">{t('comfy.enterUrl')}</p>
          {/if}
        {:else if status.kind === 'notConfigured'}
          <p class="text-pc-muted">{t(choice === 'custom' ? 'comfy.enterUrl' : 'comfy.noFolder')}</p>
        {:else if status.kind === 'notFound'}
          <p class="mb-2">{t('comfy.notFound')}</p>
          <div class="flex flex-wrap gap-2">
            <button
              class="am-btn"
              onclick={() => void chooseServer('existing').then(() => pickFolder())}
              >{t('comfy.useExisting')}</button
            >
            <button class="am-btn" disabled={install.info === null} onclick={() => openInstallWizard()}
              >{t('comfy.install')}</button
            >
          </div>
        {:else if status.kind === 'connected'}
          {@const server = status.server}
          {#if server.missingNodes.length > 0}
            <div class="am-banner am-banner--danger"><div><strong>{t('comfy.tooOld')}</strong></div></div>
            <p class="mb-2 text-xs text-pc-muted">
              {t('comfy.missingNodes', { nodes: server.missingNodes.join(', ') })}
            </p>
          {/if}
          <p class="text-xs text-pc-muted">{t('comfy.connected', { url: server.url })}</p>
          <p>{t('comfy.version', { version: server.version })}</p>
          <p class="mb-2">
            {server.gpu
              ? t('comfy.gpu', { name: server.gpu.name, vram: gb(server.gpu.vramTotal) })
              : t('comfy.noGpu')}
          </p>
          {#if server.checkpoints.length === 0}
            <div class="am-banner am-banner--danger mb-2"><div><strong>{t('comfy.noModel')}</strong></div></div>
          {:else}
            <p class="text-xs text-pc-muted">{t('comfy.checkpoints')}</p>
            <ul class="mb-2 list-inside list-disc" aria-label={t('comfy.checkpoints')}>
              {#each server.checkpoints as name (name)}<li>{name}</li>{/each}
            </ul>
          {/if}
          <p class="text-xs text-pc-muted">{t('comfy.loras')}</p>
          <ul class="list-inside list-disc" aria-label={t('comfy.loras')}>
            {#each server.loras as name (name)}<li>{name}</li>{:else}<li>{t('comfy.none')}</li>{/each}
          </ul>
        {:else if status.kind === 'stopped'}
          <div class="am-banner am-banner--danger mb-2"><div><strong>{t('comfy.stopped')}</strong></div></div>
          {#if namesDriver(status.log)}
            <div class="am-banner am-banner--warning mb-2"><div><strong>{t('comfy.driverHint')}</strong></div></div>
          {/if}
          {#if choice === 'managed'}
            <button class="am-btn mb-2" onclick={retryStart}>{t('comfy.retryStart')}</button>
          {/if}
          {#if status.log.length > 0}
            <p class="text-xs text-pc-muted">{t('comfy.output')}</p>
            <pre
              class="max-h-48 overflow-auto rounded-pc-control border border-pc-border bg-pc-bg p-2 text-xs"
              aria-label={t('comfy.output')}>{status.log.join('\n')}</pre>
          {/if}
        {:else if status.kind === 'installationNotFound'}
          <div class="am-banner am-banner--danger mb-2"><div><strong>{t('comfy.installationNotFound', { path: status.path })}</strong></div></div>
          {#if choice === 'managed'}
            {@const path = status.path}
            <div class="flex flex-wrap gap-2">
              <button class="am-btn" onclick={() => installAgain(path)}>{t('comfy.installAgain')}</button>
              <button class="am-btn" onclick={() => void chooseAnotherServer()}
                >{t('comfy.chooseServer')}</button
              >
            </div>
          {:else}
            {@render folderButton('comfy.chooseAnother')}
          {/if}
        {:else if status.kind === 'installationInvalid'}
          <div class="am-banner am-banner--danger mb-2"><div><strong>{t(FOLDER_PROBLEM[status.problem])}</strong></div></div>
          {@render folderButton('comfy.chooseAnother')}
        {:else if status.kind === 'invalidUrl'}
          <div class="am-banner am-banner--danger"><div><strong>{t('comfy.invalidUrl')}</strong></div></div>
        {:else if status.kind === 'unreachable'}
          <div class="am-banner am-banner--danger"><div><strong>{t('comfy.unreachable')}</strong></div></div>
        {:else if status.kind === 'notComfy'}
          <div class="am-banner am-banner--danger"><div><strong>{t('comfy.notComfy')}</strong></div></div>
        {:else if status.kind === 'refused'}
          <div class="am-banner am-banner--danger"><div><strong>{t('comfy.refused', { status: status.status })}</strong></div></div>
        {:else if status.kind === 'failed'}
          <div class="am-banner am-banner--danger"><div><strong>{t('comfy.failed', { error: status.message })}</strong></div></div>
        {/if}
      </div>

      <ManagedSection />

    </div>
    <footer>
      <button bind:this={closeButton} class="am-btn" onclick={onClose}>
        {t('common.close')}
      </button>
    </footer>
  </div>
</div>
{/if}
