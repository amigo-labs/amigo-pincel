<script lang="ts">
  // "AI connected" in the status bar while an external MCP client has a
  // session open, with its disconnect button (docs/specs/mcp-live-desktop.md
  // AC-12). Chat sessions of the Assistant panel do not count (AC-22).
  import { t } from '../i18n/index.svelte';
  import { agentAccess, disconnectAgents } from './access.svelte';
</script>

{#if agentAccess.sessions > 0}
  <span class="flex items-center gap-1.5 text-pc-accent-text" role="status">
    <span class="h-1.5 w-1.5 rounded-full bg-pc-accent" aria-hidden="true"></span>
    {t('status.aiConnected')}
    <button
      class="rounded px-1 text-pc-muted hover:bg-pc-hover hover:text-pc-text"
      onclick={() => void disconnectAgents()}
    >
      {t('status.aiDisconnect')}
    </button>
  </span>
{/if}
