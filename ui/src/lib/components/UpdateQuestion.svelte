<script lang="ts">
  // The first-start question of a desktop copy that can update itself
  // (docs/specs/desktop-app.md AC-22, AC-23): how Pincel handles updates.
  // Confirm stores the choice; Escape or Close store nothing.
  import { onMount } from 'svelte';
  import type { UpdateMode } from '../help/updater';
  import { t } from '../i18n/index.svelte';

  let { onConfirm, onDismiss }: { onConfirm: (mode: UpdateMode) => void; onDismiss: () => void } =
    $props();

  let mode = $state<UpdateMode>('auto');
  let confirmButton: HTMLButtonElement | undefined = $state();

  onMount(() => confirmButton?.focus());

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onDismiss();
    }
  }

  const MODES: readonly UpdateMode[] = ['auto', 'notify', 'off'];
</script>

<div
  class="fixed inset-0 z-20 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-labelledby="update-question-title"
  tabindex="-1"
  onkeydown={onKeydown}
>
  <div class="w-full max-w-md rounded border border-pc-border bg-pc-panel p-6 shadow-xl">
    <h2 id="update-question-title" class="mb-3 text-lg font-semibold">
      {t('update.question')}
    </h2>
    <div class="mb-4 space-y-1" role="radiogroup" aria-labelledby="update-question-title">
      {#each MODES as m (m)}
        <label class="flex items-center gap-2 text-sm text-pc-text">
          <input type="radio" name="update-question-mode" value={m} bind:group={mode} />
          {t(`update.${m}`)}
        </label>
      {/each}
    </div>
    <div class="flex justify-end gap-2">
      <button class="panel-btn" onclick={onDismiss}>{t('update.close')}</button>
      <button bind:this={confirmButton} class="panel-btn" onclick={() => onConfirm(mode)}>
        {t('update.confirm')}
      </button>
    </div>
  </div>
</div>
