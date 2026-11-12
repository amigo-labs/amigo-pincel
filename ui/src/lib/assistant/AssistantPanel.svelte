<script lang="ts">
  // The Assistant panel of both editors in the desktop app
  // (docs/specs/ai-chat-panel.md): the active tab's chat, its input with
  // Send / Stop, the CLI picker, the model field, the image switch, New
  // chat and the system-prompt editor. Agent text is rendered as text
  // nodes only, never as HTML (AC-21).
  import IconButton from '../components/layout/IconButton.svelte';
  import Panel from '../components/layout/Panel.svelte';
  import Icon from '../components/icons/Icon.svelte';
  import { panelSlot } from '../components/layout/panels.svelte';
  import { t } from '../i18n/index.svelte';
  import { session } from '../shell/session.svelte';
  import type { ChatItem, Failure, ToolState } from './chat-events';
  import {
    assistant,
    chatCli,
    chatModel,
    chatOf,
    choose,
    installedClis,
    newChat,
    refreshClis,
    send,
    setImageAccess,
    stopAnswer,
    type Chat,
  } from './chat.svelte';
  import { segments } from './context';
  import AssistantPromptDialog from './AssistantPromptDialog.svelte';

  interface Props {
    /** This editor shows the active tab. */
    active: boolean;
  }
  const { active }: Props = $props();
  const slot = panelSlot();

  const tab = $derived(active ? session.active : null);
  const chat = $derived<Chat | null>(tab ? (assistant.chats[tab.key] ?? null) : null);
  const clis = $derived(installedClis());
  const answering = $derived(chat?.answering ?? false);

  let draft = $state('');
  let promptOpen = $state(false);
  /** A CLI / model change waiting for "Start a new chat?" (AC-25). */
  let pending = $state<{ cli: string; model: string; revert: () => void } | null>(null);
  let log = $state<HTMLElement | null>(null);

  // The tab's chat exists once its panel shows; the CLIs are looked up
  // whenever the panel opens (AC-03).
  $effect(() => {
    if (tab) chatOf(tab.key);
  });
  $effect(() => {
    if (active && !(slot?.collapsed ?? false)) void refreshClis();
  });
  // Follow the newest message while it streams.
  $effect(() => {
    void chat?.items;
    if (log) log.scrollTop = log.scrollHeight;
  });

  function submit(): void {
    if (!tab || draft.trim() === '') return;
    const text = draft;
    draft = '';
    void send(tab, text);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      if (!answering) submit();
    }
  }

  /** A changed CLI or model: asks first while the chat has messages. */
  function change(cli: string, model: string, revert: () => void): void {
    if (!chat) return;
    if (chat.items.length > 0) pending = { cli, model, revert };
    else choose(chat, cli, model);
  }

  function onCliChange(e: Event): void {
    if (!chat) return;
    const el = e.currentTarget as HTMLSelectElement;
    const before = chatCli(chat) ?? '';
    change(el.value, assistant.models[el.value] ?? '', () => (el.value = before));
  }

  function onModelChange(e: Event): void {
    if (!chat) return;
    const el = e.currentTarget as HTMLInputElement;
    const before = chatModel(chat);
    if (el.value.trim() === before) return;
    change(chatCli(chat) ?? '', el.value, () => (el.value = before));
  }

  function confirmChange(yes: boolean): void {
    if (!pending || !chat) return;
    if (yes) choose(chat, pending.cli, pending.model);
    else pending.revert();
    pending = null;
  }

  function stateLabel(s: ToolState): string {
    return t(`assistant.state.${s}`);
  }

  function hintText(error: Failure): string {
    const id = chat?.cli ?? '';
    const name = assistant.clis?.find((c) => c.id === id)?.name ?? id;
    const model = chat?.model || t('assistant.modelDefault');
    switch (error.hint) {
      case 'signIn':
        return t('assistant.hint.signIn', { program: id });
      case 'offline':
        return t('assistant.hint.offline');
      case 'unknownModel':
        return t('assistant.hint.unknownModel', { cli: name, model });
      case 'permission':
        return t('assistant.hint.permission', { cli: name });
      case 'unexpected':
        return t('assistant.hint.unexpected', { cli: name });
    }
  }

  function argsText(args: unknown): string {
    return JSON.stringify(args, null, 2) ?? '';
  }
