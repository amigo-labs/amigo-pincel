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
    testConnection,
  } from './state.svelte';

  let { onClose }: { onClose: () => void } = $props();

  let closeButton: HTMLButtonElement | undefined = $state();

  // The dialog owns the keyboard: keys must not reach the editor behind it.
  onMount(() => closeButton?.focus());

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
</script>

{#snippet folderButton(label: MessageKey)}
  <button class="panel-btn min-h-8" onclick={() => void pickFolder()}>{t(label)}</button>
{/snippet}

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="comfy-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="max-h-full w-[28rem] max-w-full overflow-y-auto rounded border border-pc-border bg-pc-panel p-5 text-sm text-pc-text shadow-xl"
  >
    <h2 id="comfy-title" class="mb-3 text-lg font-semibold">{t('comfy.title')}</h2>

    <fieldset class="mb-3">
      <legend class="mb-1 text-xs text-pc-muted">{t('comfy.server')}</legend>
      <div class="flex flex-col gap-1" role="radiogroup" aria-label={t('comfy.server')}>
        {#each choices as [value, label] (value)}
          <label class="flex min-h-8 items-center gap-2">
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
          class="h-8 min-w-0 flex-1 rounded border border-pc-border bg-pc-bg px-1.5 text-pc-text"
          aria-label={t('comfy.url')}
        />
        <button type="submit" class="panel-btn min-h-8" disabled={comfy.busy}>{t('comfy.test')}</button>
      </form>
      {#if remote}
        <p class="mb-3 text-xs text-pc-accent-text" role="note">{t('comfy.remoteNotice')}</p>
      {/if}
    {/if}

    {#if comfy.folderProblem !== null}
      <p class="mb-3 text-pc-danger-text" role="alert">{t(FOLDER_PROBLEM[comfy.folderProblem])}</p>
    {/if}

    <div class="mb-4 border-t border-pc-border pt-3" role="status" aria-live="polite">
      {#if comfy.error !== null}
        <p class="text-pc-danger-text">{t('comfy.error', { error: comfy.error })}</p>
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
            class="panel-btn min-h-8"
            onclick={() => void chooseServer('existing').then(() => pickFolder())}
            >{t('comfy.useExisting')}</button
          >
          <button class="panel-btn min-h-8" disabled title={t('comfy.installLater')}
            >{t('comfy.install')}</button
          >
        </div>
      {:else if status.kind === 'connected'}
        {@const server = status.server}
        {#if server.missingNodes.length > 0}
          <p class="text-pc-danger-text">{t('comfy.tooOld')}</p>
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
          <p class="mb-2 text-pc-danger-text">{t('comfy.noModel')}</p>
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
        <p class="mb-2 text-pc-danger-text">{t('comfy.stopped')}</p>
        {#if status.log.length > 0}
          <p class="text-xs text-pc-muted">{t('comfy.output')}</p>
          <pre
            class="max-h-48 overflow-auto rounded border border-pc-border bg-pc-bg p-2 text-xs"
            aria-label={t('comfy.output')}>{status.log.join('\n')}</pre>
        {/if}
      {:else if status.kind === 'installationNotFound'}
        <p class="mb-2 text-pc-danger-text">{t('comfy.installationNotFound', { path: status.path })}</p>
        {@render folderButton('comfy.chooseAnother')}
      {:else if status.kind === 'installationInvalid'}
        <p class="mb-2 text-pc-danger-text">{t(FOLDER_PROBLEM[status.problem])}</p>
        {@render folderButton('comfy.chooseAnother')}
      {:else if status.kind === 'invalidUrl'}
        <p class="text-pc-danger-text">{t('comfy.invalidUrl')}</p>
      {:else if status.kind === 'unreachable'}
        <p class="text-pc-danger-text">{t('comfy.unreachable')}</p>
      {:else if status.kind === 'notComfy'}
        <p class="text-pc-danger-text">{t('comfy.notComfy')}</p>
      {:else if status.kind === 'refused'}
        <p class="text-pc-danger-text">{t('comfy.refused', { status: status.status })}</p>
      {:else if status.kind === 'unsupported'}
        <p class="text-pc-danger-text">{t('comfy.unsupported')}</p>
      {:else if status.kind === 'failed'}
        <p class="text-pc-danger-text">{t('comfy.failed', { error: status.message })}</p>
      {/if}
    </div>

    <div class="flex justify-end">
      <button bind:this={closeButton} class="panel-btn min-h-8" onclick={onClose}>
        {t('common.close')}
      </button>
    </div>
  </div>
</div>
