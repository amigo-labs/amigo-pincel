<script lang="ts">
  // "Rename layer" dialog behind Layer ▸ Rename… in both modes (spec §9.2);
  // the layers panel also renames inline. Keys typed here never reach the
  // editor's window-level shortcuts.
  import { onMount } from 'svelte';
  import { t } from '../i18n/index.svelte';

  interface Props {
    name: string;
    onRename: (name: string) => void;
    onCancel: () => void;
  }
  const { name, onRename, onCancel }: Props = $props();

  // svelte-ignore state_referenced_locally
  let draft = $state(name);
  let input: HTMLInputElement | undefined = $state();

  onMount(() => {
    input?.focus();
    input?.select();
  });

  function submit(): void {
    const next = draft.trim();
    if (next && next !== name) onRename(next);
    else onCancel();
  }

  function onKeydown(e: KeyboardEvent): void {
    e.stopPropagation();
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
  role="dialog"
  aria-modal="true"
  aria-label={t('layers.renameTitle')}
  tabindex="-1"
  onkeydown={onKeydown}
>
  <form
    class="w-80 max-w-full rounded border border-pc-border bg-pc-panel p-4 text-sm text-pc-text shadow-xl"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <p class="font-semibold">{t('layers.renameTitle')}</p>
    <label class="mt-3 flex flex-col gap-1 text-xs text-pc-muted">
      {t('layers.name')}
      <input bind:this={input} bind:value={draft} class="opt-field w-full text-sm" spellcheck="false" />
    </label>
    <div class="mt-4 flex justify-end gap-2">
      <button type="button" class="panel-btn" onclick={onCancel}>{t('common.cancel')}</button>
      <button type="submit" class="panel-btn panel-btn-primary">{t('layers.rename')}</button>
    </div>
  </form>
</div>