</script>

{#snippet agentText(text: string)}
  <div class="whitespace-pre-wrap break-words">
    {#each segments(text) as s, i (i)}
      {#if s.kind === 'block'}
        <pre class="my-1 overflow-x-auto rounded bg-pc-bg p-2 font-mono text-xs">{s.text}</pre>
      {:else if s.kind === 'code'}
        <code class="rounded bg-pc-bg px-1 font-mono text-xs">{s.text}</code>
      {:else}
        {s.text}
      {/if}
    {/each}
  </div>
{/snippet}

{#snippet item(it: ChatItem)}
  {#if it.kind === 'user'}
    <div
      class="self-end rounded-md bg-pc-selected px-2.5 py-1.5 whitespace-pre-wrap break-words"
      data-role="user"
    >
      {it.text}
    </div>
  {:else if it.kind === 'agent'}
    <div class="px-0.5" data-role="agent">{@render agentText(it.text)}</div>
  {:else if it.kind === 'tool'}
    <details
      class="rounded-md border border-pc-border bg-pc-raised text-xs"
      data-role="tool"
      data-state={it.state}
    >
      <summary class="flex cursor-pointer items-center gap-2 px-2 py-1">
        <span class="min-w-0 flex-1 truncate font-mono">{it.name}</span>
        <span
          class:text-pc-danger-text={it.state === 'error' || it.state === 'failed'}
          class:text-pc-accent-text={it.state === 'ok'}
          class:text-pc-muted={it.state === 'running' || it.state === 'stopped'}
        >
          {stateLabel(it.state)}
        </span>
      </summary>
      <div class="flex flex-col gap-1 border-t border-pc-border px-2 py-1.5">
        <span class="text-pc-faint">{t('assistant.args')}</span>
        <pre class="overflow-x-auto font-mono whitespace-pre-wrap break-words">{argsText(
            it.args,
          )}</pre>
        {#if it.state !== 'running'}
          <span class="text-pc-faint">{t('assistant.result')}</span>
          <pre class="overflow-x-auto font-mono whitespace-pre-wrap break-words">{it.text}</pre>
          {#each it.images as [w, h], i (i)}
            <span class="font-mono">{t('assistant.image', { w, h })}</span>
          {/each}
        {/if}
      </div>
    </details>
  {:else if it.kind === 'end'}
    <div class="flex flex-col gap-0.5 text-xs" data-role="end" role="status">
      {#if it.outcome === 'stopped'}
        <span class="text-pc-muted">{t('assistant.stopped')}</span>
      {:else}
        <span class="text-pc-danger-text whitespace-pre-wrap break-words">
          {t('assistant.failed', { error: it.error?.text ?? '' })}
        </span>
        {#if it.error}
          <span class="text-pc-muted">{hintText(it.error)}</span>
        {/if}
      {/if}
    </div>
  {:else}
    <div
      class="text-xs text-pc-danger-text whitespace-pre-wrap break-words"
      data-role="end"
      role="status"
    >
      {t('assistant.cannotStart', { error: it.text })}
    </div>
  {/if}
{/snippet}

<Panel bodyClass="flex min-h-0 flex-col gap-2 px-3 pb-3">
  {#snippet actions()}
    <IconButton
      icon="rename"
      label={t('assistant.editPrompt')}
      size="sm"
      disabled={answering}
      onclick={() => (promptOpen = true)}
    />
    <IconButton
      icon="plus"
      label={t('assistant.newChat')}
      size="sm"
      disabled={answering || !chat || chat.items.length === 0}
      onclick={() => chat && newChat(chat)}
    />
  {/snippet}
  {#if assistant.clis !== null && clis.length === 0}
    <div class="flex flex-col gap-1 text-xs text-pc-muted" role="status">
      <p class="text-pc-text">{t('assistant.noCli')}</p>
      {#each assistant.clis as c (c.id)}
        <p class="select-text break-all">
          {t('assistant.install', { name: c.name, url: c.installUrl })}
        </p>
      {/each}
    </div>
  {:else if chat}
    <div class="flex flex-wrap items-center gap-2 text-xs text-pc-muted">
      <select
        class="opt-field"
        aria-label={t('assistant.cli')}
        value={chatCli(chat) ?? ''}
        disabled={answering}
        onfocus={() => void refreshClis()}
        onchange={onCliChange}
      >
        {#each clis as c (c.id)}
          <option value={c.id}>{c.name}</option>
        {/each}
      </select>
      <input
        class="opt-field min-w-0 flex-1"
        aria-label={t('assistant.model')}
        placeholder={t('assistant.modelDefault')}
        value={chatModel(chat)}
        disabled={answering}
        spellcheck="false"
        onchange={onModelChange}
      />
    </div>
    <label class="flex items-center gap-2 text-xs text-pc-muted">
      <input
        type="checkbox"
        checked={chat.imageAccess}
        disabled={answering}
        onchange={(e) => setImageAccess(chat, e.currentTarget.checked)}
      />
      {t('assistant.imageAccess')}
    </label>
    {#if chat.imageAccess}
      <p class="text-xs text-pc-faint" role="note">{t('assistant.imageNotice')}</p>
    {/if}
  {/if}
  <div
    bind:this={log}
    class="flex min-h-24 flex-1 flex-col gap-2 overflow-y-auto text-sm select-text"
    role="log"
    aria-label={t('assistant.log')}
  >
    {#each chat?.items ?? [] as it, i (i)}
      {@render item(it)}
    {/each}
    {#if answering && chat?.items.at(-1)?.kind === 'user'}
      <span class="text-xs text-pc-faint">{t('assistant.working')}</span>
    {/if}
  </div>
  <form
    class="flex items-end gap-1.5"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <textarea
      class="opt-field max-h-32 min-h-9 flex-1 resize-y py-1.5 text-sm"
      rows="2"
      aria-label={t('assistant.message')}
      placeholder={t('assistant.placeholder')}
      bind:value={draft}
      disabled={!chat || clis.length === 0}
      onkeydown={onKeydown}></textarea>
    {#if answering}
      <button
        type="button"
        class="panel-btn flex items-center gap-1"
        onclick={() => chat && stopAnswer(chat)}
      >
        <Icon name="stop" size={14} />
        {t('assistant.stop')}
      </button>
    {:else}
      <button
        type="submit"
        class="panel-btn panel-btn-primary flex items-center gap-1"
        disabled={!chat || clis.length === 0 || draft.trim() === ''}
      >
        <Icon name="send" size={14} />
        {t('assistant.send')}
      </button>
    {/if}
  </form>
</Panel>

{#if pending}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
    role="alertdialog"
    aria-modal="true"
    aria-label={t('assistant.newChatQuestion')}
    tabindex="-1"
    onkeydown={(e) => {
      e.stopPropagation();
      if (e.key === 'Escape') confirmChange(false);
    }}
  >
    <div
      class="w-80 max-w-full rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    >
      <p class="font-semibold">{t('assistant.newChatQuestion')}</p>
      <p class="mt-2 text-xs text-pc-muted">{t('assistant.newChatHelp')}</p>
      <div class="mt-4 flex justify-end gap-2">
        <button type="button" class="panel-btn" onclick={() => confirmChange(false)}>
          {t('common.cancel')}
        </button>
        <button
          type="button"
          class="panel-btn panel-btn-primary"
          onclick={() => confirmChange(true)}
        >
          {t('assistant.newChatConfirm')}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if promptOpen}
  <AssistantPromptDialog onClose={() => (promptOpen = false)} />
{/if}
