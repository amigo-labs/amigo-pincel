<script lang="ts">
  // View ▸ AI agents… (docs/specs/mcp-live-desktop.md AC-02, AC-04, AC-20,
  // AC-21): the switch, the client command and the token to copy, and
  // "Regenerate token". Desktop app only.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import { agentAccess, regenerateToken, setAgentAccess } from './access.svelte';

  let { onClose }: { onClose: () => void } = $props();

  let closeButton: HTMLButtonElement | undefined = $state();
  let copied = $state<'command' | 'token' | null>(null);

  const status = $derived(agentAccess.status);
  const enabled = $derived(status?.enabled ?? false);

  onMount(() => closeButton?.focus());

  function onKeydown(e: KeyboardEvent): void {
    // Keys stay in the dialog: Delete or Ctrl+Z must not reach the editor.
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  async function copy(which: 'command' | 'token', text: string, field: HTMLInputElement | null) {
    try {
      await navigator.clipboard.writeText(text);
      copied = which;
    } catch {
      // No clipboard access: select the text for Ctrl+C.
      field?.select();
    }
  }

  let commandField: HTMLInputElement | null = $state(null);
  let tokenField: HTMLInputElement | null = $state(null);
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="agents-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div
    class="flex w-full max-w-lg flex-col gap-3 rounded border border-pc-border bg-pc-panel p-5 text-sm text-pc-text shadow-xl"
  >
    <h2 id="agents-title" class="text-base font-semibold">{t('agents.title')}</h2>
    <label class="flex min-h-8 items-center gap-2">
      <input
        type="checkbox"
        checked={enabled}
        disabled={agentAccess.busy}
        onchange={(e) => void setAgentAccess(e.currentTarget.checked)}
      />
      {t('agents.allow')}
    </label>
    <p class="text-xs text-pc-muted">{t('agents.help')}</p>
    {#if agentAccess.error !== null}
      <p class="text-xs text-pc-danger-text" role="alert">
        {t('agents.failed', { error: agentAccess.error })}
      </p>
    {/if}
    {#if enabled && status?.command && status.token}
      {#if status.portChanged}
        <p class="text-xs text-pc-accent-text" role="status">{t('agents.portChanged')}</p>
      {/if}
      <div class="flex flex-col gap-1">
        <label class="text-xs text-pc-muted" for="agents-command">{t('agents.command')}</label>
        <div class="flex gap-2">
          <input
            id="agents-command"
            bind:this={commandField}
            class="min-w-0 flex-1 rounded border border-pc-border bg-pc-bg px-2 py-1 font-mono text-xs"
            readonly
            value={status.command}
          />
          <button
            class="panel-btn"
            onclick={() => status?.command && void copy('command', status.command, commandField)}
          >
            {copied === 'command' ? t('agents.copied') : t('agents.copy')}
          </button>
        </div>
      </div>
      <div class="flex flex-col gap-1">
        <label class="text-xs text-pc-muted" for="agents-token">{t('agents.token')}</label>
        <div class="flex gap-2">
          <input
            id="agents-token"
            bind:this={tokenField}
            class="min-w-0 flex-1 rounded border border-pc-border bg-pc-bg px-2 py-1 font-mono text-xs"
            readonly
            value={status.token}
          />
          <button
            class="panel-btn"
            onclick={() => status?.token && void copy('token', status.token, tokenField)}
          >
            {copied === 'token' ? t('agents.copied') : t('agents.copy')}
          </button>
        </div>
      </div>
      <p class="text-xs text-pc-faint">{t('agents.address', { url: status.url ?? '' })}</p>
    {/if}
    <div class="flex justify-between gap-2">
      <button
        class="panel-btn"
        disabled={!enabled || agentAccess.busy}
        onclick={() => {
          copied = null;
          void regenerateToken();
        }}
      >
        {t('agents.regenerate')}
      </button>
      <button bind:this={closeButton} class="panel-btn" onclick={onClose}>
        {t('agents.close')}
      </button>
    </div>
  </div>
</div>
