<script lang="ts">
  // The unsaved-changes prompt (docs/specs/unsaved-changes-prompt.md
  // AC-02), and its several-documents form (docs/specs/document-tabs.md
  // AC-11: the names listed, Save All / Don't Save / Cancel): Save has the focus, Tab / Shift+Tab cycle the three buttons,
  // Enter activates the focused one, Escape cancels. While it is open no
  // key reaches the editors, the menus or the browser: a capture-phase
  // window listener sees every key first and stops it.
  import Button from '../components/Button.svelte';
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import type { UnsavedAnswer } from './unsaved.svelte';

  interface Props {
    /** Names of the unsaved documents the question is about, in tab order. */
    names: readonly string[];
    onAnswer: (answer: UnsavedAnswer) => void;
  }
  const { names, onAnswer }: Props = $props();
  const several = $derived(names.length > 1);

  let saveButton: HTMLButtonElement | undefined = $state();
  let dontSaveButton: HTMLButtonElement | undefined = $state();
  let cancelButton: HTMLButtonElement | undefined = $state();

  function buttons(): HTMLButtonElement[] {
    return [saveButton, dontSaveButton, cancelButton].filter(
      (b): b is HTMLButtonElement => b !== undefined,
    );
  }

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    e.preventDefault();
    const list = buttons();
    const at = list.findIndex((b) => b === document.activeElement);
    if (e.key === 'Escape') {
      onAnswer('cancel');
    } else if (e.key === 'Tab') {
      const step = e.shiftKey ? -1 : 1;
      const next = at < 0 ? 0 : (at + step + list.length) % list.length;
      list[next]?.focus();
    } else if (e.key === 'Enter' && !e.repeat) {
      (list[at] ?? saveButton)?.click();
    }
    // Every other key (Space included) does nothing while the prompt is open.
  }

  function onKeyup(e: KeyboardEvent): void {
    // A button activates on Space's keyup; the prompt answers only Enter.
    e.stopPropagation();
    e.preventDefault();
  }

  onMount(() => {
    saveButton?.focus();
    window.addEventListener('keydown', onKeydown, true);
    window.addEventListener('keyup', onKeyup, true);
    return () => {
      window.removeEventListener('keydown', onKeydown, true);
      window.removeEventListener('keyup', onKeyup, true);
    };
  });
</script>

<div
  class="fixed inset-0 z-(--pc-z-popover) flex items-center justify-center bg-black/50 p-4"
  role="alertdialog"
  aria-modal="true"
  aria-labelledby="unsaved-prompt-title"
  aria-describedby="unsaved-prompt-question"
  tabindex="-1"
>
  <div class="am-dialog">
    <header>
      <h2 id="unsaved-prompt-title">{t('shell.unsavedChanges')}</h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <div id="unsaved-prompt-question" class="text-pc-muted">
        {#if several}
          <p>{t('shell.unsavedIn', { n: names.length })}</p>
          <ul class="mt-2 max-h-40 list-disc overflow-y-auto pl-5 text-pc-text">
            {#each names as name, i (i)}
              <li class="truncate">{name}</li>
            {/each}
          </ul>
        {:else}
          <p>{t('shell.saveChangesTo', { name: names[0] ?? '' })}</p>
        {/if}
      </div>
    </div>
    <footer class="flex-wrap">
      <!-- The safe choice first, the action last (amigo-theme-controls AC-07);
           Tab still goes Save → Don't Save → Cancel (onKeydown). -->
      <Button bind:el={cancelButton} shortcut="Esc" onclick={() => onAnswer('cancel')}
        >{t('common.cancel')}</Button
      >
      <Button bind:el={dontSaveButton} variant="danger" onclick={() => onAnswer('dontSave')}
        >{t('shell.dontSave')}</Button
      >
      <Button bind:el={saveButton} variant="primary" onclick={() => onAnswer('save')}
        >{several ? t('shell.saveAll') : t('shell.save')}</Button
      >
    </footer>
  </div>
</div>
