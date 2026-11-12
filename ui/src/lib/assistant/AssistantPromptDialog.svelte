<script lang="ts">
  // The Assistant's system-prompt editor (docs/specs/ai-chat-panel.md
  // AC-18, AC-19): Save is off while the text is empty; "Reset to
  // default" restores the default prompt in the app's language. Keys typed
  // here never reach the editor's window-level shortcuts.
  import { onMount } from 'svelte';
  import { locale, t } from '../i18n/index.svelte';
  import { setSystemPrompt, systemPrompt } from './chat.svelte';
  import { defaultPrompt } from './prompts';

  interface Props {
    onClose: () => void;
  }
  const { onClose }: Props = $props();

  let draft = $state(systemPrompt());
  let field: HTMLTextAreaElement | undefined = $state();

  onMount(() => field?.focus());

  function save(): void {
    if (draft.trim() === '') return;
    // The default text saved unchanged stays the default, so it follows
    // the app's language (AC-18).
    setSystemPrompt(draft === defaultPrompt(locale()) ? null : draft);
    onClose();
  }

  function reset(): void {
    setSystemPrompt(null);
    draft = defaultPrompt(locale());
  }

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('assistant.promptTitle')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <form
    class="flex max-h-full w-[36rem] max-w-full flex-col rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <p class="font-semibold">{t('assistant.promptTitle')}</p>
    <label class="mt-3 flex min-h-0 flex-1 flex-col gap-1 text-xs text-pc-muted">
      {t('assistant.promptLabel')}
      <textarea
        bind:this={field}
        bind:value={draft}
        class="opt-field h-80 min-h-0 w-full flex-1 resize-y py-1.5 font-mono text-xs"
        spellcheck="false"></textarea>
    </label>
    <div class="mt-4 flex flex-wrap justify-end gap-2">
      <button type="button" class="panel-btn mr-auto" onclick={reset}
        >{t('assistant.promptReset')}</button
      >
      <button type="button" class="panel-btn" onclick={onClose}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary" disabled={draft.trim() === ''}>
        {t('assistant.promptSave')}
      </button>
    </div>
  </form>
</div>
