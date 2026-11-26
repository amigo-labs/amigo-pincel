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
  class="fixed inset-0 z-(--pc-z-overlay) flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="agents-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div class="am-dialog w-full max-w-lg">
    <header>
      <h2 id="agents-title">{t('agents.title')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <label class="am-check flex min-h-8 items-center gap-2">
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
        <p class="am-hint am-hint--error" role="alert">
          {t('agents.failed', { error: agentAccess.error })}
        </p>
      {/if}
      {#if enabled && status?.command && status.token}
        {#if status.portChanged}
          <div class="am-banner am-banner--warning" role="status"><div><strong>{t('agents.portChanged')}</strong></div></div>
        {/if}
        <div class="flex flex-col gap-1">
          <label class="text-xs text-pc-muted" for="agents-command">{t('agents.command')}</label>
          <div class="flex gap-2">
            <input
              id="agents-command"
              bind:this={commandField}
              class="am-input min-w-0 flex-1 font-mono"
              readonly
              value={status.command}
            />
            <button
              class="am-btn"
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
              class="am-input min-w-0 flex-1 font-mono"
              readonly
              value={status.token}
            />
            <button
              class="am-btn"
              onclick={() => status?.token && void copy('token', status.token, tokenField)}
            >
              {copied === 'token' ? t('agents.copied') : t('agents.copy')}
            </button>
          </div>
        </div>
        <p class="text-xs text-pc-faint">{t('agents.address', { url: status.url ?? '' })}</p>
      {/if}
    </div>
    <footer>
      <button
        class="am-btn mr-auto"
        disabled={!enabled || agentAccess.busy}
        onclick={() => {
          copied = null;
          void regenerateToken();
        }}
      >
        {t('agents.regenerate')}
      </button>
      <button bind:this={closeButton} class="am-btn" onclick={onClose}>
        {t('agents.close')}
      </button>
    </footer>
  </div>
</div>
