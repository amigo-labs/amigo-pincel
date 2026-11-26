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
  class="fixed inset-0 z-(--pc-z-dialog) flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('assistant.promptTitle')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <form
    class="am-dialog flex max-h-full w-[36rem] max-w-full flex-col"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <header>
      <h2>{t('assistant.promptTitle')}</h2>
    </header>
    <div class="am-body flex min-h-0 flex-1 flex-col gap-3">
      <label class="flex min-h-0 flex-1 flex-col gap-1 text-xs text-pc-muted">
        {t('assistant.promptLabel')}
        <textarea
          bind:this={field}
          bind:value={draft}
          class="am-input h-80 min-h-0 w-full flex-1 resize-y py-1.5 font-mono text-xs"
          spellcheck="false"></textarea>
      </label>
    </div>
    <footer class="flex-wrap">
      <button type="button" class="am-btn mr-auto" onclick={reset}
        >{t('assistant.promptReset')}</button
      >
      <button type="button" class="am-btn" onclick={onClose}>{t('common.cancel')}</button>
      <button type="submit" class="am-btn am-btn--primary" disabled={draft.trim() === ''}>
        {t('assistant.promptSave')}
      </button>
    </footer>
  </form>
</div>
