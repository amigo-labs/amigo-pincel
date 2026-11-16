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
  <div class="am-dialog w-full max-w-md">
    <header>
      <h2 id="update-question-title">
        {t('update.question')}
      </h2>
    </header>
    <div class="am-body flex flex-col gap-3">
      <div class="space-y-1" role="radiogroup" aria-labelledby="update-question-title">
        {#each MODES as m (m)}
          <label class="am-check flex items-center gap-2 text-sm text-pc-text">
            <input type="radio" name="update-question-mode" value={m} bind:group={mode} />
            {t(`update.${m}`)}
          </label>
        {/each}
      </div>
    </div>
    <footer>
      <button class="am-btn" onclick={onDismiss}>{t('update.close')}</button>
      <button bind:this={confirmButton} class="am-btn" onclick={() => onConfirm(mode)}>
        {t('update.confirm')}
      </button>
    </footer>
  </div>
</div>
