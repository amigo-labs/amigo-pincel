<script lang="ts">
  // The unsaved-changes prompt (docs/specs/unsaved-changes-prompt.md
  // AC-02): Save has the focus, Tab / Shift+Tab cycle the three buttons,
  // Enter activates the focused one, Escape cancels. While it is open no
  // key reaches the editors, the menus or the browser: a capture-phase
  // window listener sees every key first and stops it.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';
  import type { UnsavedAnswer } from './unsaved.svelte';

  interface Props {
    /** Document name the question names. */
    name: string;
    onAnswer: (answer: UnsavedAnswer) => void;
  }
  const { name, onAnswer }: Props = $props();

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
  class="fixed inset-0 z-60 flex items-center justify-center bg-black/50 p-4"
  role="alertdialog"
  aria-modal="true"
  aria-labelledby="unsaved-prompt-title"
  aria-describedby="unsaved-prompt-question"
  tabindex="-1"
>
  <div
    class="w-96 max-w-full rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
  >
    <p id="unsaved-prompt-title" class="font-semibold">{t('shell.unsavedChanges')}</p>
    <p id="unsaved-prompt-question" class="mt-3 text-pc-muted">
      {t('shell.saveChangesTo', { name })}
    </p>
    <div class="mt-4 flex flex-wrap justify-end gap-2">
      <button
        bind:this={saveButton}
        type="button"
        class="panel-btn panel-btn-primary"
        onclick={() => onAnswer('save')}>{t('shell.save')}</button
      >
      <button bind:this={dontSaveButton} type="button" class="panel-btn" onclick={() => onAnswer('dontSave')}
        >{t('shell.dontSave')}</button
      >
      <button bind:this={cancelButton} type="button" class="panel-btn" onclick={() => onAnswer('cancel')}
        >{t('common.cancel')}</button
      >
    </div>
  </div>
</div>
